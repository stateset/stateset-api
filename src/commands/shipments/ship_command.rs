use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Shipment, ShipmentStatus, OrderStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ShipOrderCommand {
    pub order_id: i32,

    #[validate(length(min = 1))]
    pub tracking_number: String, // Shipment tracking number
}

#[async_trait::async_trait]
impl Command for ShipOrderCommand {
    type Result = Shipment;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let saved_shipment = conn.transaction(|| {
            self.finalize_shipment(&conn)?;
            self.update_order_status(&conn)?;
            self.fetch_saved_shipment(&conn)
        }).map_err(|e| {
            error!("Transaction failed for shipping order ID {}: {}", self.order_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &saved_shipment).await?;

        Ok(saved_shipment)
    }
}

impl ShipOrderCommand {
    fn finalize_shipment(&self, conn: &PgConnection) -> Result<(), ServiceError> {
        let shipment = Shipment {
            order_id: self.order_id,
            tracking_number: self.tracking_number.clone(),
            status: ShipmentStatus::Shipped,
            // Other fields like carrier information, shipped date, etc.
        };

        diesel::insert_into(shipments::table)
            .values(&shipment)
            .execute(conn)
            .map_err(|e| {
                error!("Failed to finalize shipment for order ID {}: {}", self.order_id, e);
                ServiceError::DatabaseError(format!("Failed to finalize shipment: {}", e))
            })?;
        Ok(())
    }

    fn update_order_status(&self, conn: &PgConnection) -> Result<(), ServiceError> {
        diesel::update(orders::table.find(self.order_id))
            .set(orders::status.eq(OrderStatus::Shipped))
            .execute(conn)
            .map_err(|e| {
                error!("Failed to update order status to 'Shipped' for order ID {}: {}", self.order_id, e);
                ServiceError::DatabaseError(format!("Failed to update order status: {}", e))
            })?;
        Ok(())
    }

    fn fetch_saved_shipment(&self, conn: &PgConnection) -> Result<Shipment, ServiceError> {
        shipments::table
            .filter(shipments::order_id.eq(self.order_id))
            .filter(shipments::tracking_number.eq(&self.tracking_number))
            .first::<Shipment>(conn)
            .map_err(|e| {
                error!("Failed to fetch saved shipment for order ID {}: {}", self.order_id, e);
                ServiceError::DatabaseError(format!("Failed to fetch saved shipment: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, shipment: &Shipment) -> Result<(), ServiceError> {
        info!("Order ID: {} shipped with tracking number: {}", self.order_id, self.tracking_number);
        event_sender.send(Event::OrderShipped(self.order_id, self.tracking_number.clone()))
            .await
            .map_err(|e| {
                error!("Failed to send OrderShipped event for order ID {}: {}", self.order_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
