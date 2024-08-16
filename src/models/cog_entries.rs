use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::NaiveDate;
use rust_decimal::Decimal;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "cogs_entries")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub period: String,
    pub product: String,
    #[validate(range(min = 0, message = "Quantity sold must be non-negative"))]
    pub quantity_sold: i32,
    #[validate(range(min = 0, message = "COGS must be non-negative"))]
    pub cogs: Decimal,
    #[validate(range(min = 0, message = "Average cost must be non-negative"))]
    pub average_cost: Decimal,
    #[validate(range(min = 0, message = "Ending inventory quantity must be non-negative"))]
    pub ending_inventory_quantity: Option<Decimal>,
    pub ending_inventory_value: Option<Decimal>,
    pub currency: Option<String>,
    pub exchange_rate_id: Option<i32>,
    pub sale_transaction_id: Option<i32>,
    pub unit_selling_price: Option<Decimal>,
    pub gross_sales: Option<Decimal>,
    pub gross_profit: Option<Decimal>,
    pub gross_margin_percentage: Option<Decimal>,
    pub cogs_percentage: Option<Decimal>,
    pub cogs_method: Option<String>,
    pub product_category: Option<String>,
    pub sales_channel: Option<String>,
    pub customer_segment: Option<String>,
    pub sale_date: Option<NaiveDate>,
    pub is_return: Option<bool>,
    pub return_reason: Option<String>,
    pub cost_center: Option<String>,
    pub supplier_id: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn new(
        period: String,
        product: String,
        quantity_sold: i32,
        cogs: Decimal,
        average_cost: Decimal,
    ) -> Result<Self, ValidationError> {
        let cogs_entry = Self {
            id: 0, // This will be set by the database
            period,
            product,
            quantity_sold,
            cogs,
            average_cost,
            ending_inventory_quantity: None,
            ending_inventory_value: None,
            currency: None,
            exchange_rate_id: None,
            sale_transaction_id: None,
            unit_selling_price: None,
            gross_sales: None,
            gross_profit: None,
            gross_margin_percentage: None,
            cogs_percentage: None,
            cogs_method: None,
            product_category: None,
            sales_channel: None,
            customer_segment: None,
            sale_date: None,
            is_return: Some(false),
            return_reason: None,
            cost_center: None,
            supplier_id: None,
        };
        cogs_entry.validate()?;
        Ok(cogs_entry)
    }

    pub fn calculate_gross_profit(&mut self) -> Result<(), String> {
        match (self.gross_sales, self.cogs) {
            (Some(sales), cogs) => {
                self.gross_profit = Some(sales - cogs);
                Ok(())
            }
            _ => Err("Gross sales not set".into()),
        }
    }

    pub fn calculate_gross_margin_percentage(&mut self) -> Result<(), String> {
        match (self.gross_profit, self.gross_sales) {
            (Some(profit), Some(sales)) if sales != Decimal::ZERO => {
                self.gross_margin_percentage = Some((profit / sales) * Decimal::ONE_HUNDRED);
                Ok(())
            }
            _ => Err("Gross profit or sales not set, or sales is zero".into()),
        }
    }

    pub fn calculate_cogs_percentage(&mut self) -> Result<(), String> {
        match (self.cogs, self.gross_sales) {
            (cogs, Some(sales)) if sales != Decimal::ZERO => {
                self.cogs_percentage = Some((cogs / sales) * Decimal::ONE_HUNDRED);
                Ok(())
            }
            _ => Err("Gross sales not set or is zero".into()),
        }
    }

    pub fn set_as_return(&mut self, reason: String) {
        self.is_return = Some(true);
        self.return_reason = Some(reason);
    }
}