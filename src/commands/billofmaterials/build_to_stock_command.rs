use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{entity::*, DatabaseConnection, DatabaseTransaction, Set, TransactionError, TransactionTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        billofmaterials::{self, Entity as BOM},
        bom_line_item::{self, Entity as BOMLineItem},
        work_order::{self, Entity as WorkOrder, WorkOrderStatus, WorkOrderPriority},
        work_order_line_item::{self, Entity as WorkOrderLineItem},
    },
};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct BuildToStockCommand {
    pub bom_id: i32,
    #[validate(range(min = 1))]
    pub quantity: i32,
    pub due_date: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildToStockResult {
    pub work_order_id: Uuid,
    pub bom_id: i32,
    pub quantity: i32,
}

#[async_trait::async_trait]
impl Command for BuildToStockCommand {
    type Result = BuildToStockResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let result = db
            .transaction::<_, BuildToStockResult, ServiceError>(|txn| {
                Box::pin(async move { self.create_work_order(txn).await })
            })
            .await
            .map_err(|e| {
                error!(
                    "Transaction failed for creating WorkOrder for BOM ID {}: {}",
                    self.bom_id, e
                );
                match e {
                    TransactionError::Connection(db_err) => ServiceError::DatabaseError(db_err),
                    TransactionError::Transaction(service_err) => service_err,
                }
            })?;

        self.log_and_trigger_event(event_sender, &result).await?;

        Ok(result)
    }
}

impl BuildToStockCommand {
    async fn create_work_order(&self, txn: &DatabaseTransaction) -> Result<BuildToStockResult, ServiceError> {
        // Verify that the BOM exists and get its details
        let bom = BOM::find_by_id(self.bom_id)
            .one(txn)
            .await
            .map_err(|e| {
                error!("Failed to fetch BOM ID {}: {}", self.bom_id, e);
                ServiceError::DatabaseError(e)
            })?
            .ok_or_else(|| {
                error!("BOM ID {} not found", self.bom_id);
                ServiceError::NotFound(format!("BOM {} not found", self.bom_id))
            })?;

        // Create a new WorkOrder
        let work_order_id = Uuid::new_v4();
        let new_work_order = work_order::ActiveModel {
            id: Set(work_order_id),
            title: Set(format!("Build to Stock - BOM {}", self.bom_id)),
            description: Set(Some(format!("Build to stock for BOM {}", self.bom_id))),
            status: Set(WorkOrderStatus::Pending),
            priority: Set(WorkOrderPriority::Normal),
            asset_id: Set(None),
            assigned_to: Set(None),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            due_date: Set(self.due_date),
            parts_required: Set(serde_json::Value::Object(serde_json::Map::new())),
        };

        let work_order = WorkOrder::insert(new_work_order)
            .exec(txn)
            .await
            .map_err(|e| {
                error!("Failed to create WorkOrder: {}", e);
                ServiceError::DatabaseError(e)
            })?;

        // Fetch BOM line items
        let bom_line_items = BOMLineItem::find()
            .filter(bom_line_item::Column::BillOfMaterialsId.eq(self.bom_id))
            .all(txn)
            .await
            .map_err(|e| {
                error!("Failed to fetch BOM line items: {}", e);
                ServiceError::DatabaseError(e)
            })?;

        // Create WorkOrderLineItems based on BOMLineItems
        let work_order_line_items: Vec<work_order_line_item::ActiveModel> = bom_line_items
            .into_iter()
            .map(|item| work_order_line_item::ActiveModel {
                work_order_id: Set(work_order_id),
                line_status: Set("Pending".to_string()),
                line_type: Set("Component".to_string()),
                part_name: Set(item.part_name),
                part_number: Set(item.part_number),
                total_quantity: Set(item.quantity * self.quantity as f64),
                picked_quantity: Set(0.0),
                issued_quantity: Set(0.0),
                yielded_quantity: Set(0.0),
                scrapped_quantity: Set(0.0),
                unit_of_measure: Set("EA".to_string()),
                ..Default::default()
            })
            .collect();

        WorkOrderLineItem::insert_many(work_order_line_items)
            .exec(txn)
            .await
            .map_err(|e| {
                error!("Failed to create WorkOrderLineItems: {}", e);
                ServiceError::DatabaseError(e)
            })?;

        Ok(BuildToStockResult {
            work_order_id,
            bom_id: self.bom_id,
            quantity: self.quantity,
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        result: &BuildToStockResult,
    ) -> Result<(), ServiceError> {
        info!(
            "WorkOrder created for BOM ID: {}. WorkOrder ID: {}",
            result.bom_id, result.work_order_id
        );
        event_sender
            .send(Event::WorkOrderCreated(result.work_order_id))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send WorkOrderCreated event for WorkOrder ID {}: {}",
                    result.work_order_id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}
