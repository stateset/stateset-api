// Core models
pub mod account;
pub mod bill_of_materials;
pub mod customer;
pub mod inventory_entity;
pub mod order;
pub mod order_entity;
pub mod order_item_entity;
pub mod order_note_entity;
pub mod order_shipment_entity;
pub mod order_tag;
pub mod r#return;
pub mod return_entity;
pub mod return_history_entity;
pub mod return_item_entity;
pub mod return_line_item;
pub mod return_note_entity;
pub mod returned_item_entity;
pub mod shipment;
pub mod shipment_event;
pub mod shipment_item;
pub mod shipment_note;
pub mod supplier;
pub mod supplier_contact;
pub mod suppliers;
pub mod warranty;
pub mod work_order;
pub mod work_order_note_entity;
pub mod work_order_task_entity;

// Advanced Shipping Notice entities
pub mod asn_entity;
pub mod asn_item_entity;
pub mod asn_items;
pub mod asn_note_entity;

// Inventory related models
pub mod incoming_inventory_entity;
pub mod inventory_allocation_entity;
pub mod inventory_forecasts;
pub mod inventory_item_entity;
pub mod inventory_items;
pub mod inventory_level_entity;
pub mod inventory_reservation_entity;
pub mod inventory_snapshot;
pub mod inventory_transaction_entity;
pub mod manufacture_orders;
pub mod picks;
pub mod safety_stock_alert_entity;
pub mod safety_stock_entity;
pub mod warehouse;
pub mod warehouse_location_entity;
pub mod waste_and_scrap;

// Line items and components
pub mod bom_line_item;
pub mod manufacture_order_line_item;
pub mod order_line_item;
pub mod part;
pub mod pick_item;
pub mod warranty_line_item;
pub mod work_order_line_item;

// Finance related models
pub mod accounts;
pub mod agreement_line_item;
pub mod agreements;
pub mod cash_sale;
pub mod cog_entries;
pub mod contacts;
pub mod cycle_count_line_item;
pub mod cyclecounts;
pub mod exchange_rate;
pub mod fulfillment_order;
pub mod incidents;
pub mod invoice_line_item;
pub mod invoices;
pub mod item_receipt;
pub mod payment;
pub mod product;
pub mod product_category;
pub mod product_entity;
pub mod reconcile_line_item;
pub mod reconciles;
pub mod sale_transaction;

// Manufacturing related
pub mod billofmaterials;
pub mod machine;
pub mod maintenance_record;
pub mod manufacturing_cost_entity;

// Facility and Logistics
pub mod facility_entity;

// Procurement
pub mod purchase_order_entity;
pub mod purchase_order_item_entity;

// Marketing and Promotions
pub mod promotion;
pub mod promotion_entity;

// Re-export common types for convenience
pub use asn_entity::ASNStatus;
pub use asn_item_entity::ASNItemStatus;
pub use asn_note_entity::ASNNoteType;
pub use inventory_allocation_entity::AllocationStatus;
pub use inventory_reservation_entity::ReservationStatus;
pub use inventory_reservation_entity::ReservationType;
pub use inventory_transaction_entity::InventoryTransactionType;
pub use order::OrderStatus;
pub use order_item_entity::OrderItemStatus;
pub use safety_stock_alert_entity::AlertStatus;
pub use shipment::ShipmentStatus;
pub use warranty::WarrantyStatus;
pub use work_order::WorkOrderPriority;
pub use work_order::WorkOrderStatus;

// Convenient aliases for legacy imports
pub use asn_entity as asn;
pub use billofmaterials as bom;
pub use bom_line_item as bom_component;

