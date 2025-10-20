use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use sea_orm::QueryOrder;
use sea_orm::{DatabaseConnection, Set};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
use uuid::Uuid;
use validator::{Validate, ValidationError};

/// Custom validator for non-negative decimal values
fn validate_non_negative_decimal(value: &Decimal) -> Result<(), ValidationError> {
    if *value >= Decimal::ZERO {
        Ok(())
    } else {
        Err(ValidationError::new("must be non-negative"))
    }
}

/// Custom validator for optional non-negative decimal values
fn validate_optional_non_negative_decimal(value: &Decimal) -> Result<(), ValidationError> {
    validate_non_negative_decimal(value)
}

/// COGS calculation method enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum CogsMethod {
    #[sea_orm(string_value = "FIFO")]
    Fifo,

    #[sea_orm(string_value = "LIFO")]
    Lifo,

    #[sea_orm(string_value = "WeightedAverage")]
    WeightedAverage,

    #[sea_orm(string_value = "SpecificIdentification")]
    SpecificIdentification,
}

impl fmt::Display for CogsMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CogsMethod::Fifo => write!(f, "FIFO"),
            CogsMethod::Lifo => write!(f, "LIFO"),
            CogsMethod::WeightedAverage => write!(f, "Weighted Average"),
            CogsMethod::SpecificIdentification => write!(f, "Specific Identification"),
        }
    }
}

/// Custom error type for COGS operations
#[derive(Error, Debug)]
pub enum CogsError {
    #[error("Database error: {0}")]
    Database(#[from] DbErr),

    #[error("Validation error: {0}")]
    Validation(#[from] validator::ValidationError),

    #[error("Calculation error: {0}")]
    Calculation(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// COGS Entry entity model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "cogs_entries")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,

    #[validate(length(min = 1, max = 50, message = "Period must be between 1-50 characters"))]
    pub period: String,

    #[validate(length(
        min = 1,
        max = 100,
        message = "Product must be between 1-100 characters"
    ))]
    pub product: String,

    #[validate(range(min = 0, message = "Quantity sold must be non-negative"))]
    pub quantity_sold: i32,

    #[validate(custom = "validate_non_negative_decimal")]
    pub cogs: Decimal,

    #[validate(custom = "validate_non_negative_decimal")]
    pub average_cost: Decimal,

    #[validate(custom = "validate_optional_non_negative_decimal")]
    pub ending_inventory_quantity: Option<Decimal>,

    pub ending_inventory_value: Option<Decimal>,

    #[validate(length(min = 3, max = 3, message = "Currency must be a 3-letter ISO code"))]
    pub currency: Option<String>,

    pub exchange_rate_id: Option<i32>,
    pub sale_transaction_id: Option<i32>,
    pub unit_selling_price: Option<Decimal>,
    pub gross_sales: Option<Decimal>,
    pub gross_profit: Option<Decimal>,
    pub gross_margin_percentage: Option<Decimal>,
    pub cogs_percentage: Option<Decimal>,
    pub cogs_method: Option<CogsMethod>,

    #[validate(length(max = 100))]
    pub product_category: Option<String>,

    #[validate(length(max = 100))]
    pub sales_channel: Option<String>,

    #[validate(length(max = 100))]
    pub customer_segment: Option<String>,

    pub sale_date: Option<NaiveDate>,
    pub is_return: Option<bool>,

    #[validate(length(max = 200))]
    pub return_reason: Option<String>,

    #[validate(length(max = 100))]
    pub cost_center: Option<String>,

    pub supplier_id: Option<i32>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Database relations
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::exchange_rate::Entity",
        from = "Column::ExchangeRateId",
        to = "super::exchange_rate::Column::Id",
        on_delete = "NoAction"
    )]
    ExchangeRate,

    #[sea_orm(
        belongs_to = "super::sale_transaction::Entity",
        from = "Column::SaleTransactionId",
        to = "super::sale_transaction::Column::Id",
        on_delete = "SetNull"
    )]
    SaleTransaction,

    #[sea_orm(
        belongs_to = "super::suppliers::Entity",
        from = "Column::SupplierId",
        to = "super::suppliers::Column::Id",
        on_delete = "SetNull"
    )]
    Supplier,
}

impl Related<super::exchange_rate::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ExchangeRate.def()
    }
}

impl Related<super::sale_transaction::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SaleTransaction.def()
    }
}

impl Related<super::suppliers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Supplier.def()
    }
}

/// Active model behavior for database hooks
#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C: ConnectionTrait>(self, _db: &C, insert: bool) -> Result<Self, DbErr> {
        let mut active_model = self;
        if insert {
            active_model.set_id_if_needed();
        }
        Ok(active_model)
    }
}

impl ActiveModel {
    fn set_id_if_needed(&mut self) {
        if self.id.is_not_set() {
            // i32 primary key: let the database assign it
        }
    }
}

