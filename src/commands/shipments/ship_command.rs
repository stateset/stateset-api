use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Shipment, ShipmentStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::info;
use chrono::{DateTime, Utc};


#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ShipOrderCommand {
    pub order_id: i32,
    #[validate(length(min = 1))]
    pub tracking_number: String, // Shipment tracking number
}

#[async_trait]
impl Command for ShipOrderCommand {
    type Result = Shipment;

    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        // Finalize the shipment
        let shipment = Shipment {
            order_id: self.order_id,
            tracking_number: self.tracking_number.clone(),
            status: ShipmentStatus::Shipped,
            // Other fields like carrier information, shipped date, etc.
        };

        let saved_shipment = diesel::insert_into(shipments::table)
            .values(&shipment)
            .get_result::<Shipment>(&conn)
            .map_err(|e| ServiceError::DatabaseError)?;

        // Update order status to "Shipped"
        diesel::update(orders::table.find(self.order_id))
            .set(orders::status.eq(OrderStatus::Shipped))
            .execute(&conn)
            .map_err(|e| ServiceError::DatabaseError)?;

        // Log and trigger events
        info!("Order ID: {} shipped with tracking number: {}", self.order_id, self.tracking_number);
        event_sender.send(Event::OrderShipped(self.order_id, self.tracking_number.clone())).await.map_err(|e| ServiceError::EventError(e.to_string()))?;

        Ok(saved_shipment)
    }
}