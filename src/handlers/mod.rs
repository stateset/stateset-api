// Re-enabling all handler modules after implementing them
pub mod auth;
pub mod common;
pub mod orders;
pub mod inventory;
pub mod returns;
pub mod shipments;
pub mod warranties;
pub mod work_orders;
// TODO: Enable these modules once they are implemented
// pub mod asn;
// pub mod bom;
// pub mod cash_sales;
// pub mod customers;
// pub mod notifications;
// pub mod purchase_orders;
// pub mod reports;
// pub mod suppliers;
// pub mod users;
// pub mod commerce;

use crate::events::EventSender;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

/// Services layer that encapsulates business logic
#[derive(Debug, Clone)]
pub struct AppServices {
    // TODO: Add services after fixing module dependencies
}

impl AppServices {
    pub fn new(_db_pool: Arc<DatabaseConnection>, _event_sender: Arc<EventSender>) -> Self {
        Self {
            // TODO: Initialize services after fixing module dependencies
        }
    }
}

// Note: AppState is defined in main.rs and re-exported from lib.rs

// Common utility functions are in the separate common.rs file
