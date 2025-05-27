// Core models
pub mod shipment;
pub mod work_order;
pub mod warranty;
pub mod customer;
pub mod order;
pub mod order_entity;
pub mod order_note_entity;
pub mod order_item_entity;
pub mod return_entity;
pub mod supplier;
pub mod return;
pub mod suppliers;

// Advanced Shipping Notice entities
pub mod asn_entity;
pub mod asn_item_entity;
pub mod asn_note_entity;
pub mod asn_items;

// Inventory related models
pub mod inventory_items;
pub mod inventory_forecasts;
pub mod inventory_level_entity;
pub mod inventory_transaction_entity;
pub mod inventory_allocation_entity;
pub mod inventory_reservation_entity;
pub mod safety_stock_entity;
pub mod safety_stock_alert_entity;
pub mod waste_and_scrap;
pub mod picks;
pub mod manufacture_orders;

// Line items and components
pub mod order_line_item;
pub mod work_order_line_item;
pub mod warranty_line_item;
pub mod manufacture_order_line_item;
pub mod bom_line_item;
pub mod pick_item;
pub mod part;

// Finance related models
pub mod accounts;
pub mod agreements;
pub mod cog_entries;
pub mod contacts;
pub mod cyclecounts;
pub mod incidents;
pub mod invoices;
pub mod product_category;
pub mod reconciles;
pub mod cash_sale;
pub mod fulfillment_order;
pub mod item_receipt;
pub mod payment;

// Manufacturing related
pub mod machine;
pub mod maintenance_record;
pub mod billofmaterials;

// Re-export common types for convenience
pub use order::OrderStatus;
pub use shipment::ShipmentStatus;
pub use warranty::WarrantyStatus;
pub use work_order::WorkOrderStatus;
pub use work_order::WorkOrderPriority;
pub use asn_entity::ASNStatus;
pub use asn_item_entity::ASNItemStatus;
pub use asn_note_entity::ASNNoteType;
pub use inventory_transaction_entity::InventoryTransactionType;
pub use inventory_allocation_entity::AllocationStatus;
pub use inventory_reservation_entity::ReservationStatus;
pub use inventory_reservation_entity::ReservationType;
pub use safety_stock_alert_entity::AlertStatus;

// Export a prelude module with common entity types
pub mod prelude {
    // Core entities
    pub use super::order_entity::Entity as Order;
    pub use super::order_item_entity::Entity as OrderItem;
    pub use super::order_note_entity::Entity as OrderNote;
    pub use super::return_entity::Entity as Return;
    pub use super::shipment::Entity as Shipment;
    pub use super::warranty::Entity as Warranty;
    pub use super::work_order::Entity as WorkOrder;
    pub use super::cash_sale::Entity as CashSale;
    pub use super::fulfillment_order::Entity as FulfillmentOrder;
    
    // ASN entities
    pub use super::asn_entity::Entity as ASN;
    pub use super::asn_item_entity::Entity as ASNItem;
    pub use super::asn_note_entity::Entity as ASNNote;
    
    // Inventory entities
    pub use super::inventory_level_entity::Entity as InventoryLevel;
    pub use super::inventory_transaction_entity::Entity as InventoryTransaction;
    pub use super::inventory_allocation_entity::Entity as InventoryAllocation;
    pub use super::inventory_reservation_entity::Entity as InventoryReservation;
    pub use super::safety_stock_entity::Entity as SafetyStock;
    pub use super::safety_stock_alert_entity::Entity as SafetyStockAlert;
    
    // Common statuses and types
    pub use super::order::OrderStatus;
    pub use super::asn_entity::ASNStatus;
    pub use super::asn_item_entity::ASNItemStatus;
    pub use super::inventory_transaction_entity::InventoryTransactionType;
    pub use super::inventory_allocation_entity::AllocationStatus;
    pub use super::inventory_reservation_entity::ReservationStatus;
    pub use super::inventory_reservation_entity::ReservationType;
    pub use super::safety_stock_alert_entity::AlertStatus;
    pub use super::shipment::ShipmentStatus;
    pub use super::warranty::WarrantyStatus;
    pub use super::work_order::WorkOrderStatus;
    pub use super::work_order::WorkOrderPriority;
    pub use super::fulfillment_order::FulfillmentOrderStatus;
}