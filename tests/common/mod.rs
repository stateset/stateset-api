use std::sync::Arc;
use std::time::Duration;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    middleware,
    routing::get,
    Json, Router,
};
use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{ConnectionTrait, DatabaseBackend as DbBackend, Statement};
use serde_json::{json, Value};
use stateset_api::entities::commerce::product_variant;
use stateset_api::{
    auth::{AuthConfig, AuthService, Claims, User},
    config::AppConfig,
    db,
    events::{self, EventSender},
    handlers::AppServices,
    message_queue::InMemoryMessageQueue,
    services::commerce::product_catalog_service::{CreateProductInput, CreateVariantInput},
    AppState,
};
use tokio::sync::mpsc;
use tower::ServiceExt;
use uuid::Uuid;

/// Helper harness for spinning up an application state backed by an in-memory SQLite database.
pub struct TestApp {
    router: Router,
    pub state: AppState,
    token: String,
    #[allow(dead_code)]
    auth_service: Arc<AuthService>,
    _event_task: tokio::task::JoinHandle<()>,
}

impl TestApp {
    /// Construct a new test application with fresh database state.
    pub async fn new() -> Self {
        // Minimal configuration suitable for tests.
        let db_file = "stateset_test.db";
        let _ = std::fs::remove_file(db_file);

        let mut cfg = AppConfig::new(
            format!("sqlite://{db_file}?mode=rwc"),
            "redis://127.0.0.1:6379".to_string(),
            "test_secret_key_for_testing_purposes_only_32chars".to_string(),
            3600,
            86_400,
            "127.0.0.1".to_string(),
            18_080,
            "test".to_string(),
        );
        cfg.auto_migrate = true;
        cfg.db_max_connections = 1;
        cfg.db_min_connections = 1;

        let pool = db::establish_connection_from_app_config(&cfg)
            .await
            .expect("failed to create test database");

        // Ensure a clean schema for each test run by clearing key tables before migrations.
        let reset_statements = [
            "DROP TABLE IF EXISTS order_items;",
            "DROP TABLE IF EXISTS orders;",
            "DROP TABLE IF EXISTS product_variants;",
            "DROP TABLE IF EXISTS products;",
            "DROP TABLE IF EXISTS customer_addresses;",
            "DROP TABLE IF EXISTS customers;",
            "DROP TABLE IF EXISTS users;",
            "DROP TABLE IF EXISTS user_roles;",
            "DROP TABLE IF EXISTS refresh_tokens;",
            "DROP TABLE IF EXISTS api_keys;",
        ];
        for sql in reset_statements {
            let _ = pool
                .execute(Statement::from_string(DbBackend::Sqlite, sql.to_string()))
                .await;
        }

        db::run_migrations(&pool)
            .await
            .expect("failed to run migrations in tests");

        // Ensure critical auth tables exist for SQLite in-memory runs
        let ensure_sql = [
            r#"CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                email TEXT NOT NULL UNIQUE,
                password_hash TEXT NOT NULL,
                tenant_id TEXT,
                active INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );"#,
            r#"CREATE TABLE IF NOT EXISTS user_roles (
                id TEXT PRIMARY KEY NOT NULL,
                user_id TEXT NOT NULL,
                role_name TEXT NOT NULL,
                created_at TEXT NOT NULL
            );"#,
            r#"CREATE TABLE IF NOT EXISTS refresh_tokens (
                id TEXT PRIMARY KEY NOT NULL,
                user_id TEXT NOT NULL,
                token_id TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                revoked INTEGER NOT NULL DEFAULT 0
            );"#,
            r#"CREATE TABLE IF NOT EXISTS api_keys (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                key_hash TEXT NOT NULL,
                user_id TEXT NOT NULL,
                tenant_id TEXT,
                created_at TEXT NOT NULL,
                expires_at TEXT,
                last_used_at TEXT,
                revoked INTEGER NOT NULL DEFAULT 0
            );"#,
        ];

        for sql in ensure_sql {
            pool.execute(Statement::from_string(DbBackend::Sqlite, sql.into()))
                .await
                .expect("ensure auth table exists");
        }

        let db_arc = Arc::new(pool);
        let (event_tx, event_rx) = mpsc::channel(256);
        let event_sender = EventSender::new(event_tx);
        let event_task = tokio::spawn(events::process_events(event_rx, None, None));

        let inventory_service = stateset_api::services::inventory::InventoryService::new(
            db_arc.clone(),
            event_sender.clone(),
        );

        let redis_client = Arc::new(
            redis::Client::open(cfg.redis_url.clone()).expect("invalid redis url for tests"),
        );

        let auth_cfg = AuthConfig::new(
            cfg.jwt_secret.clone(),
            "stateset-api".to_string(),
            "stateset-auth".to_string(),
            Duration::from_secs(cfg.jwt_expiration as u64),
            Duration::from_secs(cfg.refresh_token_expiration as u64),
            "sk_".to_string(),
        )
        .expect("valid auth config for tests");
        let auth_service = Arc::new(AuthService::new(auth_cfg, db_arc.clone()));

        let base_logger =
            stateset_api::logging::setup_logger(stateset_api::logging::LoggerConfig::default());
        let services = AppServices::new(
            db_arc.clone(),
            Arc::new(event_sender.clone()),
            redis_client.clone(),
            auth_service.clone(),
            Arc::new(InMemoryMessageQueue::new()),
            base_logger,
        );

        let state = AppState {
            db: db_arc,
            config: cfg.clone(),
            event_sender,
            inventory_service,
            services,
            redis: redis_client,
        };

        // Ensure generated tokens include admin role and useful permissions.
        std::env::set_var("AUTH_ADMIN", "1");
        std::env::set_var("STATESET_AUTH_ALLOW_ADMIN_OVERRIDE", "1");
        std::env::set_var(
            "AUTH_DEFAULT_PERMISSIONS",
            "orders:read,orders:create,orders:update,purchaseorders:manage,asns:manage",
        );

        let user = User {
            id: Uuid::new_v4(),
            name: "Test User".to_string(),
            email: "test@example.com".to_string(),
            password_hash: "".to_string(),
            tenant_id: None,
            active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let now = Utc::now();
        let access_claims = Claims {
            sub: user.id.to_string(),
            name: Some(user.name.clone()),
            email: Some(user.email.clone()),
            roles: vec!["admin".to_string()],
            permissions: std::env::var("AUTH_DEFAULT_PERMISSIONS")
                .unwrap_or_else(|_| "orders:read,orders:create,orders:update".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
            tenant_id: user.tenant_id.clone(),
            jti: Uuid::new_v4().to_string(),
            iat: now.timestamp(),
            exp: (now + chrono::Duration::hours(1)).timestamp(),
            nbf: now.timestamp(),
            iss: "stateset-auth".to_string(),
            aud: "stateset-api".to_string(),
            scope: None,
        };

        let access_token = jsonwebtoken::encode(
            &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
            &access_claims,
            &jsonwebtoken::EncodingKey::from_secret(cfg.jwt_secret.as_bytes()),
        )
        .expect("encode access token");

        let auth_service_for_layer = auth_service.clone();
        let api_router = stateset_api::api_v1_routes().layer(middleware::from_fn_with_state(
            auth_service_for_layer,
            |axum::extract::State(auth): axum::extract::State<Arc<AuthService>>,
             mut req: Request<Body>,
             next: axum::middleware::Next| async move {
                req.extensions_mut().insert(auth);
                next.run(req).await
            },
        ));

        let router = Router::new()
            .route("/health", get(stateset_api::health::simple_health_check))
            .route(
                "/health/live",
                get(|| async {
                    (
                        StatusCode::OK,
                        Json(json!({
                            "status": "up",
                        })),
                    )
                }),
            )
            .route(
                "/health/ready",
                get(|| async {
                    (
                        StatusCode::OK,
                        Json(json!({
                            "status": "up",
                        })),
                    )
                }),
            )
            .nest("/api/v1", api_router)
            .with_state(state.clone());

        Self {
            router,
            state,
            token: access_token,
            auth_service,
            _event_task: event_task,
        }
    }

    /// Access the auth service used by the test application.
    #[allow(dead_code)]
    pub fn auth_service(&self) -> Arc<AuthService> {
        self.auth_service.clone()
    }

    /// Access the bearer token for the default admin user.
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Send a request against the router with an optional bearer token.
    pub async fn request(
        &self,
        method: Method,
        uri: &str,
        body: Option<Value>,
        token: Option<&str>,
    ) -> axum::response::Response {
        let mut builder = Request::builder().method(method).uri(uri);

        if let Some(tok) = token {
            builder = builder.header("authorization", format!("Bearer {}", tok));
        }

        let body = if let Some(json) = body {
            builder = builder.header("content-type", "application/json");
            Body::from(serde_json::to_vec(&json).expect("failed to serialize json request body"))
        } else {
            Body::empty()
        };

        let request = builder.body(body).expect("failed to build request");
        self.router
            .clone()
            .oneshot(request)
            .await
            .expect("router error during test request")
    }

    /// Convenience helper for authenticated JSON requests.
    pub async fn request_authenticated(
        &self,
        method: Method,
        uri: &str,
        body: Option<Value>,
    ) -> axum::response::Response {
        self.request(method, uri, body, Some(self.token())).await
    }

    pub async fn request_authenticated_with_headers(
        &self,
        method: Method,
        uri: &str,
        body: Option<Value>,
        headers: &[(&str, &str)],
    ) -> axum::response::Response {
        let mut builder = Request::builder().method(method).uri(uri);
        builder = builder.header("authorization", format!("Bearer {}", self.token()));
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }

        let body = if let Some(json) = body {
            builder = builder.header("content-type", "application/json");
            Body::from(serde_json::to_vec(&json).expect("failed to serialize json request body"))
        } else {
            Body::empty()
        };

        let request = builder.body(body).expect("failed to build request");
        self.router
            .clone()
            .oneshot(request)
            .await
            .expect("router error during test request")
    }

    pub async fn seed_product_variant(&self, sku: &str, price: Decimal) -> product_variant::Model {
        use std::collections::HashMap;

        let catalog = self.state.services.product_catalog.clone();
        let product = catalog
            .create_product(CreateProductInput {
                name: format!("Test Product {}", sku),
                sku: format!("test-product-{}", sku.to_lowercase()),
                description: Some("Test product seeded for integration tests".to_string()),
                price,
                currency: "USD".to_string(),
                is_active: true,
                is_digital: false,
                image_url: None,
                brand: None,
                manufacturer: None,
                weight_kg: None,
                dimensions_cm: None,
                tags: None,
                cost_price: Some(price),
                msrp: None,
                tax_rate: None,
                meta_title: None,
                meta_description: None,
                reorder_point: None,
            })
            .await
            .expect("seed product for tests");

        catalog
            .create_variant(CreateVariantInput {
                product_id: product.id,
                sku: sku.to_string(),
                name: format!("Variant {}", sku),
                price,
                compare_at_price: None,
                cost: Some(price),
                weight: None,
                dimensions: None,
                options: HashMap::new(),
                inventory_tracking: true,
                position: 0,
            })
            .await
            .expect("seed product variant for tests")
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        self._event_task.abort();
    }
}
