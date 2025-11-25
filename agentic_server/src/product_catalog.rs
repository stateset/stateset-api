use crate::errors::ServiceError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{debug, info};

/// Product details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: String,
    pub name: String,
    pub description: String,
    pub price: i64, // Price in cents
    pub currency: String,
    pub inventory_quantity: i32,
    pub is_active: bool,
    pub category: Option<String>,
    pub image_url: Option<String>,
    pub weight_grams: Option<i32>, // For shipping calculations
    pub metadata: HashMap<String, String>,
}

/// Inventory reservation
#[derive(Debug, Clone)]
struct InventoryReservation {
    product_id: String,
    quantity: i32,
    session_id: String,
    expires_at: chrono::DateTime<chrono::Utc>,
}

/// Product catalog service
#[derive(Clone)]
pub struct ProductCatalogService {
    products: Arc<RwLock<HashMap<String, Product>>>,
    reservations: Arc<RwLock<Vec<InventoryReservation>>>,
}

impl ProductCatalogService {
    pub fn new() -> Self {
        let mut products = HashMap::new();

        // Add demo products
        products.insert(
            "laptop_pro_16_inch".to_string(),
            Product {
                id: "laptop_pro_16_inch".to_string(),
                name: "MacBook Pro 16\"".to_string(),
                description: "Apple M3 Max, 48GB RAM, 1TB SSD".to_string(),
                price: 349900, // $3,499.00
                currency: "usd".to_string(),
                inventory_quantity: 15,
                is_active: true,
                category: Some("Electronics".to_string()),
                image_url: Some("https://example.com/laptop.jpg".to_string()),
                weight_grams: Some(2100),
                metadata: HashMap::new(),
            },
        );

        products.insert(
            "item_123".to_string(),
            Product {
                id: "item_123".to_string(),
                name: "Wireless Mouse".to_string(),
                description: "Ergonomic wireless mouse with USB-C charging".to_string(),
                price: 7999, // $79.99
                currency: "usd".to_string(),
                inventory_quantity: 250,
                is_active: true,
                category: Some("Accessories".to_string()),
                image_url: Some("https://example.com/mouse.jpg".to_string()),
                weight_grams: Some(100),
                metadata: HashMap::new(),
            },
        );

        products.insert(
            "test".to_string(),
            Product {
                id: "test".to_string(),
                name: "Test Product".to_string(),
                description: "For testing purposes".to_string(),
                price: 5000, // $50.00
                currency: "usd".to_string(),
                inventory_quantity: 1000,
                is_active: true,
                category: Some("Test".to_string()),
                image_url: None,
                weight_grams: Some(500),
                metadata: HashMap::new(),
            },
        );

        Self {
            products: Arc::new(RwLock::new(products)),
            reservations: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get all active products
    pub async fn get_all_products(&self) -> Result<Vec<Product>, anyhow::Error> {
        let products = self.products.read().unwrap();
        Ok(products
            .values()
            .filter(|p| p.is_active)
            .cloned()
            .collect())
    }

    /// Get product by ID
    pub fn get_product(&self, product_id: &str) -> Result<Product, ServiceError> {
        let products = self.products.read().unwrap();

        products
            .get(product_id)
            .filter(|p| p.is_active)
            .cloned()
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Product {} not found or inactive", product_id))
            })
    }

    /// Check if product has sufficient inventory
    pub fn check_inventory(&self, product_id: &str, quantity: i32) -> Result<bool, ServiceError> {
        let product = self.get_product(product_id)?;

        // Check available inventory (excluding reservations)
        let reserved = self.get_reserved_quantity(product_id);
        let available = product.inventory_quantity - reserved;

        Ok(available >= quantity)
    }