impl Model {
    /// Create a new COGS entry with required fields
    pub fn new(
        period: String,
        product: String,
        quantity_sold: i32,
        cogs: Decimal,
        average_cost: Decimal,
        sale_date: Option<NaiveDate>,
    ) -> Result<Self, validator::ValidationError> {
        let now = Utc::now();
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
            sale_date,
            is_return: Some(false),
            return_reason: None,
            cost_center: None,
            supplier_id: None,
            created_at: now,
            updated_at: now,
        };
        cogs_entry
            .validate()
            .map_err(|_| ValidationError::new("Validation failed"))?;
        Ok(cogs_entry)
    }

    /// Set optional inventory information
    pub fn with_inventory(
        mut self,
        ending_inventory_quantity: Decimal,
        ending_inventory_value: Decimal,
    ) -> Self {
        self.ending_inventory_quantity = Some(ending_inventory_quantity);
        self.ending_inventory_value = Some(ending_inventory_value);
        self
    }

    /// Set sales information
    pub fn with_sales_info(
        mut self,
        unit_selling_price: Decimal,
        sales_channel: Option<String>,
        customer_segment: Option<String>,
    ) -> Result<Self, validator::ValidationError> {
        self.unit_selling_price = Some(unit_selling_price);

        // Calculate gross sales based on unit price and quantity
        self.gross_sales = Some(unit_selling_price * Decimal::from(self.quantity_sold));

        self.sales_channel = sales_channel;
        self.customer_segment = customer_segment;

        // Recalculate dependent values
        self.calculate_gross_profit()
            .map_err(|_| ValidationError::new("Failed to calculate gross profit"))?;
        self.calculate_gross_margin_percentage()
            .map_err(|_| ValidationError::new("Failed to calculate gross margin"))?;
        self.calculate_cogs_percentage()
            .map_err(|_| ValidationError::new("Failed to calculate COGS percentage"))?;

        self.validate()
            .map_err(|_| ValidationError::new("Validation failed"))?;
        Ok(self)
    }

    /// Calculate gross profit from gross sales and COGS
    pub fn calculate_gross_profit(&mut self) -> Result<(), CogsError> {
        match self.gross_sales {
            Some(sales) => {
                self.gross_profit = Some(sales - self.cogs);
                Ok(())
            }
            None => Err(CogsError::Calculation("Gross sales not set".to_string())),
        }
    }

    /// Calculate gross margin percentage
    pub fn calculate_gross_margin_percentage(&mut self) -> Result<(), CogsError> {
        match (self.gross_profit, self.gross_sales) {
            (Some(profit), Some(sales)) if !sales.is_zero() => {
                self.gross_margin_percentage = Some((profit / sales) * Decimal::ONE_HUNDRED);
                Ok(())
            }
            (None, _) => Err(CogsError::Calculation(
                "Gross profit not calculated".to_string(),
            )),
            (_, None) => Err(CogsError::Calculation("Gross sales not set".to_string())),
            (_, Some(sales)) if sales.is_zero() => {
                Err(CogsError::Calculation("Sales value is zero".to_string()))
            }
            _ => Err(CogsError::Internal(
                "Unexpected error in margin calculation".to_string(),
            )),
        }
    }

    /// Calculate COGS as percentage of gross sales
    pub fn calculate_cogs_percentage(&mut self) -> Result<(), CogsError> {
        match self.gross_sales {
            Some(sales) if !sales.is_zero() => {
                self.cogs_percentage = Some((self.cogs / sales) * Decimal::ONE_HUNDRED);
                Ok(())
            }
            None => Err(CogsError::Calculation("Gross sales not set".to_string())),
            Some(sales) if sales.is_zero() => {
                Err(CogsError::Calculation("Sales value is zero".to_string()))
            }
            _ => Err(CogsError::Internal(
                "Unexpected error in COGS percentage calculation".to_string(),
            )),
        }
    }

    /// Mark entry as a return with reason
    pub fn set_as_return(&mut self, reason: String) -> Result<(), validator::ValidationError> {
        self.is_return = Some(true);
        self.return_reason = Some(reason);

        // For returns, we negate quantities and amounts
        self.quantity_sold = -self.quantity_sold.abs();

        // Mark as a return by making gross sales negative
        if let Some(ref mut sales) = self.gross_sales {
            *sales = -sales.abs();
        }

        // Recalculate dependent values if gross sales exist
        if self.gross_sales.is_some() {
            let _ = self.calculate_gross_profit();
            let _ = self.calculate_gross_margin_percentage();
            let _ = self.calculate_cogs_percentage();
        }

        self.validate()
            .map_err(|_| ValidationError::new("Validation failed"))?;
        Ok(())
    }

    /// Save the COGS entry to database
    pub async fn save(&self, db: &DatabaseConnection) -> Result<Model, CogsError> {
        // Validate before saving
        self.validate()
            .map_err(|_| ValidationError::new("Validation failed"))?;

        let model: ActiveModel = self.clone().into();
        let result = match self.id {
            0 => model.insert(db).await?,
            _ => model.update(db).await?,
        };

        Ok(result)
    }

    /// Find COGS entries by period
    pub async fn find_by_period(
        db: &DatabaseConnection,
        period: &str,
    ) -> Result<Vec<Model>, DbErr> {
        Entity::find()
            .filter(Column::Period.eq(period))
            .order_by_asc(Column::Product)
            .all(db)
            .await
    }

    /// Find COGS entries by product
    pub async fn find_by_product(
        db: &DatabaseConnection,
        product: &str,
    ) -> Result<Vec<Model>, DbErr> {
        Entity::find()
            .filter(Column::Product.eq(product))
            .order_by_desc(Column::SaleDate)
            .all(db)
            .await
    }

    /// Calculate total COGS for a period
    pub async fn calculate_period_total_cogs(
        db: &DatabaseConnection,
        period: &str,
    ) -> Result<Decimal, DbErr> {
        let entries = Self::find_by_period(db, period).await?;

        let total = entries.iter().map(|entry| entry.cogs).sum();

        Ok(total)
    }

    /// Create a builder for COGS entry
    pub fn builder(
        period: String,
        product: String,
        quantity_sold: i32,
        cogs: Decimal,
        average_cost: Decimal,
    ) -> CogsEntryBuilder {
        CogsEntryBuilder {
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
            cogs_method: None,
            product_category: None,
            sales_channel: None,
            customer_segment: None,
            sale_date: None,
            cost_center: None,
            supplier_id: None,
        }
    }
}

