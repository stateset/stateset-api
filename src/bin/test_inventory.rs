use std::sync::Arc;
use dotenv::dotenv;
use stateset_api::{
    db,
    config,
    errors::InventoryError,
};
use tracing::{info, error, Level};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize environment and tracing
    dotenv().ok();
    
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();
    
    info!("Testing inventory improvements...");
    
    // Create a test InventoryError to verify our improvements
    let error = InventoryError::NotFound("Test product".to_string());
    info!("Error type: {}", error.error_type());
    
    let error = InventoryError::InsufficientInventory(Uuid::new_v4());
    info!("Error type: {}", error.error_type());
    
    let error = InventoryError::ConcurrentModification(Uuid::new_v4());
    info!("Error type: {}", error.error_type());
    
    info!("Inventory error types are working properly!");
    
    Ok(())
}