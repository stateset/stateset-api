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
pub mod commerce;
pub mod incoming_shipment_item_entity;
pub mod inventory_adjustment_entity;
pub mod inventory_movement_entity;
// pub mod manufacture_orders; // Re-export removed - imports from non-existent models module
// pub mod manufacture_order_line_item; // Re-export removed - imports from non-existent models module
pub mod work_order;
pub mod work_order_note;
// pub mod work_order_line_item; // Re-export removed - imports from non-existent models module
// pub mod return_entity; // Re-export removed - use proper entity definition
// order_entity consolidated into order.rs
// order_item_entity consolidated into order_item.rs
pub mod inventory_items;
pub mod shipment; // Re-export removed - imports from non-existent models module
                  // pub mod asn_entity; // Re-export removed - use proper entity definition
                  // pub mod suppliers; // Re-export removed - imports from non-existent models module
                  // pub mod work_order_task_entity; // Re-export removed - imports from non-existent models module
                  // pub mod work_order_material_entity; // Re-export removed - imports from non-existent models module

// New ERP entities
pub mod bom_header;
pub mod bom_line;
pub mod inventory_balance;
pub mod inventory_location;
pub mod item_master;
pub mod ledger_entry;
pub mod manufacturing;
pub mod manufacturing_work_orders;
pub mod order_fulfillments;
pub mod po_receipt_headers;
pub mod po_receipt_lines;
pub mod purchase_invoice_lines;
pub mod purchase_invoices;
pub mod purchase_order_distributions;
pub mod purchase_order_headers;
pub mod purchase_order_lines;
pub mod sales_invoice;
pub mod sales_invoice_line;
pub mod sales_order_header;
pub mod sales_order_line;
