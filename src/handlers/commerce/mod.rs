/// Commerce API handlers module
pub mod products;
pub mod carts;
pub mod checkout;
pub mod customers;

// Re-export route builders
pub use products::products_routes;
pub use carts::carts_routes;
pub use checkout::checkout_routes;
pub use customers::customers_routes; 