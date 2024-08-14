use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{PurchaseOrder, InventoryItem, ManufacturingOrder}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use chrono::{NaiveDateTime};
use bigdecimal::BigDecimal;

lazy_static! {
    static ref AVERAGE_COST_CALCULATIONS: IntCounter = 
        IntCounter::new("average_cost_calculations_total", "Total number of average cost calculations")
            .expect("metric can be created");

    static ref AVERAGE_COST_CALCULATION_FAILURES: IntCounter = 
        IntCounter::new("average_cost_calculation_failures_total", "Total number of failed average cost calculations")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CalculateAverageCostCommand {
    pub product_id: i32,
    pub start_date: NaiveDateTime,
    pub end_date: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AverageCostResult {
    pub average_cost: BigDecimal,
    pub total_cost: BigDecimal,
    pub total_quantity: BigDecimal,
}

#[async_trait]
impl Command for CalculateAverageCostCommand {
    type Result = AverageCostResult;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            AVERAGE_COST_CALCULATION_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        let purchases = self.get_purchases(&conn)?;
        let inventory = self.get_inventory(&conn)?;
        let manufacturing_costs = self.get_manufacturing_costs(&conn)?;

        let total_cost = purchases.iter().fold(BigDecimal::from(0), |sum, po| sum + &po.total_cost) +
                         manufacturing_costs.iter().fold(BigDecimal::from(0), |sum, cost| sum + &cost.amount);

        let total_quantity = inventory.iter().fold(BigDecimal::from(0), |sum, inv| sum + BigDecimal::from(inv.quantity));

        let average_cost = if !total_quantity.is_zero() {
            total_cost.clone() / total_quantity.clone()
        } else {
            BigDecimal::from(0)
        };

        let result = AverageCostResult {
            average_cost: average_cost.clone(),
            total_cost: total_cost.clone(),
            total_quantity: total_quantity.clone(),
        };

        // Trigger an event indicating that average cost was calculated
        if let Err(e) = event_sender.send(Event::AverageCostCalculated(self.product_id, average_cost)).await {
            AVERAGE_COST_CALCULATION_FAILURES.inc();
            error!("Failed to send AverageCostCalculated event for product {}: {}", self.product_id, e);
            return Err(ServiceError::EventError(e.to_string()));
        }

        AVERAGE_COST_CALCULATIONS.inc();

        info!(
            product_id = %self.product_id,
            average_cost = %result.average_cost,
            total_cost = %result.total_cost,
            total_quantity = %result.total_quantity,
            "Average cost calculated successfully"
        );

        Ok(result)
    }
}

impl CalculateAverageCostCommand {
    fn get_purchases(&self, conn: &PgConnection) -> Result<Vec<PurchaseOrder>, ServiceError> {
        purchase_orders::table
            .filter(purchase_orders::product_id.eq(self.product_id))
            .filter(purchase_orders::date.between(self.start_date, self.end_date))
            .load::<PurchaseOrder>(conn)
            .map_err(|e| {
                AVERAGE_COST_CALCULATION_FAILURES.inc();
                error!("Failed to fetch purchase orders for product {}: {}", self.product_id, e);
                ServiceError::DatabaseError
            })
    }

    fn get_inventory(&self, conn: &PgConnection) -> Result<Vec<InventoryItem>, ServiceError> {
        inventory_items::table
            .filter(inventory_items::product_id.eq(self.product_id))
            .filter(inventory_items::date.between(self.start_date, self.end_date))
            .load::<InventoryItem>(conn)
            .map_err(|e| {
                AVERAGE_COST_CALCULATION_FAILURES.inc();
                error!("Failed to fetch inventory for product {}: {}", self.product_id, e);
                ServiceError::DatabaseError
            })
    }

    fn get_manufacturing_costs(&self, conn: &PgConnection) -> Result<Vec<ManufacturingOrder>, ServiceError> {
        manufacturing_orders::table
            .filter(manufacturing_orders::product_id.eq(self.product_id))
            .filter(manufacturing_orders::date.between(self.start_date, self.end_date))
            .load::<ManufacturingOrder>(conn)
            .map_err(|e| {
                AVERAGE_COST_CALCULATION_FAILURES.inc();
                error!("Failed to fetch manufacturing costs for product {}: {}", self.product_id, e);
                ServiceError::DatabaseError
            })
    }
}