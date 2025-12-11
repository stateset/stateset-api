// Core services
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
pub mod inventory_reservation_service;
pub mod inventory_sync;
pub mod manufacturing;
pub mod purchase_receipt;
pub mod sales_fulfillment;

// Manufacturing and Supply Chain
pub mod asn;
pub mod billofmaterials;
pub mod procurement;

// Customer Management
pub mod accounts;
pub mod leads;
pub mod notifications;

// Financial Services
pub mod accounting;
pub mod cash_sale;
pub mod invoicing;
pub mod payments;
pub mod promotions;
pub mod stablepay_crypto_service;
pub mod stablepay_reconciliation_service;
pub mod stablepay_service;
pub mod stateset_blockchain_service;

// Analytics and Reporting
pub mod business_intelligence;
pub mod forecasting;
pub mod reports;

// External Services
pub mod geocoding;

// Commerce and Analytics
pub mod analytics;
pub mod commerce;
