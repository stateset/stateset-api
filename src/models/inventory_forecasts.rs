use chrono::NaiveDate;
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "inventory_forecasts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub part_number: String,
    pub forecast_date: NaiveDate,
    pub forecast_quantity: i32,
    pub forecast_method: String,
    pub forecast_accuracy: Option<Decimal>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    // You can define relationships here if needed
    // For example, if there's a relation to an Inventory Item model:
    // #[sea_orm(
    //     belongs_to = "super::inventory_item::Entity",
    //     from = "Column::PartNumber",
    //     to = "super::inventory_item::Column::PartNumber"
    // )]
    // InventoryItem,
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn new(
        part_number: String,
        forecast_date: NaiveDate,
        forecast_quantity: i32,
        forecast_method: String,
    ) -> Self {
        Self {
            id: 0, // This will be set by the database
            part_number,
            forecast_date,
            forecast_quantity,
            forecast_method,
            forecast_accuracy: None,
        }
    }

    pub fn update_accuracy(&mut self, actual_quantity: i32) {
        let forecast = self.forecast_quantity as f64;
        let actual = actual_quantity as f64;
        let accuracy = (1.0 - (forecast - actual).abs() / actual) * 100.0;
        self.forecast_accuracy = Some(Decimal::from_f64(accuracy).unwrap_or(Decimal::ZERO));
    }

    pub fn is_accurate(&self, threshold: Decimal) -> bool {
        self.forecast_accuracy
            .map(|accuracy| accuracy >= threshold)
            .unwrap_or(false)
    }
}

#[derive(Debug, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String")]
pub enum ForecastMethod {
    #[sea_orm(string_value = "Moving Average")]
    MovingAverage,
    #[sea_orm(string_value = "Exponential Smoothing")]
    ExponentialSmoothing,
    #[sea_orm(string_value = "Linear Regression")]
    LinearRegression,
    #[sea_orm(string_value = "ARIMA")]
    Arima,
    #[sea_orm(string_value = "Machine Learning")]
    MachineLearning,
    #[sea_orm(string_value = "Manual")]
    Manual,
}
