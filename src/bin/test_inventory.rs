use dotenv::dotenv;
use stateset_api::errors::InventoryError;
use tracing::{info, Level};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize environment and tracing
    dotenv().ok();

    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Testing inventory improvements...");

    // Create test InventoryErrors (which is a type alias for ServiceError)
    let error = InventoryError::NotFound("Test product".to_string());
    info!("Error: {}", error);

    let error = InventoryError::ValidationError("Insufficient inventory".to_string());
    info!("Error: {}", error);

    let error = InventoryError::ConcurrentModification(Uuid::new_v4());
    info!("Error: {}", error);

    info!("Inventory error types are working properly!");

    Ok(())
}
#![cfg(feature = "demos")]
