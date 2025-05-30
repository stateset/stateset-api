use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use validator::Validate;
use tracing::{info, error, instrument};
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, TransactionTrait, Set};

use crate::errors::ServiceError;
use crate::events::{Event, EventSender};
use crate::models::{shipment, order, ShipmentStatus, OrderStatus};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ShipOrderCommand {
    pub order_id: i32,

    #[validate(length(min = 1))]
    pub tracking_number: String, // Shipment tracking number
}

#[async_trait::async_trait]
impl Command for ShipOrderCommand {
    type Result = shipment::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DatabaseConnection>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let txn = db_pool.begin().await.map_err(|e| {
            error!("Failed to begin transaction: {}", e);
            ServiceError::DatabaseError("Failed to begin transaction".into())
        })?;

        let saved_shipment = self.finalize_shipment(&txn).await
            .and_then(|_| self.update_order_status(&txn).await)
            .and_then(|_| self.fetch_saved_shipment(&txn).await)
            .await
            .map_err(|e| {
                txn.rollback().await.ok();
                error!("Transaction failed for shipping order ID {}: {}", self.order_id, e);
                e
            })?;

        txn.commit().await.map_err(|e| {
            error!("Failed to commit transaction: {}", e);
            ServiceError::DatabaseError("Failed to commit transaction".into())
        })?;

        self.log_and_trigger_event(event_sender, &saved_shipment).await?;

        Ok(saved_shipment)
    }
}

impl ShipOrderCommand {
    async fn finalize_shipment(&self, txn: &DatabaseTransaction) -> Result<(), ServiceError> {
        let new_shipment = shipment::ActiveModel {
            order_id: Set(self.order_id),
            tracking_number: Set(self.tracking_number.clone()),
            status: Set(ShipmentStatus::Shipped),
            ..Default::default()
        };

        new_shipment.insert(txn).await.map_err(|e| {
            error!("Failed to finalize shipment for order ID {}: {}", self.order_id, e);
            ServiceError::DatabaseError(format!("Failed to finalize shipment: {}", e))
        })?;
        Ok(())
    }

    async fn update_order_status(&self, txn: &DatabaseTransaction) -> Result<(), ServiceError> {
        let mut order: order::ActiveModel = order::Entity::find_by_id(self.order_id)
            .one(txn).await.map_err(|e| {
                error!("Failed to fetch order ID {}: {}", self.order_id, e);
                ServiceError::DatabaseError(format!("Failed to fetch order: {}", e))
            })?
            .ok_or_else(|| ServiceError::NotFound("Order not found".to_string()))?
            .into();

        order.status = Set(OrderStatus::Shipped);

        order.update(txn).await.map_err(|e| {
            error!("Failed to update order status to 'Shipped' for order ID {}: {}", self.order_id, e);
            ServiceError::DatabaseError(format!("Failed to update order status: {}", e))
        })?;
        Ok(())
    }

    async fn fetch_saved_shipment(&self, txn: &DatabaseTransaction) -> Result<shipment::Model, ServiceError> {
        shipment::Entity::find()
            .filter(shipment::Column::OrderId.eq(self.order_id))
            .filter(shipment::Column::TrackingNumber.eq(self.tracking_number.clone()))
            .one(txn).await.map_err(|e| {
                error!("Failed to fetch saved shipment for order ID {}: {}", self.order_id, e);
                ServiceError::DatabaseError(format!("Failed to fetch saved shipment: {}", e))
            })?
            .ok_or_else(|| ServiceError::NotFound("Shipment not found".to_string()))
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, shipment: &shipment::Model) -> Result<(), ServiceError> {
        info!("Order ID: {} shipped with tracking number: {}", self.order_id, self.tracking_number);
        event_sender.send(Event::OrderShipped(self.order_id, self.tracking_number.clone()))
            .await
            .map_err(|e| {
                error!("Failed to send OrderShipped event for order ID {}: {}", self.order_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
