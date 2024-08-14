use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::OrderShipment};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use prometheus::IntCounter;

lazy_static! {
    static ref TRACKING_INFO_ADDED: IntCounter = 
        IntCounter::new("tracking_info_added_total", "Total number of tracking information added to orders")
            .expect("metric can be created");

    static ref TRACKING_INFO_ADD_FAILURES: IntCounter = 
        IntCounter::new("tracking_info_add_failures_total", "Total number of failed tracking information additions to orders")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AddTrackingInformationCommand {
    pub order_id: i32,
    #[validate(length(min = 1, max = 100))]
    pub tracking_number: String,
    #[validate(length(min = 1, max = 100))]
    pub carrier: String,
    pub expected_delivery_date: Option<chrono::NaiveDate>,
}

#[async_trait]
impl Command for AddTrackingInformationCommand {
    type Result = OrderShipment;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        // Validate the command
        if let Err(e) = self.validate() {
            TRACKING_INFO_ADD_FAILURES.inc();
            error!("Invalid AddTrackingInformationCommand: {}", e);
            return Err(ServiceError::ValidationError(e.to_string()));
        }

        let conn = db_pool.get().map_err(|e| {
            TRACKING_INFO_ADD_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        // Create a new OrderShipment or update existing one
        let shipment = conn.transaction::<_, diesel::result::Error, _>(|| {
            // Check if a shipment already exists for this order
            let existing_shipment = order_shipments::table
                .filter(order_shipments::order_id.eq(self.order_id))
                .first::<OrderShipment>(&conn)
                .optional()?;

            if let Some(mut shipment) = existing_shipment {
                // Update existing shipment
                shipment.tracking_number = self.tracking_number.clone();
                shipment.carrier = self.carrier.clone();
                shipment.expected_delivery_date = self.expected_delivery_date;
                shipment.updated_at = chrono::Utc::now().naive_utc();

                diesel::update(order_shipments::table)
                    .filter(order_shipments::id.eq(shipment.id))
                    .set(&shipment)
                    .execute(&conn)?;

                Ok(shipment)
            } else {
                // Create new shipment
                let new_shipment = OrderShipment {
                    id: 0, // This will be set by the database
                    order_id: self.order_id,
                    tracking_number: self.tracking_number.clone(),
                    carrier: self.carrier.clone(),
                    expected_delivery_date: self.expected_delivery_date,
                    created_at: chrono::Utc::now().naive_utc(),
                    updated_at: chrono::Utc::now().naive_utc(),
                };

                diesel::insert_into(order_shipments::table)
                    .values(&new_shipment)
                    .get_result::<OrderShipment>(&conn)
            }
        }).map_err(|e| {
            TRACKING_INFO_ADD_FAILURES.inc();
            error!("Failed to add/update tracking information for order {}: {}", self.order_id, e);
            ServiceError::DatabaseError
        })?;

        // Trigger an event indicating that tracking information was added/updated
        if let Err(e) = event_sender.send(Event::TrackingInformationUpdated(self.order_id, shipment.id)).await {
            TRACKING_INFO_ADD_FAILURES.inc();
            error!("Failed to send TrackingInformationUpdated event for order {}: {}", self.order_id, e);
            return Err(ServiceError::EventError(e.to_string()));
        }

        TRACKING_INFO_ADDED.inc();

        info!(
            order_id = %self.order_id,
            tracking_number = %self.tracking_number,
            carrier = %self.carrier,
            "Tracking information added/updated successfully"
        );

        Ok(shipment)
    }
}