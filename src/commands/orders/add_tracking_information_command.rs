use uuid::Uuid;
use crate::commands::Command;
use crate::events::{Event, EventSender};
use crate::{
    db::DbPool,
    errors::ServiceError,
    models::{order_shipment_entity, order_shipment_entity::Entity as OrderShipment},
};
use async_trait::async_trait;
use chrono::Utc;
use lazy_static::lazy_static;
use prometheus::IntCounter;
use sea_orm::{*, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

lazy_static! {
    static ref TRACKING_INFO_ADDED: IntCounter = IntCounter::new(
        "tracking_info_added_total",
        "Total number of tracking information added to orders"
    )
    .expect("metric can be created");
    static ref TRACKING_INFO_ADD_FAILURES: IntCounter = IntCounter::new(
        "tracking_info_add_failures_total",
        "Total number of failed tracking information additions to orders"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AddTrackingInformationCommand {
    pub order_id: Uuid,
    #[validate(length(min = 1, max = 100))]
    pub tracking_number: String,
    #[validate(length(min = 1, max = 100))]
    pub carrier: String,
    pub expected_delivery_date: Option<chrono::NaiveDate>,
}

#[async_trait]
impl Command for AddTrackingInformationCommand {
    type Result = order_shipment_entity::Model;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        // Validate the command
        if let Err(e) = self.validate() {
            TRACKING_INFO_ADD_FAILURES.inc();
            error!("Invalid AddTrackingInformationCommand: {}", e);
            return Err(ServiceError::ValidationError(e.to_string()));
        }

        let db = db_pool;

        // Create a new OrderShipment or update existing one
        let shipment = db
            .transaction::<_, order_shipment_entity::Model, ServiceError>(|txn| {
                Box::pin(async move {
                    // Check if a shipment already exists for this order
                    let existing_shipment = OrderShipment::find()
                        .filter(order_shipment_entity::Column::OrderId.eq(self.order_id))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            error!("Database error: {}", e);
                        })?;

                    let shipment = if let Some(existing) = existing_shipment {
                        // Update existing shipment
                        let mut shipment: order_shipment_entity::ActiveModel = existing.into();
                        shipment.tracking_number = Set(self.tracking_number.clone());
                        shipment.carrier = Set(self.carrier.clone());
                        shipment.expected_delivery_date = Set(self.expected_delivery_date);
                        shipment.updated_at = Set(Utc::now().naive_utc());

                        shipment.update(txn).await.map_err(|e| {
                            error!("Failed to update shipment: {}", e);
                        })?
                    } else {
                        // Create new shipment
                        let new_shipment = order_shipment_entity::ActiveModel {
                            order_id: Set(self.order_id),
                            tracking_number: Set(self.tracking_number.clone()),
                            carrier: Set(self.carrier.clone()),
                            expected_delivery_date: Set(self.expected_delivery_date),
                            created_at: Set(Utc::now().naive_utc()),
                            updated_at: Set(Utc::now().naive_utc()),
                            ..Default::default()
                        };

                        new_shipment.insert(txn).await.map_err(|e| {
                            error!("Failed to insert new shipment: {}", e);
                        })?
                    };

                    Ok(shipment)
                })
            })
            .await
            .map_err(|e| {
                TRACKING_INFO_ADD_FAILURES.inc();
                error!(
                    "Failed to add/update tracking information for order {}: {}",
                    self.order_id, e
                );
                e
            })?;

        // Trigger an event indicating that tracking information was added/updated
        if let Err(e) = event_sender
            .send(Event::TrackingInformationUpdated(
                self.order_id,
                shipment.id,
            ))
            .await
        {
            TRACKING_INFO_ADD_FAILURES.inc();
            error!(
                "Failed to send TrackingInformationUpdated event for order {}: {}",
                self.order_id, e
            );
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