/// Builder pattern for creating COGS entries
pub struct CogsEntryBuilder {
    period: String,
    product: String,
    quantity_sold: i32,
    cogs: Decimal,
    average_cost: Decimal,
    ending_inventory_quantity: Option<Decimal>,
    ending_inventory_value: Option<Decimal>,
    currency: Option<String>,
    exchange_rate_id: Option<i32>,
    sale_transaction_id: Option<i32>,
    unit_selling_price: Option<Decimal>,
    cogs_method: Option<CogsMethod>,
    product_category: Option<String>,
    sales_channel: Option<String>,
    customer_segment: Option<String>,
    sale_date: Option<NaiveDate>,
    cost_center: Option<String>,
    supplier_id: Option<i32>,
}

impl CogsEntryBuilder {
    /// Set inventory information
    pub fn with_inventory(mut self, quantity: Decimal, value: Decimal) -> Self {
        self.ending_inventory_quantity = Some(quantity);
        self.ending_inventory_value = Some(value);
        self
    }

    /// Set currency information
    pub fn with_currency(mut self, currency: String, exchange_rate_id: Option<i32>) -> Self {
        self.currency = Some(currency);
        self.exchange_rate_id = exchange_rate_id;
        self
    }

    /// Set sales information
    pub fn with_sales_info(
        mut self,
        unit_price: Decimal,
        transaction_id: Option<i32>,
        channel: Option<String>,
        segment: Option<String>,
    ) -> Self {
        self.unit_selling_price = Some(unit_price);
        self.sale_transaction_id = transaction_id;
        self.sales_channel = channel;
        self.customer_segment = segment;
        self
    }

    /// Set product information
    pub fn with_product_info(mut self, category: String, supplier_id: Option<i32>) -> Self {
        self.product_category = Some(category);
        self.supplier_id = supplier_id;
        self
    }

    /// Set COGS method
    pub fn with_cogs_method(mut self, method: CogsMethod) -> Self {
        self.cogs_method = Some(method);
        self
    }

    /// Set date information
    pub fn with_date(mut self, date: NaiveDate) -> Self {
        self.sale_date = Some(date);
        self
    }

    /// Set cost center
    pub fn with_cost_center(mut self, cost_center: String) -> Self {
        self.cost_center = Some(cost_center);
        self
    }

    /// Build the COGS entry
    pub fn build(self) -> Result<Model, validator::ValidationError> {
        let mut entry = Model::new(
            self.period,
            self.product,
            self.quantity_sold,
            self.cogs,
            self.average_cost,
            self.sale_date,
        )?;

        entry.ending_inventory_quantity = self.ending_inventory_quantity;
        entry.ending_inventory_value = self.ending_inventory_value;
        entry.currency = self.currency;
        entry.exchange_rate_id = self.exchange_rate_id;
        entry.sale_transaction_id = self.sale_transaction_id;
        entry.unit_selling_price = self.unit_selling_price;
        entry.cogs_method = self.cogs_method;
        entry.product_category = self.product_category;
        entry.sales_channel = self.sales_channel;
        entry.customer_segment = self.customer_segment;
        entry.cost_center = self.cost_center;
        entry.supplier_id = self.supplier_id;

        // Calculate derived values if we have unit price
        if let Some(unit_price) = self.unit_selling_price {
            entry.gross_sales = Some(unit_price * Decimal::from(self.quantity_sold));
            let _ = entry.calculate_gross_profit();
            let _ = entry.calculate_gross_margin_percentage();
            let _ = entry.calculate_cogs_percentage();
        }

        entry
            .validate()
            .map_err(|_| ValidationError::new("Validation failed"))?;
        Ok(entry)
    }
}
