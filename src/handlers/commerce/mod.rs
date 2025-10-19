pub mod agentic_checkout;
pub mod carts;
pub mod checkout;
pub mod customers;
/// Commerce API handlers module
pub mod products;

// Re-export route builders
pub use agentic_checkout::agentic_checkout_routes;
pub use carts::carts_routes;
pub use checkout::checkout_routes;
pub use customers::customers_routes;
pub use products::products_routes;
