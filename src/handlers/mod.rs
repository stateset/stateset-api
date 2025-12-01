// Re-enabling all handler modules after implementing them
pub mod analytics;
pub mod asn;
pub mod auth;
pub mod bom;
pub mod cash_sales;
pub mod common;
pub mod customers;
pub mod inventory;
pub mod notifications;
pub mod orders;
pub mod payment_webhooks;
pub mod payments;
pub mod purchase_orders;
pub mod returns;
pub mod shipments;
pub mod warranties;
pub mod work_orders;
// pub mod reports; // Disabled due to missing service dependencies
// pub mod suppliers; // Disabled due to missing service dependencies
pub mod agents;
pub mod commerce;
pub mod outbox_admin;
pub mod users;
pub mod manufacturing;

use crate::events::EventSender;
use crate::message_queue::MessageQueue;
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
    pub payments: Arc<crate::services::payments::PaymentService>,
    pub invoicing: Arc<crate::services::invoicing::InvoicingService>,
    pub cash_sales: Arc<crate::services::cash_sale::CashSaleService>,
    pub returns: Arc<crate::services::returns::ReturnService>,
    pub shipments: Arc<crate::services::shipments::ShipmentService>,
    pub warranties: Arc<crate::services::warranties::WarrantyService>,
    pub bill_of_materials: Arc<crate::services::billofmaterials::BillOfMaterialsService>,
    pub procurement: Arc<crate::services::procurement::ProcurementService>,
    pub asn: Arc<crate::services::asn::ASNService>,
    pub work_orders: Arc<crate::services::work_orders::WorkOrderService>,
    // pub reports: Arc<crate::services::reports::ReportService>,
}

impl AppServices {
    /// Build a default AppServices container with in-memory queue and basic logger.
    pub fn new(
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
        redis_client: Arc<redis::Client>,
        auth_service: Arc<crate::auth::AuthService>,
        message_queue: Arc<dyn MessageQueue>,
        base_logger: Logger,
    ) -> Self {
        let circuit_breaker = Arc::new(CircuitBreaker::new(5, Duration::from_secs(60), 2));
        let circuit_breaker_registry = Arc::new(CircuitBreakerRegistry::new(None));
        let returns_logger = base_logger.new(slog::o!("component" => "returns_service"));
        let shipments_logger = base_logger.new(slog::o!("component" => "shipments_service"));
        let warranties_logger = base_logger.new(slog::o!("component" => "warranties_service"));
        let procurement_logger = base_logger.new(slog::o!("component" => "procurement_service"));
        let asn_logger = base_logger.new(slog::o!("component" => "asn_service"));

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
        let payments = Arc::new(crate::services::payments::PaymentService::new(
            db_pool.clone(),
            event_sender.clone(),
        ));
        let invoicing = Arc::new(crate::services::invoicing::InvoicingService::new(
            db_pool.clone(),
        ));
        let cash_sales = Arc::new(crate::services::cash_sale::CashSaleService::new(
            db_pool.clone(),
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
        let agentic_checkout = Arc::new(crate::services::commerce::AgenticCheckoutService::new(
            db_pool.clone(),
            cache.clone(),
            event_sender.clone(),
            product_catalog.clone(),
            order_service.clone(),
            payments.clone(),
            shipments.clone(),
            invoicing.clone(),
            cash_sales.clone(),
        ));
        let bill_of_materials = Arc::new(
            crate::services::billofmaterials::BillOfMaterialsService::new(
                db_pool.clone(),
                event_sender.clone(),
            ),
        );
        let returns = Arc::new(crate::services::returns::ReturnService::new(
            db_pool.clone(),
            event_sender.clone(),
            redis_client.clone(),
            message_queue.clone(),
            circuit_breaker.clone(),
            returns_logger,
        ));
        let warranties = Arc::new(crate::services::warranties::WarrantyService::new(
            db_pool.clone(),
            event_sender.clone(),
            redis_client.clone(),
            message_queue.clone(),
            circuit_breaker.clone(),
            warranties_logger,
        ));
        let procurement = Arc::new(crate::services::procurement::ProcurementService::new(
            db_pool.clone(),
            event_sender.clone(),
            redis_client.clone(),
            message_queue.clone(),
            circuit_breaker.clone(),
            procurement_logger,
        ));
        let asn = Arc::new(crate::services::asn::ASNService::new(
            db_pool.clone(),
            event_sender.clone(),
            redis_client.clone(),
            message_queue.clone(),
            circuit_breaker.clone(),
            asn_logger,
        ));
        let work_orders_logger = base_logger.new(slog::o!("component" => "work_orders_service"));
        let work_orders = Arc::new(crate::services::work_orders::WorkOrderService::new(
            db_pool.clone(),
            event_sender.clone(),
            redis_client.clone(),
            message_queue.clone(),
            circuit_breaker.clone(),
            work_orders_logger,
        ));

        Self {
            product_catalog,
            cart,
            checkout,
            agentic_checkout,
            customer,
            order: order_service,
            payments,
            invoicing,
            cash_sales,
            returns,
            shipments,
            warranties,
            bill_of_materials,
            procurement,
            asn,
            work_orders,
        }
    }
}

// Note: AppState is defined in main.rs and re-exported from lib.rs

// Common utility functions are in the separate common.rs file
