use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{PurchaseOrder, InventoryMovement, ManufacturingCost}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use chrono::{NaiveDateTime, Utc};
use bigdecimal::BigDecimal;
use std::cmp::Ordering;

lazy_static! {
    static ref WAVG_COGS_CALCULATIONS: IntCounter = 
        IntCounter::new("wavg_cogs_calculations_total", "Total number of Weighted Average COGS calculations")
            .expect("metric can be created");

    static ref WAVG_COGS_CALCULATION_FAILURES: IntCounter = 
        IntCounter::new("wavg_cogs_calculation_failures_total", "Total number of failed Weighted Average COGS calculations")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CalculateWeightedAverageCOGSCommand {
    pub product_id: i32,
    pub start_date: NaiveDateTime,
    pub end_date: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WeightedAverageCOGSResult {
    pub cogs: BigDecimal,
    pub ending_inventory: EndingInventory,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EndingInventory {
    pub quantity: BigDecimal,
    pub average_cost: BigDecimal,
}

#[derive(Debug)]
enum Movement {
    Purchase(PurchaseOrder),
    InventoryMovement(InventoryMovement),
    Manufacturing(ManufacturingCost),
}

impl Movement {
    fn date(&self) -> NaiveDateTime {
        match self {
            Movement::Purchase(po) => po.date,
            Movement::InventoryMovement(im) => im.date,
            Movement::Manufacturing(mc) => mc.date,
        }
    }
}

#[async_trait]
impl Command for CalculateWeightedAverageCOGSCommand {
    type Result = WeightedAverageCOGSResult;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            WAVG_COGS_CALCULATION_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        let purchase_orders = self.get_purchase_orders(&conn)?;
        let inventory_movements = self.get_inventory_movements(&conn)?;
        let manufacturing_costs = self.get_manufacturing_costs(&conn)?;

        let mut all_movements: Vec<Movement> = Vec::new();
        all_movements.extend(purchase_orders.into_iter().map(Movement::Purchase));
        all_movements.extend(inventory_movements.into_iter().map(Movement::InventoryMovement));
        all_movements.extend(manufacturing_costs.into_iter().map(Movement::Manufacturing));

        all_movements.sort_by(|a, b| a.date().cmp(&b.date()));

        let result = self.calculate_cogs(all_movements)?;

        // Trigger an event indicating that Weighted Average COGS was calculated
        if let Err(e) = event_sender.send(Event::WeightedAverageCOGSCalculated(self.product_id, result.cogs.clone())).await {
            WAVG_COGS_CALCULATION_FAILURES.inc();
            error!("Failed to send WeightedAverageCOGSCalculated event for product {}: {}", self.product_id, e);
            return Err(ServiceError::EventError(e.to_string()));
        }

        WAVG_COGS_CALCULATIONS.inc();

        info!(
            product_id = %self.product_id,
            cogs = %result.cogs,
            ending_inventory_quantity = %result.ending_inventory.quantity,
            ending_inventory_cost = %result.ending_inventory.average_cost,
            "Weighted Average COGS calculated successfully"
        );

        Ok(result)
    }
}

impl CalculateWeightedAverageCOGSCommand {
    fn get_purchase_orders(&self, conn: &PgConnection) -> Result<Vec<PurchaseOrder>, ServiceError> {
        purchase_orders::table
            .filter(purchase_orders::product_id.eq(self.product_id))
            .filter(purchase_orders::date.between(self.start_date, self.end_date))
            .load::<PurchaseOrder>(conn)
            .map_err(|e| {
                WAVG_COGS_CALCULATION_FAILURES.inc();
                error!("Failed to fetch purchase orders for product {}: {}", self.product_id, e);
                ServiceError::DatabaseError
            })
    }

    fn get_inventory_movements(&self, conn: &PgConnection) -> Result<Vec<InventoryMovement>, ServiceError> {
        inventory_movements::table
            .filter(inventory_movements::product_id.eq(self.product_id))
            .filter(inventory_movements::date.between(self.start_date, self.end_date))
            .load::<InventoryMovement>(conn)
            .map_err(|e| {
                WAVG_COGS_CALCULATION_FAILURES.inc();
                error!("Failed to fetch inventory movements for product {}: {}", self.product_id, e);
                ServiceError::DatabaseError
            })
    }

    fn get_manufacturing_costs(&self, conn: &PgConnection) -> Result<Vec<ManufacturingCost>, ServiceError> {
        manufacturing_costs::table
            .filter(manufacturing_costs::product_id.eq(self.product_id))
            .filter(manufacturing_costs::date.between(self.start_date, self.end_date))
            .load::<ManufacturingCost>(conn)
            .map_err(|e| {
                WAVG_COGS_CALCULATION_FAILURES.inc();
                error!("Failed to fetch manufacturing costs for product {}: {}", self.product_id, e);
                ServiceError::DatabaseError
            })
    }

    fn calculate_cogs(&self, movements: Vec<Movement>) -> Result<WeightedAverageCOGSResult, ServiceError> {
        let mut inventory = EndingInventory {
            quantity: BigDecimal::from(0),
            average_cost: BigDecimal::from(0),
        };
        let mut cogs = BigDecimal::from(0);
        let mut total_purchases = Vec::new();

        for movement in movements {
            match movement {
                Movement::Purchase(po) => {
                    total_purchases.push((BigDecimal::from(po.quantity), po.unit_cost));
                }
                Movement::Manufacturing(mc) => {
                    let unit_cost = mc.total_cost.div(BigDecimal::from(mc.quantity));
                    total_purchases.push((BigDecimal::from(mc.quantity), unit_cost));
                }
                Movement::InventoryMovement(im) => {
                    if im.quantity_change > 0 {
                        let new_average_cost = Self::calculate_weighted_average_cost(&inventory, &total_purchases);
                        inventory.quantity += BigDecimal::from(im.quantity_change);
                        inventory.average_cost = new_average_cost;
                        total_purchases.clear();
                    } else {
                        let quantity_sold = BigDecimal::from(im.quantity_change.abs());
                        cogs += &inventory.average_cost * &quantity_sold;
                        inventory.quantity -= quantity_sold;
                    }
                }
            }
        }

        Ok(WeightedAverageCOGSResult {
            cogs,
            ending_inventory: inventory,
        })
    }

    fn calculate_weighted_average_cost(inventory: &EndingInventory, purchases: &[(BigDecimal, BigDecimal)]) -> BigDecimal {
        let mut total_cost = &inventory.quantity * &inventory.average_cost;
        let mut total_quantity = inventory.quantity.clone();

        for (quantity, cost) in purchases {
            total_cost += quantity * cost;
            total_quantity += quantity;
        }

        if total_quantity.is_zero() {
            BigDecimal::from(0)
        } else {
            total_cost.div(total_quantity)
        }
    }
}