// Core services
// Temporarily commented out services that depend on models module
// pub mod fulfillment_orders;
pub mod inventory;
pub mod orders;
pub mod returns;
pub mod shipments;
pub mod warranties;
pub mod work_orders;

// Simple status helpers that work directly with entities
pub mod order_status;

// Service factory for dependency injection
pub mod factory;

// Inventory management services
pub mod bom;
pub mod inventory_adjustment_service;
pub mod inventory_sync;
pub mod manufacturing;
pub mod purchase_receipt;
pub mod sales_fulfillment;

// Manufacturing and Supply Chain
// pub mod billofmaterials;
// pub mod asn; // Disabled due to missing handler dependencies
pub mod procurement;
// pub mod suppliers;

// Customer Management
pub mod accounts;
// pub mod customers;
pub mod leads;
pub mod notifications;

// Financial Services
// pub mod accounting;
pub mod cash_sale;
// pub mod invoicing;
// pub mod item_receipts;
pub mod payments;

// Analytics and Reporting
pub mod business_intelligence;
pub mod forecasting;
// pub mod reports;

// External Services
pub mod geocoding;

// Legacy module aliases for backwards compatibility
// Commented out due to depending on disabled modules
/*
pub mod order_service {
    pub use super::orders::OrderService;
}
pub mod inventory_service {
    pub use super::inventory::InventoryService;
}
pub mod return_service {
    pub use super::returns::ReturnService;
}
pub mod warranty_service {
    pub use super::warranties::WarrantyService;
}
pub mod shipment_service {
    pub use super::shipments::ShipmentService;
}
pub mod work_order_service {
    pub use super::work_orders::WorkOrderService;
}
pub mod fulfillment_order_service {
    pub use super::fulfillment_orders::FulfillmentOrderService;
}
pub mod item_receipt_service {
    pub use super::item_receipts::ItemReceiptService;
}
*/
// pub mod category_service;
pub mod analytics;
pub mod commerce;
