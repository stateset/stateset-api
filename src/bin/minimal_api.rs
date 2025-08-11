use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::{
    entity::prelude::*,
    ActiveModelTrait, 
    ActiveValue::Set,
    Database, DatabaseConnection, EntityTrait,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::{error, info};
use uuid::Uuid;

// Simple Product Entity
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "products")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub sku: String,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub price: Decimal,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Request/Response DTOs
#[derive(Debug, Deserialize)]
struct CreateProductRequest {
    name: String,
    description: Option<String>,
    sku: String,
    price: Decimal,
}

#[derive(Debug, Serialize)]
struct ProductResponse {
    id: Uuid,
    name: String,
    description: Option<String>,
    sku: String,
    price: Decimal,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

impl From<Model> for ProductResponse {
    fn from(product: Model) -> Self {
        ProductResponse {
            id: product.id,
            name: product.name,
            description: product.description,
            sku: product.sku,
            price: product.price,
            is_active: product.is_active,
            created_at: product.created_at,
            updated_at: product.updated_at,
        }
    }
}

// App State
#[derive(Clone)]
struct AppState {
    db: Arc<DatabaseConnection>,
}

// Handlers
async fn create_product(
    State(state): State<AppState>,
    Json(payload): Json<CreateProductRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let product = ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(payload.name),
        description: Set(payload.description),
        sku: Set(payload.sku),
        price: Set(payload.price),
        is_active: Set(true),
        created_at: Set(Utc::now()),
        updated_at: Set(None),
    };

    let result = product.insert(&*state.db).await.map_err(|e| {
        error!("Failed to create product: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((StatusCode::CREATED, Json(ProductResponse::from(result))))
}

async fn get_product(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let product = Entity::find_by_id(id)
        .one(&*state.db)
        .await
        .map_err(|e| {
            error!("Failed to fetch product: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(ProductResponse::from(product)))
}

async fn list_products(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let products = Entity::find()
        .all(&*state.db)
        .await
        .map_err(|e| {
            error!("Failed to list products: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let response: Vec<ProductResponse> = products.into_iter().map(ProductResponse::from).collect();
    Ok(Json(response))
}

async fn health_check() -> impl IntoResponse {
    "OK"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("debug")
        .init();

    info!("Starting minimal API...");

    // Get database URL from environment
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/stateset_db".to_string());

    // Connect to database
    info!("Connecting to database...");
    let db = Database::connect(&database_url).await?;
    
    // Create products table if it doesn't exist
    let create_table_sql = r#"
        CREATE TABLE IF NOT EXISTS products (
            id UUID PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            description TEXT,
            sku VARCHAR(100) NOT NULL UNIQUE,
            price DECIMAL(19,4) NOT NULL,
            is_active BOOLEAN NOT NULL DEFAULT true,
            created_at TIMESTAMPTZ NOT NULL,
            updated_at TIMESTAMPTZ
        );
    "#;
    
    db.execute_unprepared(create_table_sql).await?;
    info!("Database table ready");

    // Create app state
    let state = AppState {
        db: Arc::new(db),
    };

    // Build the router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/products", get(list_products))
        .route("/api/products", post(create_product))
        .route("/api/products/:id", get(get_product))
        .layer(CorsLayer::permissive())
        .with_state(state);

    // Run the server
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    info!("Listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
} 