use uuid::Uuid;
use std::str::FromStr;
use super::calculate_cogs_command::CalculateCOGSCommand;
use crate::commands::Command;
use crate::events::{Event, EventSender};
use crate::{
    db::DbPool,
    errors::ServiceError,
    models::{cogs_data_entity, work_order_entity},
};
use async_trait::async_trait;
use bigdecimal::BigDecimal;
use rust_decimal::Decimal as RustDecimal;
use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use futures::stream::{self, StreamExt};
use lazy_static::lazy_static;
use prometheus::IntCounter;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, Order, QueryFilter};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

lazy_static! {
    static ref MONTHLY_COGS_CALCULATIONS: IntCounter = IntCounter::new(
        "monthly_cogs_calculations_total",
        "Total number of monthly COGS calculations"
    )
    .expect("metric can be created");
    static ref MONTHLY_COGS_CALCULATION_FAILURES: IntCounter = IntCounter::new(
        "monthly_cogs_calculation_failures_total",
        "Total number of failed monthly COGS calculations"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CalculateMonthlyCOGSCommand {
    // This command doesn't need any parameters as it always calculates for the current month
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MonthlyCOGSResult {
    pub period: String,
    pub cogs: BigDecimal,
    pub average_cogs: BigDecimal,
    pub quantity_produced: i32,
    pub cogs_trend: BigDecimal,
}

#[async_trait::async_trait]
impl Command for CalculateMonthlyCOGSCommand {
    type Result = MonthlyCOGSResult;
    
    #[instrument(skip(db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();
        let now = Utc::now().naive_utc();
        let start_date = NaiveDateTime::new(
            NaiveDate::from_ymd_opt(now.year(), now.month(), 1).unwrap(),
            NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
        );
        let end_date = if now.month() == 12 {
            NaiveDateTime::new(
                NaiveDate::from_ymd_opt(now.year() + 1, 1, 1).unwrap(),
                NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            )
        } else {
            NaiveDateTime::new(
                NaiveDate::from_ymd_opt(now.year(), now.month() + 1, 1).unwrap(),
                NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            )
        };
        
        let work_orders = self
            .get_work_orders_for_period(&db, start_date, end_date)
            .await?;
        let (cogs, total_quantity_produced) = self
            .calculate_cogs_and_quantity(work_orders, &db_pool)
            .await?;
        let average_cogs = if total_quantity_produced > 0 {
            cogs.clone() / BigDecimal::from(total_quantity_produced)
        } else {
            BigDecimal::from(0)
        };
        
        let previous_period = format!(
            "{}-{:02}",
            if now.month() == 1 {
                now.year() - 1
            } else {
                now.year()
            },
            if now.month() == 1 {
                12
            } else {
                now.month() - 1
            }
        );
        
        let previous_cogs_data = self.get_previous_cogs_data(&db, &previous_period).await?;
        let cogs_trend = previous_cogs_data
            .map(|data| {
                ((cogs.clone() - BigDecimal::from_str(&data.cogs.to_string()).unwrap_or(BigDecimal::from(0))) / BigDecimal::from_str(&data.cogs.to_string()).unwrap_or(BigDecimal::from(1))) * BigDecimal::from(100)
            })
            .unwrap_or_else(|| BigDecimal::from(0));
        
        let current_period = format!("{}-{:02}", now.year(), now.month());
        let result = MonthlyCOGSResult {
            period: current_period.clone(),
            cogs: cogs.clone(),
            average_cogs: average_cogs.clone(),
            quantity_produced: total_quantity_produced,
            cogs_trend: cogs_trend.clone(),
        };
        
        self.store_cogs_data(&db, &result).await?;
        
        // Trigger an event indicating that monthly COGS was calculated
        if let Err(e) = event_sender
            .send(Event::MonthlyCOGSCalculated(current_period, RustDecimal::from_str(&cogs.to_string()).unwrap_or(RustDecimal::ZERO)))
            .await
        {
            MONTHLY_COGS_CALCULATION_FAILURES.inc();
            error!("Failed to send MonthlyCOGSCalculated event: {}", e);
            return Err(ServiceError::EventError(e.to_string()));
        }
        
        MONTHLY_COGS_CALCULATIONS.inc();
        info!(
            period = %result.period,
            cogs = %result.cogs,
            average_cogs = %result.average_cogs,
            quantity_produced = %result.quantity_produced,
            cogs_trend = %result.cogs_trend,
            "Monthly COGS calculated successfully"
        );
        Ok(result)
    }
}

impl CalculateMonthlyCOGSCommand {
    async fn get_work_orders_for_period(
        &self,
        db: &DatabaseConnection,
        start_date: NaiveDateTime,
        end_date: NaiveDateTime,
    ) -> Result<Vec<work_order_entity::Model>, ServiceError> {
        work_order_entity::Entity::find()
            .filter(work_order_entity::Column::CreatedAt.between(start_date, end_date))
            .all(db)
            .await
            .map_err(|e| {
                MONTHLY_COGS_CALCULATION_FAILURES.inc();
                error!("Failed to fetch work orders: {}", e);
                ServiceError::DatabaseError(e)
            })
    }

    async fn calculate_cogs_and_quantity(
        &self,
        work_orders: Vec<work_order_entity::Model>,
        db_pool: &Arc<DbPool>,
    ) -> Result<(BigDecimal, i32), ServiceError> {
        let results = stream::iter(work_orders)
            .map(|work_order| async {
                let command = CalculateCOGSCommand {
                    work_order.id: work_order.number.clone(),
                };
                command
                    .execute(db_pool.clone(), Arc::new(EventSender::new()))
                    .await
            })
            .buffer_unordered(10) // Process up to 10 work orders concurrently
            .collect::<Vec<_>>()
            .await;
        
        let mut cogs = BigDecimal::from(0);
        let mut total_quantity_produced = 0;
        
        for result in results {
            match result {
                Ok(cogs_result) => {
                    cogs += cogs_result.total_cost;
                    total_quantity_produced += cogs_result.quantity_produced;
                }
                Err(e) => {
                    error!("Failed to calculate COGS for a work order: {}", e);
                    // Decide how to handle individual failures. Here, we're continuing with the calculation.
                }
            }
        }
        
        Ok((cogs, total_quantity_produced))
    }

    async fn get_previous_cogs_data(
        &self,
        db: &DatabaseConnection,
        previous_period: &str,
    ) -> Result<Option<cogs_data_entity::Model>, ServiceError> {
        cogs_data_entity::Entity::find()
            .filter(cogs_data_entity::Column::Period.eq(previous_period))
            .one(db)
            .await
            .map_err(|e| {
                error!("Failed to fetch previous COGS data: {}", e);
                ServiceError::DatabaseError(e)
            })
    }

    async fn store_cogs_data(
        &self,
        db: &DatabaseConnection,
        result: &MonthlyCOGSResult,
    ) -> Result<(), ServiceError> {
        let new_cogs_data = cogs_data_entity::ActiveModel {
            period: sea_orm::ActiveValue::Set(result.period.clone()),
            cogs: sea_orm::ActiveValue::Set(RustDecimal::from_str(&result.cogs.to_string()).unwrap_or(RustDecimal::ZERO)),
            average_cogs: sea_orm::ActiveValue::Set(RustDecimal::from_str(&result.average_cogs.to_string()).unwrap_or(RustDecimal::ZERO)),
            quantity_produced: sea_orm::ActiveValue::Set(result.quantity_produced),
            cogs_trend: sea_orm::ActiveValue::Set(RustDecimal::from_str(&result.cogs_trend.to_string()).unwrap_or(RustDecimal::ZERO)),
            ..Default::default()
        };
        
        cog_entries::Entity::insert(new_cogs_data).exec(db).await.map_err(|e| {
            error!("Failed to store COGS data: {}", e);
            ServiceError::DatabaseError(e)
        })?;
        
        Ok(())
    }
}