// Re-enabling all handler modules after implementing them
pub mod auth;
pub mod common;
pub mod inventory;
pub mod orders;
pub mod returns;
pub mod shipments;
pub mod warranties;
pub mod work_orders;
// pub mod asn; // File removed due to compilation issues
// pub mod bom; // Disabled due to missing service dependencies
pub mod cash_sales;
pub mod customers;
pub mod notifications;
pub mod payment_webhooks;
pub mod payments;
// pub mod purchase_orders; // Disabled due to missing service dependencies
// pub mod reports; // Disabled due to missing service dependencies
// pub mod suppliers; // Disabled due to missing service dependencies
pub mod agents;
pub mod commerce;
pub mod outbox_admin;
pub mod users;

use crate::events::EventSender;
use crate::message_queue::{InMemoryMessageQueue, MessageQueue};
use crate::{
    circuit_breaker::{CircuitBreaker, CircuitBreakerRegistry},
    db::DbPool,
};
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
    pub cash_sales: Arc<crate::services::cash_sale::CashSaleService>,
    pub returns: Arc<crate::services::returns::ReturnService>,
    pub shipments: Arc<crate::services::shipments::ShipmentService>,
    pub warranties: Arc<crate::services::warranties::WarrantyService>,
    // pub reports: Arc<crate::services::reports::ReportService>,
}

impl AppServices {
    /// Build a default AppServices container with in-memory queue and basic logger.
    pub fn new(
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
        redis_client: Arc<redis::Client>,
        auth_service: Arc<crate::auth::AuthService>,
    ) -> Self {
        let message_queue: Arc<dyn MessageQueue> = Arc::new(InMemoryMessageQueue::new());
        let circuit_breaker = Arc::new(CircuitBreaker::new(5, Duration::from_secs(60), 2));
        let circuit_breaker_registry = Arc::new(CircuitBreakerRegistry::new(None));
        let base_logger = Logger::root(slog::Discard, slog::o!());
        let returns_logger = base_logger.new(slog::o!("component" => "returns_service"));
        let shipments_logger = base_logger.new(slog::o!("component" => "shipments_service"));
        let warranties_logger = base_logger.new(slog::o!("component" => "warranties_service"));

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
        let cash_sales = Arc::new(crate::services::cash_sale::CashSaleService::new(
            db_pool.clone(),
        ));
        let returns = Arc::new(crate::services::returns::ReturnService::new(
            db_pool.clone(),
            event_sender.clone(),
            redis_client.clone(),
            message_queue.clone(),
            circuit_breaker.clone(),
            returns_logger,
        ));
        let shipments = Arc::new(crate::services::shipments::ShipmentService::new(
            db_pool.clone(),
            event_sender.clone(),
            redis_client.clone(),
            message_queue.clone(),
            circuit_breaker.clone(),
            circuit_breaker_registry.clone(),
            shipments_logger,
        ));
        let warranties = Arc::new(crate::services::warranties::WarrantyService::new(
            db_pool.clone(),
            event_sender,
            redis_client,
            message_queue,
            circuit_breaker,
            warranties_logger,
        ));

        Self {
            product_catalog,
            cart,
            checkout,
            agentic_checkout,
            customer,
            order: order_service,
            cash_sales,
            returns,
            shipments,
            warranties,
        }
    }
}

// Note: AppState is defined in main.rs and re-exported from lib.rs

// Common utility functions are in the separate common.rs file
pub mod analytics;
