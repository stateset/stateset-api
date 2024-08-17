use async_trait::async_trait;;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, Set};
use tracing::{info, error, instrument};
use validator::Validate;
use prometheus::IntCounter;
use crate::{errors::ServiceError, events::{Event, EventSender}, models::order_shipment};
use lazy_static::lazy_static

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

#[async_trait::async_trait]
impl Command for AddTrackingInformationCommand {
    type Result = order_shipment::Model;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DatabaseConnection>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        // Validate the command
        if let Err(e) = self.validate() {
            TRACKING_INFO_ADD_FAILURES.inc();
            error!("Invalid AddTrackingInformationCommand: {}", e);
            return Err(ServiceError::ValidationError(e.to_string()));
        }

        // Transaction to add or update tracking information
        let txn = db_pool.begin().await.map_err(|e| {
            TRACKING_INFO_ADD_FAILURES.inc();
            error!("Failed to start transaction: {}", e);
            ServiceError::DatabaseError("Failed to start transaction".into())
        })?;

        let shipment = match order_shipment::Entity::find()
            .filter(order_shipment::Column::OrderId.eq(self.order_id))
            .one(db_pool.as_ref())
            .await
            .map_err(|e| {
                TRACKING_INFO_ADD_FAILURES.inc();
                error!("Failed to find shipment for order {}: {}", self.order_id, e);
                ServiceError::DatabaseError
            })? {
            Some(mut existing_shipment) => {
                existing_shipment.tracking_number = Set(self.tracking_number.clone());
                existing_shipment.carrier = Set(self.carrier.clone());
                existing_shipment.expected_delivery_date = Set(self.expected_delivery_date);
                existing_shipment.updated_at = Set(chrono::Utc::now().naive_utc());

                existing_shipment.update(db_pool.as_ref()).await.map_err(|e| {
                    TRACKING_INFO_ADD_FAILURES.inc();
                    error!("Failed to update shipment for order {}: {}", self.order_id, e);
                    ServiceError::DatabaseError
                })?
            }
            None => {
                let new_shipment = order_shipment::ActiveModel {
                    id: Set(0), // Assuming ID is auto-incremented
                    order_id: Set(self.order_id),
                    tracking_number: Set(self.tracking_number.clone()),
                    carrier: Set(self.carrier.clone()),
                    expected_delivery_date: Set(self.expected_delivery_date),
                    created_at: Set(chrono::Utc::now().naive_utc()),
                    updated_at: Set(chrono::Utc::now().naive_utc()),
                    ..Default::default()
                };

                new_shipment.insert(db_pool.as_ref()).await.map_err(|e| {
                    TRACKING_INFO_ADD_FAILURES.inc();
                    error!("Failed to create new shipment for order {}: {}", self.order_id, e);
                    ServiceError::DatabaseError
                })?
            }
        };

        txn.commit().await.map_err(|e| {
            TRACKING_INFO_ADD_FAILURES.inc();
            error!("Failed to commit transaction: {}", e);
            ServiceError::DatabaseError("Failed to commit transaction".into())
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
