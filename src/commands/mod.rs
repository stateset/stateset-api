use crate::{db::DbPool, errors::ServiceError, events::EventSender};
use async_trait::async_trait;
use std::sync::Arc;

/// Command trait for implementing the Command Pattern
///
/// This trait allows for encapsulating all the logic needed to execute a business operation
/// into a single object that can be validated, executed, and produce events.
#[async_trait]
pub trait Command: Send + Sync {
    /// The return type of the command when executed successfully
    type Result;

    /// Execute the command with the given dependencies
    ///
    /// # Arguments
    /// * `db_pool` - Database connection pool for persistence operations
    /// * `event_sender` - Channel to publish domain events
    ///
    /// # Returns
    /// * `Result<Self::Result, ServiceError>` - The result of command execution or an error
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError>;
}

pub mod orders;
// Other command modules are temporarily disabled to reduce compile surface
// pub mod advancedshippingnotice;
// pub mod billofmaterials;
// pub mod inventory;
// pub mod purchaseorders;
// pub mod returns;
// pub mod shipments;
// pub mod warranties;
// pub mod workorders;
// pub mod picking;
// pub mod receiving;
// pub mod analytics;
// pub mod audit;
// pub mod carriers;
// pub mod customers;
// pub mod forecasting;
// pub mod kitting;
// pub mod maintenance;
// pub mod packaging;
// pub mod payments;
// pub mod quality;
// pub mod suppliers;
// pub mod transfers;
// pub mod warehouses;
