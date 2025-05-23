// Core services
pub mod orders;
pub mod inventory;
pub mod returns;
pub mod shipments;
pub mod fulfillment_orders;
pub mod warranties;
pub mod work_orders;

// Manufacturing and Supply Chain
pub mod billofmaterials;
pub mod suppliers;
pub mod procurement;

// Customer Management
pub mod customers;
pub mod leads;
pub mod accounts;

// Financial Services
pub mod invoicing;
pub mod payments;
pub mod cash_sale;
pub mod accounting;

// Analytics and Reporting
pub mod business_intelligence;
pub mod forecasting;
pub mod reports;

// Legacy module aliases for backwards compatibility
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
pub mod category_service;