// Type re-exports for compatibility
pub use asn_entity::ActiveModel as NewASN;
pub use asn_entity::Model as ASN;
pub use asn_item_entity::ActiveModel as NewASNLineItem;
pub use asn_item_entity::Model as ASNLineItem;
pub use billofmaterials::ActiveModel as NewBOM;
pub use billofmaterials::Model as BOM;
pub use bom_line_item::ActiveModel as NewBOMComponent;
pub use bom_line_item::Model as BOMComponent;
pub use promotion_entity::PromotionStatus;
pub use purchase_order_entity::PurchaseOrderStatus;
pub use shipment::Model as Shipment;
pub use shipment::ShippingCarrier as ShippingMethod;
pub use shipment_note::NewShipmentNote;
pub use work_order as work_order_entity;
pub use work_order::ActiveModel as NewWorkOrder;
pub use work_order_note_entity::NewWorkOrderNote;

// Module aliases for incorrect imports
pub use asn_item_entity as asn_line_item;
pub use billofmaterials as bill_of_materials_entity;
pub use bom_line_item as bom_item_entity;
pub use cog_entries as cogs_data_entity;
pub use inventory_transaction_entity as inventory_movement_entity;
pub use manufacture_orders as manufacturing_order_entity;
pub use purchase_order_entity as purchase_order;

// Create a stub asn_package module since it's referenced but doesn't exist
pub mod asn_package {
    pub use super::asn_item_entity::ActiveModel;
    pub use super::asn_item_entity::ActiveModel as NewASNPackage;
    pub use super::asn_item_entity::Model as ASNPackage;
}

// Entity alias for compatibility with existing code
pub use asn_package as asn_package_entity;

// Work order material entity (alias for work_order_line_item)
pub mod work_order_material_entity {
    pub use super::work_order_line_item::*;
}

// Manufacture order operation entity (alias for manufacture_order_line_item)
pub mod manufacture_order_operation_entity {
    pub use super::manufacture_order_line_item::*;
}

// Shipment item entity (alias for shipment_item)
pub use shipment_item as shipment_item_entity;

// Tracking event entity (alias for shipment_event)
pub mod tracking_event_entity {
    pub use super::shipment_event::*;
}

// Type aliases for shipment queries
pub type ShipmentItem = shipment_item::Model;
pub type TrackingEvent = shipment_event::Model;

// Export a prelude module with common entity types
pub mod prelude {
    // Core entities
    pub use super::cash_sale::Entity as CashSale;
    pub use super::fulfillment_order::Entity as FulfillmentOrder;
    pub use super::order_entity::Entity as Order;
    pub use super::order_item_entity::Entity as OrderItem;
    pub use super::order_note_entity::Entity as OrderNote;
    pub use super::return_entity::Entity as Return;
    pub use super::shipment::Entity as Shipment;
    pub use super::warranty::Entity as Warranty;
    pub use super::work_order::Entity as WorkOrder;

    // ASN entities
    pub use super::asn_entity::Entity as ASN;
    pub use super::asn_item_entity::Entity as ASNItem;
    pub use super::asn_note_entity::Entity as ASNNote;

    // Inventory entities
    pub use super::inventory_allocation_entity::Entity as InventoryAllocation;
    pub use super::inventory_level_entity::Entity as InventoryLevel;
    pub use super::inventory_reservation_entity::Entity as InventoryReservation;
    pub use super::inventory_transaction_entity::Entity as InventoryTransaction;
    pub use super::safety_stock_alert_entity::Entity as SafetyStockAlert;
    pub use super::safety_stock_entity::Entity as SafetyStock;

    // Common statuses and types
    pub use super::asn_entity::ASNStatus;
    pub use super::asn_item_entity::ASNItemStatus;
    pub use super::fulfillment_order::FulfillmentOrderStatus;
    pub use super::inventory_allocation_entity::AllocationStatus;
    pub use super::inventory_reservation_entity::ReservationStatus;
    pub use super::inventory_reservation_entity::ReservationType;
    pub use super::inventory_transaction_entity::InventoryTransactionType;
    pub use super::order::OrderStatus;
    pub use super::safety_stock_alert_entity::AlertStatus;
    pub use super::shipment::ShipmentStatus;
    pub use super::warranty::WarrantyStatus;
    pub use super::work_order::WorkOrderPriority;
    pub use super::work_order::WorkOrderStatus;
}
