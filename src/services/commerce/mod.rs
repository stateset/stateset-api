/// Commerce services module - Core eCommerce business logic
pub mod product_catalog_service;
pub mod cart_service;
pub mod checkout_service;
pub mod agentic_checkout;
pub mod customer_service;
pub mod pricing_service;

// Re-export services for convenience
pub use product_catalog_service::ProductCatalogService;
pub use cart_service::CartService;
pub use checkout_service::CheckoutService;
pub use agentic_checkout::AgenticCheckoutService;
pub use customer_service::CustomerService; 