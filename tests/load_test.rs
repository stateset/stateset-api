#![cfg(feature = "full-suite")]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use serde_json::json;
use stateset_api::{config, db, events::EventSender, handlers::AppServices, AppState};
use std::sync::Arc;
use tokio::sync::mpsc;
use tower::ServiceExt;

// Benchmark setup
fn create_benchmark_app_state() -> AppState {
    let config = config::AppConfig {
        database_url: ":memory:".to_string(),
        host: "127.0.0.1".to_string(),
        port: 8080,
        auto_migrate: false,
        env: "benchmark".to_string(),
        jwt_secret: "benchmark_secret_key".to_string(),
        jwt_expiration: 3600,
        refresh_token_expiration: 86400,
        redis_url: "redis://localhost:6379".to_string(),
        rate_limit_requests_per_window: 10000,
        rate_limit_window_seconds: 60,
        rate_limit_enable_headers: false,
        log_level: "error".to_string(),
        log_json: false,
        cors_allowed_origins: None,
        cors_allow_credentials: false,
        grpc_port: None,
        is_production: false,
        rate_limit_path_policies: None,
        rate_limit_api_key_policies: None,
        rate_limit_user_policies: None,
        statement_timeout: None,
    };

    let db_arc = Arc::new(
        db::establish_connection(&config.database_url)
            .await
            .unwrap(),
    );
    let (tx, _rx) = mpsc::channel(1000);
    let event_sender = EventSender::new(tx);

    let inventory_service = stateset_api::services::inventory::InventoryService::new(
        db_arc.clone(),
        event_sender.clone(),
    );

    let order_service = stateset_api::services::orders::OrderService::new(
        db_arc.clone(),
        Some(Arc::new(event_sender.clone())),
    );

    AppState {
        db: db_arc,
        config,
        event_sender,
        inventory_service,
        services: AppServices {
            product_catalog: Arc::new(
                stateset_api::services::commerce::ProductCatalogService::new(
                    db_arc.clone(),
                    event_sender.clone(),
                ),
            ),
            cart: Arc::new(stateset_api::services::commerce::CartService::new(
                db_arc.clone(),
                event_sender.clone(),
            )),
            checkout: Arc::new(stateset_api::services::commerce::CheckoutService::new(
                db_arc.clone(),
                event_sender.clone(),
                order_service.clone(),
            )),
            customer: Arc::new(stateset_api::services::commerce::CustomerService::new(
                db_arc.clone(),
                event_sender.clone(),
                Arc::new(stateset_api::auth::AuthService::new(
                    stateset_api::auth::AuthConfig::new(
                        config.jwt_secret.clone(),
                        "stateset-api".to_string(),
                        "stateset-auth".to_string(),
                        std::time::Duration::from_secs(config.jwt_expiration as u64),
                        std::time::Duration::from_secs(config.refresh_token_expiration as u64),
                        "sk_".to_string(),
                    ),
                    db_arc.clone(),
                )),
            )),
            order: order_service,
        },
        redis: Arc::new(
            redis::Client::open(config.redis_url.clone())
                .unwrap_or_else(|_| redis::Client::open("redis://mock:6379").unwrap()),
        ),
    }
}

async fn setup_benchmark_app() -> axum::Router {
    let state = create_benchmark_app_state();

    axum::Router::new()
        .nest(
            "/api/v1",
            stateset_api::api_v1_routes().with_state(state.clone()),
        )
        .with_state(state)
}

pub fn benchmark_health_endpoint(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("health_endpoint", |b| {
        b.to_async(&rt).iter(|| async {
            let app = setup_benchmark_app().await;
            let request = Request::builder()
                .uri("/api/v1/status")
                .body(Body::empty())
                .unwrap();

            let _response = app.oneshot(request).await.unwrap();
        });
    });
}

pub fn benchmark_order_creation(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("order_creation", |b| {
        b.to_async(&rt).iter(|| async {
            let app = setup_benchmark_app().await;

            let order_data = json!({
                "customer_id": "550e8400-e29b-41d4-a716-446655440000",
                "items": [
                    {
                        "product_id": "550e8400-e29b-41d4-a716-446655440001",
                        "quantity": 1,
                        "unit_price": 29.99
                    }
                ]
            });

            let request = Request::builder()
                .method("POST")
                .uri("/api/v1/orders")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&order_data).unwrap()))
                .unwrap();

            let _response = app.oneshot(request).await.unwrap();
        });
    });
}

