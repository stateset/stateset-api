use crate::commands::Command;
use crate::events::{Event, EventSender};
use crate::{
    db::DbPool,
    errors::ServiceError,
    models::{inventory_item_entity, manufacturing_order_entity, purchase_order_entity},
};
use async_trait::async_trait;
use bigdecimal::{BigDecimal, Zero};
use chrono::{DateTime, NaiveDateTime, Utc};
use lazy_static::lazy_static;
use prometheus::IntCounter;
use rust_decimal::Decimal as RustDecimal;
use sea_orm::{ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref AVERAGE_COST_CALCULATIONS: IntCounter = IntCounter::new(
        "average_cost_calculations_total",
        "Total number of average cost calculations"
    )
    .expect("metric can be created");
    static ref AVERAGE_COST_CALCULATION_FAILURES: IntCounter = IntCounter::new(
        "average_cost_calculation_failures_total",
        "Total number of failed average cost calculations"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CalculateAverageCostCommand {
    pub product_id: Uuid,
    pub start_date: NaiveDateTime,
    pub end_date: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AverageCostResult {
    pub average_cost: BigDecimal,
    pub total_amount: BigDecimal,
    pub total_quantity: BigDecimal,
}
#[async_trait::async_trait]
impl Command for CalculateAverageCostCommand {
    type Result = AverageCostResult;
    #[instrument(skip(db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();
        let purchases = self.get_purchases(&db).await?;
        let inventory = self.get_inventory(&db).await?;
        let manufacturing_costs = self.get_manufacturing_costs(&db).await?;
        let total_amount = purchases.iter().fold(BigDecimal::from(0), |sum, po| {
            sum + BigDecimal::from_str(&po.total_amount.to_string()).unwrap_or(BigDecimal::from(0))
        }) + manufacturing_costs.iter().fold(
            BigDecimal::from(0),
            |sum, cost| {
                sum + BigDecimal::from_str(&cost.amount.to_string()).unwrap_or(BigDecimal::from(0))
            },
        );
        let total_quantity = inventory.iter().fold(BigDecimal::from(0), |sum, inv| {
            sum + BigDecimal::from(inv.quantity)
        });
        let average_cost = if !total_quantity.is_zero() {
            total_amount.clone() / total_quantity.clone()
        } else {
            BigDecimal::from(0)
        };
        let result = AverageCostResult {
            average_cost: average_cost.clone(),
            total_amount: total_amount.clone(),
            total_quantity: total_quantity.clone(),
        };

        if let Err(e) = event_sender
            .send(Event::WorkOrderAverageCostCalculated {
                product_id: self.product_id,
                average_cost: RustDecimal::from_str(&average_cost.to_string())
                    .unwrap_or(RustDecimal::ZERO),
            })
            .await
        {
            AVERAGE_COST_CALCULATION_FAILURES.inc();
            error!(
                "Failed to send AverageCostCalculated event for product {}: {}",
                self.product_id, e
            );
            return Err(ServiceError::EventError(e.to_string()));
        }
        AVERAGE_COST_CALCULATIONS.inc();
        info!(
            product_id = %self.product_id,
            average_cost = %result.average_cost,
            total_amount = %result.total_amount,
            total_quantity = %result.total_quantity,
            "Average cost calculated successfully"
        );
        Ok(result)
    }
}

impl CalculateAverageCostCommand {
    async fn get_purchases(
        &self,
        db: &DatabaseConnection,
    ) -> Result<Vec<purchase_order_entity::Model>, ServiceError> {
        purchase_order_entity::Entity::find()
            .filter(
                Condition::all()
                    .add(purchase_order_entity::Column::Id.is_not_null())
                    .add(
                        purchase_order_entity::Column::OrderDate
                            .between(self.start_date, self.end_date),
                    ),
            )
            .all(db)
            .await
            .map_err(|e| {
                AVERAGE_COST_CALCULATION_FAILURES.inc();
                error!(
                    "Failed to fetch purchase orders for product {}: {}",
                    self.product_id, e
                );
                ServiceError::db_error(e)
            })
    }

    async fn get_inventory(
        &self,
        db: &DatabaseConnection,
    ) -> Result<Vec<inventory_item_entity::Model>, ServiceError> {
        let start: DateTime<Utc> = DateTime::from_naive_utc_and_offset(self.start_date, Utc);
        let end: DateTime<Utc> = DateTime::from_naive_utc_and_offset(self.end_date, Utc);
        inventory_item_entity::Entity::find()
            .filter(
                Condition::all()
                    .add(inventory_item_entity::Column::ProductId.eq(self.product_id))
                    .add(inventory_item_entity::Column::UpdatedAt.between(start, end)),
            )
            .all(db)
            .await
            .map_err(|e| {
                AVERAGE_COST_CALCULATION_FAILURES.inc();
                error!(
                    "Failed to fetch inventory for product {}: {}",
                    self.product_id, e
                );
                ServiceError::db_error(e)
            })
    }

    async fn get_manufacturing_costs(
        &self,
        db: &DatabaseConnection,
    ) -> Result<Vec<manufacturing_order_entity::Model>, ServiceError> {
        manufacturing_order_entity::Entity::find()
            .filter(
                Condition::all()
                    // .add(manufacturing_order_entity::Column::ProductId.eq(self.product_id)) // ProductId field does not exist in manufacture_orders
                    .add(
                        manufacturing_order_entity::Column::CreatedOn
                            .between(self.start_date, self.end_date),
                    ),
            )
            .all(db)
            .await
            .map_err(|e| {
                AVERAGE_COST_CALCULATION_FAILURES.inc();
                error!(
                    "Failed to fetch manufacturing costs for product {}: {}",
                    self.product_id, e
                );
                ServiceError::db_error(e)
            })
    }
}
