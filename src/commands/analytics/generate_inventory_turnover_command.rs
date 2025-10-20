use uuid::Uuid;
use crate::models::{
    inventory_level_entity::{self, Entity as InventoryLevel},
    inventory_transaction_entity::{
        self, Entity as InventoryTransaction, InventoryTransactionType,
    },
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct GenerateInventoryTurnoverCommand {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateInventoryTurnoverResult {
    pub turnover_rate: f64,
}

#[async_trait]
impl Command for GenerateInventoryTurnoverCommand {
    type Result = GenerateInventoryTurnoverResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        let db = db_pool.as_ref();

        // Total sales during the period
        let sales_qty: i32 = InventoryTransaction::find()
            .filter(
                Condition::all()
                    .add(
                        inventory_transaction_entity::Column::TransactionType
                            .eq(InventoryTransactionType::Sale),
                    )
                    .add(
                        inventory_transaction_entity::Column::CreatedAt.gte(self.start.naive_utc()),
                    )
                    .add(inventory_transaction_entity::Column::CreatedAt.lte(self.end.naive_utc())),
            )
            .sum::<i32>(inventory_transaction_entity::Column::Quantity)
            .await
            .map_err(|e| {
                error!("Failed to sum sales quantity: {}", e);
                ServiceError::db_error(e)
            })?
            .unwrap_or(0);

        // Average on hand inventory across all products
        let levels = InventoryLevel::find().all(db).await.map_err(|e| {
            error!("Failed to fetch inventory levels: {}", e);
            ServiceError::db_error(e)
        })?;
        let avg_inventory = if levels.is_empty() {
            0.0
        } else {
            levels
                .iter()
                .map(|l| l.on_hand_quantity as f64)
                .sum::<f64>()
                / levels.len() as f64
        };

        let turnover_rate = if avg_inventory > 0.0 {
            sales_qty as f64 / avg_inventory
        } else {
            0.0
        };

        info!(start = %self.start, end = %self.end, turnover_rate, "Generating inventory turnover report");

        event_sender
            .send(Event::with_data("inventory_turnover_generated".to_string()))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(GenerateInventoryTurnoverResult { turnover_rate })
    }
}