pub fn benchmark_inventory_listing(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("inventory_listing", |b| {
        b.to_async(&rt).iter(|| async {
            let app = setup_benchmark_app().await;
            let request = Request::builder()
                .uri("/api/v1/inventory?page=1&limit=20")
                .body(Body::empty())
                .unwrap();

            let _response = app.oneshot(request).await.unwrap();
        });
    });
}

pub fn benchmark_concurrent_requests(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("concurrent_requests_10", |b| {
        b.to_async(&rt).iter(|| async {
            let app = setup_benchmark_app().await;

            // Simulate 10 concurrent requests
            let mut handles = vec![];
            for _ in 0..10 {
                let app_clone = app.clone();
                let handle = tokio::spawn(async move {
                    let request = Request::builder()
                        .uri("/api/v1/status")
                        .body(Body::empty())
                        .unwrap();

                    let _response = app_clone.oneshot(request).await.unwrap();
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.await.unwrap();
            }
        });
    });
}

criterion_group!(
    benches,
    benchmark_health_endpoint,
    benchmark_order_creation,
    benchmark_inventory_listing,
    benchmark_concurrent_requests
);
criterion_main!(benches);

// Load testing utilities
#[cfg(test)]
mod load_tests {
    use super::*;
    use std::time::{Duration, Instant};
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_sustained_load() {
        println!("🏋️ Running sustained load test...");

        let app = setup_benchmark_app().await;
        let start_time = Instant::now();
        let test_duration = Duration::from_secs(30); // 30 second test
        let mut request_count = 0;

        while start_time.elapsed() < test_duration {
            let app_clone = app.clone();

            // Make multiple concurrent requests
            let mut handles = vec![];
            for _ in 0..5 {
                let app_clone = app_clone.clone();
                let handle = tokio::spawn(async move {
                    let request = Request::builder()
                        .uri("/api/v1/status")
                        .body(Body::empty())
                        .unwrap();

                    let response = app_clone.oneshot(request).await.unwrap();
                    assert_eq!(response.status(), StatusCode::OK);
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.await.unwrap();
            }

            request_count += 5;

            // Small delay to prevent overwhelming
            sleep(Duration::from_millis(10)).await;
        }

        let elapsed = start_time.elapsed();
        let requests_per_second = request_count as f64 / elapsed.as_secs_f64();

        println!("✅ Sustained load test completed:");
        println!("  - Total requests: {}", request_count);
        println!("  - Duration: {:.2}s", elapsed.as_secs_f64());
        println!("  - Requests/second: {:.2}", requests_per_second);

        // Should handle at least 100 requests/second
        assert!(
            requests_per_second > 100.0,
            "Performance too low: {:.2} req/s",
            requests_per_second
        );
    }

    #[tokio::test]
    async fn test_memory_usage() {
        println!("🧠 Testing memory usage under load...");

        let app = setup_benchmark_app().await;
        let mut handles = vec![];

        // Create 100 concurrent requests
        for _ in 0..100 {
            let app_clone = app.clone();
            let handle = tokio::spawn(async move {
                for _ in 0..10 {
                    let request = Request::builder()
                        .uri("/api/v1/status")
                        .body(Body::empty())
                        .unwrap();

                    let _response = app_clone.clone().oneshot(request).await.unwrap();
                }
            });
            handles.push(handle);
        }

        // Wait for all requests to complete
        for handle in handles {
            handle.await.unwrap();
        }

        println!("✅ Memory usage test completed successfully");
        // Memory leak detection would require external monitoring tools
    }

    #[tokio::test]
    async fn test_database_connection_pool() {
        println!("💾 Testing database connection pool...");

        let app = setup_benchmark_app().await;
        let mut handles = vec![];

        // Create concurrent requests that access the database
        for i in 0..20 {
            let app_clone = app.clone();
            let handle = tokio::spawn(async move {
                for j in 0..5 {
                    let request = Request::builder()
                        .uri("/api/v1/orders?page=1&limit=10")
                        .body(Body::empty())
                        .unwrap();

                    let response = app_clone.clone().oneshot(request).await.unwrap();
                    assert_eq!(response.status(), StatusCode::OK);
                }
            });
            handles.push(handle);
        }

        // Wait for all database operations to complete
        for handle in handles {
            handle.await.unwrap();
        }

        println!("✅ Database connection pool test completed successfully");
    }
}
