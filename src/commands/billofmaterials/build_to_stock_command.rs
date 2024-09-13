use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use validator::Validate;
use tracing::{info, error, instrument};
use chrono::Utc;
use sea_orm::{entity::*, query::*, DbConn, TransactionTrait};

use crate::errors::ServiceError;
use crate::events::{Event, EventSender};
use crate::db::DbPool;
use crate::models::{bom, bom_line_item, work_order, work_order_line_item};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct BuildToStockCommand {
    pub bom_id: i32,
    #[validate(range(min = 1))]
    pub quantity: i32,
    pub due_date: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildToStockResult {
    pub work_order_id: i32,
    pub bom_id: i32,
    pub quantity: i32,
}

#[async_trait::async_trait]
impl Command for BuildToStockCommand {
    type Result = BuildToStockResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let result = db.transaction::<_, ServiceError, _>(|txn| {
            Box::pin(async move {
                self.create_work_order(txn).await
            })
        }).await.map_err(|e| {
            error!("Transaction failed for creating WorkOrder for BOM ID {}: {}", self.bom_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &result).await?;

        Ok(result)
    }
}

impl BuildToStockCommand {
    async fn create_work_order(&self, txn: &DbConn) -> Result<BuildToStockResult, ServiceError> {
        // Verify that the BOM exists and get its details
        let bom = bom::Entity::find_by_id(self.bom_id)
            .one(txn)
            .await
            .map_err(|e| {
                error!("Failed to fetch BOM: {}", e);
                ServiceError::DatabaseError("Failed to fetch BOM".into())
            })?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("BOM with ID {} not found", self.bom_id))
            })?;

        // Create a new WorkOrder
        let new_work_order = work_order::ActiveModel {
            bom_id: ActiveValue::Set(self.bom_id),
            quantity: ActiveValue::Set(self.quantity),
            status: ActiveValue::Set("Planned".to_string()),
            due_date: ActiveValue::Set(self.due_date),
            created_at: ActiveValue::Set(Utc::now()),
            ..Default::default()
        };

        let work_order = work_order::Entity::insert(new_work_order)
            .exec(txn)
            .await
            .map_err(|e| {
                error!("Failed to create WorkOrder: {}", e);
                ServiceError::DatabaseError("Failed to create WorkOrder".into())
            })?;

        // Fetch BOM line items
        let bom_line_items = bom_line_item::Entity::find()
            .filter(bom_line_item::Column::BomId.eq(self.bom_id))
            .all(txn)
            .await
            .map_err(|e| {
                error!("Failed to fetch BOM line items: {}", e);
                ServiceError::DatabaseError("Failed to fetch BOM line items".into())
            })?;

        // Create WorkOrderLineItems based on BOMLineItems
        let work_order_line_items: Vec<work_order_line_item::ActiveModel> = bom_line_items
            .iter()
            .map(|item| work_order_line_item::ActiveModel {
                work_order_id: ActiveValue::Set(work_order.last_insert_id),
                component_id: ActiveValue::Set(item.component_id),
                quantity: ActiveValue::Set(item.quantity * self.quantity),
                ..Default::default()
            })
            .collect();

        work_order_line_item::Entity::insert_many(work_order_line_items)
            .exec(txn)
            .await
            .map_err(|e| {
                error!("Failed to create WorkOrderLineItems: {}", e);
                ServiceError::DatabaseError("Failed to create WorkOrderLineItems".into())
            })?;

        Ok(BuildToStockResult {
            work_order_id: work_order.last_insert_id,
            bom_id: self.bom_id,
            quantity: self.quantity,
        })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, result: &BuildToStockResult) -> Result<(), ServiceError> {
        info!("WorkOrder created for BOM ID: {}. WorkOrder ID: {}", result.bom_id, result.work_order_id);
        event_sender.send(Event::WorkOrderCreated(result.work_order_id))
            .await
            .map_err(|e| {
                error!("Failed to send WorkOrderCreated event for WorkOrder ID {}: {}", result.work_order_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
