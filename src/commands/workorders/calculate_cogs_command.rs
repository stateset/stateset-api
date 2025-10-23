use crate::commands::Command;
use crate::events::{Event, EventSender};
use crate::{
    db::DbPool,
    errors::ServiceError,
    models::{bill_of_materials_entity, bom_item_entity, inventory_item_entity, work_order_entity},
};
use bigdecimal::BigDecimal;
use futures::stream::{self, StreamExt};
use futures::TryStreamExt;
use lazy_static::lazy_static;
use prometheus::IntCounter;
use rust_decimal::Decimal as RustDecimal;
use sea_orm::QueryOrder;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
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
    pub work_order_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct COGSResult {
    pub work_order_id: Uuid,
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
        let work_order = work_order_entity::Entity::find_by_id(self.work_order_id)
            .one(&*db)
            .await
            .map_err(|e| {
                COGS_CALCULATION_FAILURES.inc();
                error!("Failed to fetch work order {}: {}", self.work_order_id, e);
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Work order {} not found", self.work_order_id))
            })?;

        // Fetch the bill of materials
        let bom_number = work_order.bill_of_materials_number.clone().ok_or_else(|| {
            ServiceError::NotFound(format!(
                "Work order {} has no bill of materials",
                self.work_order_id
            ))
        })?;
        let bom = bill_of_materials_entity::Entity::find()
            .filter(bill_of_materials_entity::Column::Number.eq(bom_number.clone()))
            .one(&*db)
            .await
            .map_err(ServiceError::db_error)?
            .ok_or_else(|| {
                ServiceError::NotFound(format!(
                    "Bill of materials not found for work order {}",
                    self.work_order_id
                ))
            })?;

        // Calculate total cost
        let total_cost = self.calculate_total_cost(&*db, &bom).await?;

        // Calculate quantity produced - simplified version
        let quantity_produced = work_order.quantity_produced.unwrap_or(0);

        let result = COGSResult {
            work_order_id: self.work_order_id,
            total_cost,
            quantity_produced,
        };

        // Trigger an event indicating that COGS was calculated
        if let Err(e) = event_sender
            .send(Event::COGSCalculated {
                work_order_id: self.work_order_id,
                total_cogs: RustDecimal::from_str(&result.total_cost.to_string())
                    .unwrap_or(RustDecimal::ZERO),
            })
            .await
        {
            COGS_CALCULATION_FAILURES.inc();
            error!(
                "Failed to send COGSCalculated event for work order {}: {}",
                self.work_order_id, e
            );
            return Err(ServiceError::EventError(e.to_string()));
        }

        COGS_CALCULATIONS.inc();
        info!(
            work_order_id = %self.work_order_id,
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
        db: &DatabaseConnection,
        bom: &bill_of_materials_entity::Model,
    ) -> Result<BigDecimal, ServiceError> {
        let bom_items = bom_item_entity::Entity::find()
            .filter(bom_item_entity::Column::BillOfMaterialsId.eq(bom.id))
            .all(db)
            .await
            .map_err(|e| {
                error!("Failed to fetch BOM items for BOM {}: {}", bom.number, e);
                ServiceError::db_error(e)
            })?;

        let total_cost = stream::iter(bom_items)
            .map(|item| async move {
                let component_cost = self.get_component_cost(db, &item.part_number).await?;
                let quantity = BigDecimal::from_str(&item.quantity.to_string())
                    .unwrap_or_else(|_| BigDecimal::from(0));
                Ok::<BigDecimal, ServiceError>(component_cost * quantity)
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
        db: &DatabaseConnection,
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
                ServiceError::db_error(e)
            })?;

        match latest_inventory {
            Some(inventory) => {
                let cost = BigDecimal::from_str(&inventory.unit_cost.to_string())
                    .unwrap_or_else(|_| BigDecimal::from(0));
                Ok(cost)
            }
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