    /// Reserve inventory for a checkout session
    pub fn reserve_inventory(
        &self,
        product_id: &str,
        quantity: i32,
        session_id: &str,
    ) -> Result<(), ServiceError> {
        // Check inventory first
        if !self.check_inventory(product_id, quantity)? {
            let product = self.get_product(product_id)?;
            let reserved = self.get_reserved_quantity(product_id);
            let available = product.inventory_quantity - reserved;

            return Err(ServiceError::InsufficientStock(format!(
                "Insufficient stock for {}: requested {}, available {}",
                product_id, quantity, available
            )));
        }

        // Clean up expired reservations
        self.cleanup_expired_reservations();

        // Create reservation (expires in 1 hour)
        let reservation = InventoryReservation {
            product_id: product_id.to_string(),
            quantity,
            session_id: session_id.to_string(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        };

        let mut reservations = self.reservations.write().unwrap();
        reservations.push(reservation);

        debug!(
            "Reserved {} units of {} for session {}",
            quantity, product_id, session_id
        );
        Ok(())
    }

    /// Release inventory reservation (on cancel or expiry)
    pub fn release_reservation(&self, session_id: &str) {
        let mut reservations = self.reservations.write().unwrap();
        reservations.retain(|r| r.session_id != session_id);
        debug!("Released reservations for session {}", session_id);
    }

    /// Commit inventory (on successful checkout)
    pub fn commit_inventory(&self, session_id: &str) -> Result<(), ServiceError> {
        let mut reservations = self.reservations.write().unwrap();
        let mut products = self.products.write().unwrap();

        // Find all reservations for this session
        let session_reservations: Vec<_> = reservations
            .iter()
            .filter(|r| r.session_id == session_id)
            .cloned()
            .collect();

        // Deduct from inventory
        for reservation in &session_reservations {
            if let Some(product) = products.get_mut(&reservation.product_id) {
                product.inventory_quantity -= reservation.quantity;
                info!(
                    "Committed {} units of {} (remaining: {})",
                    reservation.quantity, reservation.product_id, product.inventory_quantity
                );
            }
        }

        // Remove reservations
        reservations.retain(|r| r.session_id != session_id);

        Ok(())
    }

    /// Get total reserved quantity for a product
    fn get_reserved_quantity(&self, product_id: &str) -> i32 {
        let reservations = self.reservations.read().unwrap();
        let now = chrono::Utc::now();

        reservations
            .iter()
            .filter(|r| r.product_id == product_id && r.expires_at > now)
            .map(|r| r.quantity)
            .sum()
    }

    /// Clean up expired reservations
    fn cleanup_expired_reservations(&self) {
        let mut reservations = self.reservations.write().unwrap();
        let now = chrono::Utc::now();
        let before = reservations.len();

        reservations.retain(|r| r.expires_at > now);

        let removed = before - reservations.len();
        if removed > 0 {
            debug!("Cleaned up {} expired reservations", removed);
        }
    }

    /// Add a product (for testing/admin)
    pub fn add_product(&self, product: Product) {
        let mut products = self.products.write().unwrap();
        products.insert(product.id.clone(), product);
    }

    /// Update inventory quantity
    pub fn update_inventory(&self, product_id: &str, quantity: i32) -> Result<(), ServiceError> {
        let mut products = self.products.write().unwrap();

        let product = products
            .get_mut(product_id)
            .ok_or_else(|| ServiceError::NotFound(format!("Product {} not found", product_id)))?;

        product.inventory_quantity = quantity;
        info!("Updated inventory for {}: {}", product_id, quantity);

        Ok(())
    }

    /// Update product price
    pub fn update_price(&self, product_id: &str, price: i64) -> Result<(), ServiceError> {
        let mut products = self.products.write().unwrap();

        let product = products
            .get_mut(product_id)
            .ok_or_else(|| ServiceError::NotFound(format!("Product {} not found", product_id)))?;

        let old_price = product.price;
        product.price = price;
        info!("Updated price for {}: {} -> {}", product_id, old_price, price);

        Ok(())
    }
}

impl Default for ProductCatalogService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_product() {
        let catalog = ProductCatalogService::new();
        let product = catalog.get_product("laptop_pro_16_inch").unwrap();

        assert_eq!(product.name, "MacBook Pro 16\"");
        assert_eq!(product.price, 349900);
    }

    #[test]
    fn test_inventory_reservation() {
        let catalog = ProductCatalogService::new();

        // Reserve inventory
        assert!(catalog.reserve_inventory("test", 10, "session_1").is_ok());

        // Check available inventory decreased
        let reserved = catalog.get_reserved_quantity("test");
        assert_eq!(reserved, 10);
    }

    #[test]
    fn test_insufficient_stock() {
        let catalog = ProductCatalogService::new();

        // Try to reserve more than available
        let result = catalog.reserve_inventory("test", 2000, "session_1");
        assert!(result.is_err());
    }

    #[test]
    fn test_commit_inventory() {
        let catalog = ProductCatalogService::new();

        let initial = catalog.get_product("test").unwrap().inventory_quantity;

        // Reserve and commit
        catalog
            .reserve_inventory("test", 10, "session_commit")
            .unwrap();
        catalog.commit_inventory("session_commit").unwrap();

        let final_qty = catalog.get_product("test").unwrap().inventory_quantity;
        assert_eq!(final_qty, initial - 10);
    }
}
