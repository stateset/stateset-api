use crate::commands::Command;
use crate::events::{Event, EventSender};
use crate::{
    db::DbPool,
    errors::ServiceError,
    models::{inventory_transaction_entity, manufacturing_cost_entity, purchase_order_item_entity},
};
use bigdecimal::{BigDecimal, Zero};
use chrono::NaiveDateTime;
use lazy_static::lazy_static;
use prometheus::IntCounter;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref WAVG_COGS_CALCULATIONS: IntCounter = IntCounter::new(
        "wavg_cogs_calculations_total",
        "Total number of Weighted Average COGS calculations"
    )
    .expect("metric can be created");
    static ref WAVG_COGS_CALCULATION_FAILURES: IntCounter = IntCounter::new(
        "wavg_cogs_calculation_failures_total",
        "Total number of failed Weighted Average COGS calculations"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CalculateWeightedAverageCOGSCommand {
    pub product_id: Uuid,
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
    Purchase(purchase_order_item_entity::Model),
    InventoryMovement(inventory_transaction_entity::Model),
    Manufacturing(manufacturing_cost_entity::Model),
}

impl Movement {
    fn date(&self) -> NaiveDateTime {
        match self {
            Movement::Purchase(poi) => poi.created_at.naive_utc(),
            Movement::InventoryMovement(im) => im.created_at.naive_utc(),
            Movement::Manufacturing(mc) => mc.created_at.naive_utc(),
        }
    }
}

#[async_trait::async_trait]
impl Command for CalculateWeightedAverageCOGSCommand {
    type Result = WeightedAverageCOGSResult;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();
        let purchase_orders = self.get_purchase_orders(&db).await?;
        let inventory_movements = self.get_inventory_movements(&db).await?;
        let manufacturing_costs = self.get_manufacturing_costs(&db).await?;

        let mut all_movements: Vec<Movement> = Vec::new();
        all_movements.extend(purchase_orders.into_iter().map(Movement::Purchase));
        all_movements.extend(
            inventory_movements
                .into_iter()
                .map(Movement::InventoryMovement),
        );
        all_movements.extend(manufacturing_costs.into_iter().map(Movement::Manufacturing));
        all_movements.sort_by(|a, b| a.date().cmp(&b.date()));

        let result = self.calculate_cogs(all_movements)?;
        let total_cogs_decimal = rust_decimal::Decimal::from_str(&result.cogs.to_string())
            .unwrap_or(rust_decimal::Decimal::ZERO);

        // Trigger an event indicating that Weighted Average COGS was calculated
        if let Err(e) = event_sender
            .send(Event::WeightedAverageCOGSCalculated(
                self.product_id,
                total_cogs_decimal,
            ))
            .await
        {
            WAVG_COGS_CALCULATION_FAILURES.inc();
            error!(
                "Failed to send WeightedAverageCOGSCalculated event for product {}: {}",
                self.product_id, e
            );
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
    async fn get_purchase_orders(
        &self,
        db: &DatabaseConnection,
    ) -> Result<Vec<purchase_order_item_entity::Model>, ServiceError> {
        purchase_order_item_entity::Entity::find()
            .filter(purchase_order_item_entity::Column::ProductName.is_not_null())
            .all(db)
            .await
            .map_err(|e| {
                WAVG_COGS_CALCULATION_FAILURES.inc();
                error!(
                    "Failed to fetch purchase order items for product {}: {}",
                    self.product_id, e
                );
                ServiceError::db_error(e)
            })
    }

    async fn get_inventory_movements(
        &self,
        db: &DatabaseConnection,
    ) -> Result<Vec<inventory_transaction_entity::Model>, ServiceError> {
        inventory_transaction_entity::Entity::find()
            .filter(inventory_transaction_entity::Column::ProductId.eq(self.product_id))
            .filter(
                inventory_transaction_entity::Column::CreatedAt
                    .between(self.start_date, self.end_date),
            )
            .all(db)
            .await
            .map_err(|e| {
                WAVG_COGS_CALCULATION_FAILURES.inc();
                error!(
                    "Failed to fetch inventory movements for product {}: {}",
                    self.product_id, e
                );
                ServiceError::db_error(e)
            })
    }

    async fn get_manufacturing_costs(
        &self,
        db: &DatabaseConnection,
    ) -> Result<Vec<manufacturing_cost_entity::Model>, ServiceError> {
        // Since manufacturing_cost_entity doesn't have ProductId and Date columns directly,
        // we need to join with work_order table to get the relevant costs
        let costs = manufacturing_cost_entity::Entity::find()
            .filter(
                manufacturing_cost_entity::Column::CreatedAt
                    .between(self.start_date, self.end_date),
            )
            .all(db)
            .await
            .map_err(|e| {
                WAVG_COGS_CALCULATION_FAILURES.inc();
                error!(
                    "Failed to fetch manufacturing costs for product {}: {}",
                    self.product_id, e
                );
                ServiceError::db_error(e)
            })?;

        // For now, return all manufacturing costs
        // In a real implementation, we would filter by product_id using a join with work_orders
        Ok(costs)
    }

    fn calculate_cogs(
        &self,
        movements: Vec<Movement>,
    ) -> Result<WeightedAverageCOGSResult, ServiceError> {
        let mut inventory = EndingInventory {
            quantity: BigDecimal::from(0),
            average_cost: BigDecimal::from(0),
        };
        let mut cogs = BigDecimal::from(0);
        let mut total_purchases = Vec::new();

        for movement in movements {
            match movement {
                Movement::Purchase(poi) => {
                    total_purchases.push((
                        BigDecimal::from(poi.quantity_received),
                        poi.unit_cost
                            .to_string()
                            .parse::<BigDecimal>()
                            .unwrap_or_default(),
                    ));
                }
                Movement::Manufacturing(mc) => {
                    // Get the quantity from the work order associated with this manufacturing cost
                    // For now, we'll use a placeholder quantity of 1
                    let quantity = BigDecimal::from(1);
                    let unit_cost = mc
                        .cost_amount
                        .to_string()
                        .parse::<BigDecimal>()
                        .unwrap_or_default();
                    total_purchases.push((quantity, unit_cost));
                }
                Movement::InventoryMovement(im) => {
                    match im.transaction_type {
                        inventory_transaction_entity::InventoryTransactionType::Receipt
                        | inventory_transaction_entity::InventoryTransactionType::Return
                        | inventory_transaction_entity::InventoryTransactionType::Production => {
                            let new_average_cost =
                                Self::calculate_weighted_average_cost(&inventory, &total_purchases);
                            inventory.quantity += BigDecimal::from(im.quantity);
                            inventory.average_cost = new_average_cost;
                            total_purchases.clear();
                        }
                        inventory_transaction_entity::InventoryTransactionType::Sale
                        | inventory_transaction_entity::InventoryTransactionType::Scrap => {
                            let quantity_sold = BigDecimal::from(im.quantity.abs());
                            cogs += &inventory.average_cost * &quantity_sold;
                            inventory.quantity -= quantity_sold;
                        }
                        _ => {
                            // For other transaction types like Adjustment, Count, Transfer
                            // We need to check if it's an addition or reduction
                            if im.quantity > 0 {
                                let new_average_cost = Self::calculate_weighted_average_cost(
                                    &inventory,
                                    &total_purchases,
                                );
                                inventory.quantity += BigDecimal::from(im.quantity);
                                inventory.average_cost = new_average_cost;
                                total_purchases.clear();
                            } else if im.quantity < 0 {
                                let quantity_change = BigDecimal::from(im.quantity.abs());
                                cogs += &inventory.average_cost * &quantity_change;
                                inventory.quantity -= quantity_change;
                            }
                        }
                    }
                }
            }
        }

        Ok(WeightedAverageCOGSResult {
            cogs,
            ending_inventory: inventory,
        })
    }

    fn calculate_weighted_average_cost(
        inventory: &EndingInventory,
        purchases: &[(BigDecimal, BigDecimal)],
    ) -> BigDecimal {
        let mut total_cost = &inventory.quantity * &inventory.average_cost;
        let mut total_quantity = inventory.quantity.clone();

        for (quantity, cost) in purchases {
            total_cost += quantity * cost;
            total_quantity += quantity;
        }

        if total_quantity.is_zero() {
            BigDecimal::from(0)
        } else {
            total_cost / total_quantity
        }
    }
}
