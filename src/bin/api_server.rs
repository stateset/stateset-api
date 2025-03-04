use axum::{
    routing::{get, post},
    Router, 
    extract::{State, Json, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use std::net::SocketAddr;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::mpsc;
use tracing::{info, error};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, ConnectOptions, Database};
use std::time::Duration;

// App state that will be shared across handlers
#[derive(Clone)]
pub struct AppState {
    db_pool: Arc<DatabaseConnection>,
    event_sender: mpsc::Sender<Event>,
}

// Basic event system
#[derive(Debug, Clone)]
pub enum Event {
    OrderCreated(Uuid),
    OrderUpdated(Uuid),
    OrderStatusChanged {
        order_id: Uuid,
        old_status: String,
        new_status: String,
    },
    // We can add more event types as needed
}

// Order model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub status: String,
    pub total_amount: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Simple order creation request
#[derive(Debug, Deserialize)]
pub struct CreateOrderRequest {
    pub customer_id: Uuid,
    pub items: Vec<OrderItem>,
}

// Order item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderItem {
    pub product_id: Uuid,
    pub quantity: i32,
    pub price: f64,
}

// Order response
#[derive(Debug, Serialize)]
pub struct OrderResponse {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub status: String,
    pub total_amount: f64,
    pub created_at: DateTime<Utc>,
    pub items: Vec<OrderItem>,
}

// Order status change request
#[derive(Debug, Deserialize)]
pub struct UpdateOrderStatusRequest {
    pub status: String,
}

// Health response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub timestamp: String,
}

// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub details: Option<String>,
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        let body = Json(json!({
            "error": self.error,
            "details": self.details,
        }));
        (StatusCode::BAD_REQUEST, body).into_response()
    }
}

// Handler for health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(HealthResponse {
        status: "up".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: Utc::now().to_rfc3339(),
    })
}

// Handler for root endpoint
async fn root() -> impl IntoResponse {
    Json(json!({
        "name": "Stateset API",
        "version": env!("CARGO_PKG_VERSION"),
        "documentation": "/docs",
    }))
}

// Handler for creating orders
async fn create_order(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateOrderRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    // In a real implementation, this would write to the database
    let order_id = Uuid::new_v4();
    let now = Utc::now();
    
    // Calculate total amount from items
    let total_amount = payload.items.iter()
        .map(|item| item.price * item.quantity as f64)
        .sum();
    
    let order = Order {
        id: order_id,
        customer_id: payload.customer_id,
        status: "pending".to_string(),
        total_amount,
        created_at: now,
        updated_at: now,
    };
    
    // Send event notification
    if let Err(e) = state.event_sender.send(Event::OrderCreated(order_id)).await {
        error!("Failed to send order created event: {}", e);
    }
    
    let response = OrderResponse {
        id: order.id,
        customer_id: order.customer_id,
        status: order.status,
        total_amount: order.total_amount,
        created_at: order.created_at,
        items: payload.items,
    };
    
    Ok((StatusCode::CREATED, Json(response)))
}

// Handler for retrieving orders
async fn get_order(
    State(state): State<Arc<AppState>>,
    Path(order_id): Path<Uuid>,
) -> Result<impl IntoResponse, ErrorResponse> {
    // In a real implementation, this would fetch from the database
    // For demonstration, we'll just create a mock order
    let now = Utc::now();
    
    let order = Order {
        id: order_id,
        customer_id: Uuid::new_v4(), // Mock customer id
        status: "pending".to_string(),
        total_amount: 99.99,
        created_at: now - chrono::Duration::days(1),
        updated_at: now,
    };
    
    let response = OrderResponse {
        id: order.id,
        customer_id: order.customer_id,
        status: order.status,
        total_amount: order.total_amount,
        created_at: order.created_at,
        items: vec![
            OrderItem {
                product_id: Uuid::new_v4(),
                quantity: 2,
                price: 49.99,
            }
        ],
    };
    
    Ok(Json(response))
}

// Handler for updating order status
async fn update_order_status(
    State(state): State<Arc<AppState>>,
    Path(order_id): Path<Uuid>,
    Json(payload): Json<UpdateOrderStatusRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    // In a real implementation, this would update the database
    
    // Mock old status
    let old_status = "pending".to_string();
    
    // Send event notification
    if let Err(e) = state.event_sender.send(Event::OrderStatusChanged {
        order_id,
        old_status: old_status.clone(),
        new_status: payload.status.clone(),
    }).await {
        error!("Failed to send order status changed event: {}", e);
    }
    
    Ok(Json(json!({
        "id": order_id,
        "status": payload.status,
        "updated_at": Utc::now().to_rfc3339(),
    })))
}

// Setup API routes
fn routes(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/health", get(health_check))
        .route("/orders", post(create_order))
        .route("/orders/:id", get(get_order))
        .route("/orders/:id/status", post(update_order_status))
        .with_state(app_state)
}

// Event processor
async fn process_events(mut rx: mpsc::Receiver<Event>) {
    while let Some(event) = rx.recv().await {
        match event {
            Event::OrderCreated(id) => {
                info!("Processing order created event for order {}", id);
                // Add business logic for order creation processing
            },
            Event::OrderUpdated(id) => {
                info!("Processing order updated event for order {}", id);
                // Add business logic for order update processing
            },
            Event::OrderStatusChanged { order_id, old_status, new_status } => {
                info!(
                    "Processing order status changed event for order {}: {} -> {}", 
                    order_id, old_status, new_status
                );
                // Add business logic for status change processing
            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    info!("Stateset API Server starting...");
    
    // Configure database connection
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite::memory:".to_string());
    
    info!("Connecting to database: {}", database_url);
    
    // Setup connection options
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(10)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(10))
        .acquire_timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(300))
        .sqlx_logging(true);
    
    // Create database connection
    let db_conn = Database::connect(options).await?;
    let db_pool = Arc::new(db_conn);
    
    // Set up event channel
    let (sender, receiver) = mpsc::channel(100);
    
    // Create application state
    let app_state = Arc::new(AppState {
        db_pool,
        event_sender: sender,
    });
    
    // Start the event processor
    tokio::spawn(process_events(receiver));
    
    // Build the API router
    let app = routes(app_state);
    
    // Start the server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("Listening on http://{}", addr);
    
    axum::serve(
        tokio::net::TcpListener::bind(addr).await?,
        app.into_make_service(),
    ).await?;
    
    Ok(())
}