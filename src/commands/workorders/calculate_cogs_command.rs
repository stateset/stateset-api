use uuid::Uuid;
use sea_orm::DatabaseTransaction;
use crate::commands::Command;
use crate::events::{Event, EventSender};
use crate::{
    db::DbPool,
    errors::ServiceError,
    models::{bill_of_materials_entity, bom_item_entity, inventory_item_entity, work_order_entity},
};
use async_trait::async_trait;
use bigdecimal::BigDecimal;
use rust_decimal::Decimal as RustDecimal;
use futures::stream::{self, StreamExt};
use futures::TryStreamExt;
use lazy_static::lazy_static;
use prometheus::IntCounter;
use sea_orm::QueryOrder;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, Order, QueryFilter};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

lazy_static! {
    static ref COGS_CALCULATIONS: IntCounter = IntCounter::new(
        "cogs_calculations_total",
        "Total number of COGS calculations"
    )
    .expect("metric can be created");
    static ref COGS_CALCULATION_FAILURES: IntCounter = IntCounter::new(
        "cogs_calculation_failures_total",
        "Total number of failed COGS calculations"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CalculateCOGSCommand {
    pub work_order_number: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct COGSResult {
    pub work_order_number: String,
    pub total_cost: BigDecimal,
    pub quantity_produced: i32,
}

#[async_trait::async_trait]
impl Command for CalculateCOGSCommand {
    type Result = COGSResult;
    
    #[instrument(skip(db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();
        
        // Fetch the work order
        let work_order = work_order_entity::Entity::find()
            .filter(work_order_entity::Column::Id.eq(self.work_order_number.parse::<Uuid>().map_err(|_| ServiceError::ValidationError("Invalid work order number format".to_string()))?))
            .one(&*db)
            .await
            .map_err(|e| {
                COGS_CALCULATION_FAILURES.inc();
                error!(
                    "Failed to fetch work order {}: {}",
                    self.work_order_number, e
                );
                ServiceError::DatabaseError(e)
            })?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Work order {} not found", self.work_order_number))
            })?;

        // Fetch the bill of materials
        let bom = bill_of_materials_entity::Entity::find()
            .filter(
                bill_of_materials_entity::Column::Number
                    .eq(work_order.bill_of_materials_number.clone()),
            )
            .one(&*db)
            .await
            .map_err(ServiceError::DatabaseError)?
            .ok_or_else(|| ServiceError::NotFound(format!("Bill of materials not found for work order {}", self.work_order_number)))?;
        
        // Calculate total cost
        let total_cost = self.calculate_total_cost(&*db, &bom).await?;
        
        // Calculate quantity produced - simplified version
        let quantity_produced = work_order.quantity_produced.unwrap_or(0);
        
        let result = COGSResult {
            work_order_number: self.work_order_number.clone(),
            total_cost,
            quantity_produced,
        };
        
        // Trigger an event indicating that COGS was calculated
        if let Err(e) = event_sender
            .send(Event::COGSCalculated {
                work_order_id: self.work_order_number.parse::<Uuid>().unwrap_or_default(),
                total_cogs: RustDecimal::from_str(&result.total_cost.to_string()).unwrap_or(RustDecimal::ZERO),
            })
            .await
        {
            COGS_CALCULATION_FAILURES.inc();
            error!(
                "Failed to send COGSCalculated event for work order {}: {}",
                self.work_order_number, e
            );
            return Err(ServiceError::EventError(e.to_string()));
        }
        
        COGS_CALCULATIONS.inc();
        info!(
            work_order_number = %self.work_order_number,
            total_cost = %result.total_cost,
            quantity_produced = %result.quantity_produced,
            "COGS calculated successfully"
        );
        Ok(result)
    }
}

impl CalculateCOGSCommand {
    async fn calculate_total_cost(
        &self,
        db: &DatabaseTransaction,
        bom: &bill_of_materials_entity::Model,
    ) -> Result<BigDecimal, ServiceError> {
        let bom_items = bom_item_entity::Entity::find()
            .filter(bom_item_entity::Column::BillOfMaterialsId.eq(bom.id))
            .all(db)
            .await
            .map_err(|e| {
                error!("Failed to fetch BOM items for BOM {}: {}", bom.number, e);
                ServiceError::DatabaseError(e)
            })?;
        
        let total_cost = stream::iter(bom_items)
            .map(|item| async move {
                let component_cost = self.get_component_cost(db, &item.part_number).await?;
                Ok::<BigDecimal, ServiceError>(component_cost * BigDecimal::from_f64(item.quantity).unwrap_or(BigDecimal::from(0)))
            })
            .buffer_unordered(10) // Process up to 10 items concurrently
            .try_fold(
                BigDecimal::from(0),
                |acc, cost| async move { Ok(acc + cost) },
            )
            .await?;
        Ok(total_cost)
    }

    async fn get_component_cost(
        &self,
        db: &DatabaseTransaction,
        part_number: &str,
    ) -> Result<BigDecimal, ServiceError> {
        let latest_inventory = inventory_item_entity::Entity::find()
            .filter(inventory_item_entity::Column::LotNumber.eq(part_number))
            .order_by_desc(inventory_item_entity::Column::UpdatedAt)
            .one(db)
            .await
            .map_err(|e| {
                error!(
                    "Failed to fetch latest inventory for part {}: {}",
                    part_number, e
                );
                ServiceError::DatabaseError(e)
            })?;
        
        match latest_inventory {
            Some(inventory) => Ok(inventory.unit_cost),
            None => {
                error!("No inventory found for part number: {}", part_number);
                Err(ServiceError::InvalidOperation(format!(
                    "No inventory found for part number: {}",
                    part_number
                )))
            }
        }
    }
}