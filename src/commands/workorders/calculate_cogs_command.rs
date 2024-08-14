use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{WorkOrder, BillOfMaterials, InventoryItem}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use diesel::dsl::*;
use prometheus::IntCounter;
use futures::stream::{self, StreamExt};
use bigdecimal::BigDecimal;

lazy_static! {
    static ref COGS_CALCULATIONS: IntCounter = 
        IntCounter::new("cogs_calculations_total", "Total number of COGS calculations")
            .expect("metric can be created");

    static ref COGS_CALCULATION_FAILURES: IntCounter = 
        IntCounter::new("cogs_calculation_failures_total", "Total number of failed COGS calculations")
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

#[async_trait]
impl Command for CalculateCOGSCommand {
    type Result = COGSResult;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            COGS_CALCULATION_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        // Fetch the work order
        let work_order = work_orders::table
            .filter(work_orders::number.eq(&self.work_order_number))
            .first::<WorkOrder>(&conn)
            .map_err(|e| {
                COGS_CALCULATION_FAILURES.inc();
                error!("Failed to fetch work order {}: {}", self.work_order_number, e);
                ServiceError::DatabaseError
            })?;

        // Fetch the bill of materials
        let bom = bill_of_materials::table
            .filter(bill_of_materials::number.eq(&work_order.bill_of_materials_number))
            .first::<BillOfMaterials>(&conn)
            .map_err(|e| {
                COGS_CALCULATION_FAILURES.inc();
                error!("Failed to fetch BOM for work order {}: {}", self.work_order_number, e);
                ServiceError::DatabaseError
            })?;

        // Calculate total cost
        let total_cost = self.calculate_total_cost(&conn, &bom).await?;

        // Calculate quantity produced
        let quantity_produced = work_order_items::table
            .filter(work_order_items::work_order_id.eq(work_order.id))
            .select(sum(work_order_items::total_quantity))
            .first::<Option<i32>>(&conn)
            .map_err(|e| {
                COGS_CALCULATION_FAILURES.inc();
                error!("Failed to calculate quantity produced for work order {}: {}", self.work_order_number, e);
                ServiceError::DatabaseError
            })?.unwrap_or(0);

        let result = COGSResult {
            work_order_number: self.work_order_number.clone(),
            total_cost,
            quantity_produced,
        };

        // Trigger an event indicating that COGS was calculated
        if let Err(e) = event_sender.send(Event::COGSCalculated(self.work_order_number.clone(), result.total_cost.clone())).await {
            COGS_CALCULATION_FAILURES.inc();
            error!("Failed to send COGSCalculated event for work order {}: {}", self.work_order_number, e);
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
    async fn calculate_total_cost(&self, conn: &PgConnection, bom: &BillOfMaterials) -> Result<BigDecimal, ServiceError> {
        let bom_items = bom_items::table
            .filter(bom_items::bill_of_materials_id.eq(bom.id))
            .load::<BOMItem>(conn)
            .map_err(|e| {
                COGS_CALCULATION_FAILURES.inc();
                error!("Failed to fetch BOM items for BOM {}: {}", bom.number, e);
                ServiceError::DatabaseError
            })?;

        let total_cost = stream::iter(bom_items)
            .map(|item| async move {
                let component_cost = self.get_component_cost(conn, &item.part_number).await?;
                Ok::<BigDecimal, ServiceError>(component_cost * BigDecimal::from(item.quantity))
            })
            .buffer_unordered(10) // Process up to 10 items concurrently
            .try_fold(BigDecimal::from(0), |acc, cost| async move { Ok(acc + cost) })
            .await?;

        Ok(total_cost)
    }

    async fn get_component_cost(&self, conn: &PgConnection, part_number: &str) -> Result<BigDecimal, ServiceError> {
        let latest_inventory = inventory_items::table
            .filter(inventory_items::part_number.eq(part_number))
            .order(inventory_items::updated_at.desc())
            .first::<InventoryItem>(conn)
            .optional()
            .map_err(|e| {
                COGS_CALCULATION_FAILURES.inc();
                error!("Failed to fetch latest inventory for part {}: {}", part_number, e);
                ServiceError::DatabaseError
            })?;

        match latest_inventory {
            Some(inventory) => Ok(inventory.unit_cost),
            None => {
                COGS_CALCULATION_FAILURES.inc();
                error!("No inventory found for part number: {}", part_number);
                Err(ServiceError::BusinessLogicError(format!("No inventory found for part number: {}", part_number)))
            }
        }
    }
}