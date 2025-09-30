// Re-enabling all handler modules after implementing them
pub mod auth;
pub mod common;
pub mod orders;
pub mod inventory;
pub mod returns;
pub mod shipments;
pub mod warranties;
pub mod work_orders;
// pub mod asn; // File removed due to compilation issues
// pub mod bom; // Disabled due to missing service dependencies
pub mod cash_sales;
pub mod customers;
pub mod payments;
pub mod payment_webhooks;
pub mod notifications;
// pub mod purchase_orders; // Disabled due to missing service dependencies
// pub mod reports; // Disabled due to missing service dependencies
// pub mod suppliers; // Disabled due to missing service dependencies
pub mod users;
pub mod commerce;
pub mod agents;
pub mod outbox_admin;

use crate::events::EventSender;
use crate::message_queue::{InMemoryMessageQueue, MessageQueue};
use crate::{circuit_breaker::CircuitBreaker, db::DbPool};
use sea_orm::DatabaseConnection;
use slog::Logger;
use std::sync::Arc;
use std::time::Duration;

// Re-export AppState so handler modules can import it as crate::handlers::AppState
pub use crate::AppState;

/// Services layer that encapsulates business logic used by HTTP handlers
#[derive(Clone)]
pub struct AppServices {
    pub product_catalog: Arc<crate::services::commerce::ProductCatalogService>,
    pub cart: Arc<crate::services::commerce::CartService>,
    pub checkout: Arc<crate::services::commerce::CheckoutService>,
    pub agentic_checkout: Arc<crate::services::commerce::AgenticCheckoutService>,
    pub customer: Arc<crate::services::commerce::CustomerService>,
    pub order: Arc<crate::services::orders::OrderService>,
    // pub cash_sales: Arc<crate::services::cash_sale::CashSaleService>,
    // pub reports: Arc<crate::services::reports::ReportService>,
}

impl AppServices {
    /// Build a default AppServices container with in-memory queue and basic logger.
    pub fn new(
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
        _redis_client: Arc<redis::Client>,
        auth_service: Arc<crate::auth::AuthService>,
    ) -> Self {
        let message_queue: Arc<dyn MessageQueue> = Arc::new(InMemoryMessageQueue::new());
        let circuit_breaker = Arc::new(CircuitBreaker::new(5, Duration::from_secs(60), 2));
        let logger = Logger::root(slog::Discard, slog::o!());

        let product_catalog = Arc::new(crate::services::commerce::ProductCatalogService::new(
            db_pool.clone(),
            event_sender.clone(),
        ));
        let cart = Arc::new(crate::services::commerce::CartService::new(
            db_pool.clone(),
            event_sender.clone(),
        ));
        let order_service = Arc::new(crate::services::orders::OrderService::new(
            db_pool.clone(),
            Some(event_sender.clone()),
        ));
        let checkout = Arc::new(crate::services::commerce::CheckoutService::new(
            db_pool.clone(),
            event_sender.clone(),
            order_service.clone(),
        ));
        let customer = Arc::new(crate::services::commerce::CustomerService::new(
            db_pool.clone(),
            event_sender.clone(),
            auth_service,
        ));

        let cache = Arc::new(crate::cache::InMemoryCache::new());
        let agentic_checkout = Arc::new(crate::services::commerce::AgenticCheckoutService::new(
            db_pool.clone(),
            cache,
            event_sender.clone(),
        ));

        Self { 
            product_catalog, 
            cart, 
            checkout, 
            agentic_checkout,
            customer, 
            order: order_service 
        }
    }
}

// Note: AppState is defined in main.rs and re-exported from lib.rs

// Common utility functions are in the separate common.rs file
pub mod analytics;
