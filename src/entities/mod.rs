pub mod inventory_reservation;
pub mod inventory_transaction;
pub mod order;
pub mod order_item;
pub mod product; // Main product entity - consolidated from multiple duplicates
pub mod warranty;
pub mod warranty_claim;

// Additional entity modules
// pub mod cycle_count_entity; // Re-export removed - imports from non-existent models module
// pub mod pick_task_entity; // Re-export removed - imports from non-existent models module
pub mod user_entity;
// pub mod warehouse_inventory_entity; // Re-export removed - imports from non-existent models module
pub mod inventory_adjustment_entity;
pub mod inventory_movement_entity;
pub mod incoming_shipment_item_entity;
pub mod commerce;
// pub mod manufacture_orders; // Re-export removed - imports from non-existent models module
// pub mod manufacture_order_line_item; // Re-export removed - imports from non-existent models module
// pub mod work_order; // Re-export removed - use proper entity definition
// pub mod work_order_line_item; // Re-export removed - imports from non-existent models module
// pub mod return_entity; // Re-export removed - use proper entity definition
// order_entity consolidated into order.rs
// order_item_entity consolidated into order_item.rs
pub mod inventory_items;
// pub mod shipment; // Re-export removed - imports from non-existent models module
// pub mod asn_entity; // Re-export removed - use proper entity definition
// pub mod suppliers; // Re-export removed - imports from non-existent models module
// pub mod work_order_task_entity; // Re-export removed - imports from non-existent models module
// pub mod work_order_material_entity; // Re-export removed - imports from non-existent models module

// New ERP entities
pub mod item_master;
pub mod inventory_locations;
pub mod inventory_balances;
pub mod bom_headers;
pub mod bom_lines;
pub mod manufacturing_work_orders;
pub mod sales_order_headers;
pub mod sales_order_lines;
pub mod order_fulfillments;
pub mod purchase_order_headers;
pub mod purchase_order_lines;
pub mod purchase_order_distributions;
pub mod purchase_invoices;
pub mod purchase_invoice_lines;
pub mod po_receipt_headers;
pub mod po_receipt_lines;
// pub mod sales_invoices;
// pub mod sales_invoice_lines;
