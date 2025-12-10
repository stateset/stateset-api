use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::{Validate, ValidationError};
/// Enum representing the possible statuses of a return line item.
#[derive(Debug, Clone, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum ReturnLineItemStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,
    #[sea_orm(string_value = "Inspected")]
    Inspected,
    #[sea_orm(string_value = "Approved")]
    Approved,
    #[sea_orm(string_value = "Rejected")]
    Rejected,
    #[sea_orm(string_value = "Processed")]
    Processed,
}

/// The `return_line_items` table.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "return_line_items")]
pub struct Model {
    /// Primary key: Unique identifier for the return line item.
    #[sea_orm(primary_key)]
    pub id: Uuid,

    /// Identifier for the return this line item belongs to.
    pub r#return: Uuid,

    /// Product identifier for the returned item.
    pub product_id: Uuid,

    /// Quantity of the product being returned.
    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    pub quantity: u32,

    /// Sale price per unit in cents.
    #[validate(custom = "validate_decimal_non_negative")]
    pub sale_price: Decimal,

    /// Original price per unit in cents before any discounts.
    #[validate(custom = "validate_decimal_non_negative")]
    pub original_price: Decimal,

    /// Discount applied by the seller in cents.
    #[validate(custom = "validate_decimal_non_negative")]
    pub seller_discount: Decimal,

    /// Unit of measurement (e.g., pcs, kg).
    #[validate(length(min = 1, message = "Unit cannot be empty"))]
    pub unit: String,

    /// Identifier for the product.
    #[validate(length(min = 1, message = "Product code cannot be empty"))]
    pub product_code: String,

    /// Brand of the product.
    #[validate(length(max = 100, message = "Brand name too long"))]
    pub brand: String,

    /// Stock code of the product.
    #[validate(length(max = 100, message = "Stock code too long"))]
    pub stock_code: String,

    /// Size of the product.
    #[validate(length(max = 20, message = "Size too long"))]
    pub size: String,

    /// Color of the product.
    #[validate(length(max = 20, message = "Color too long"))]
    pub color: String,

    /// Weight of the product.
    #[validate(range(min = 0, message = "Weight cannot be negative"))]
    pub weight: Option<f64>,

    /// Current status of the return line item.
    pub status: ReturnLineItemStatus,

    /// Reason for the return.
    #[validate(length(max = 500, message = "Reason too long"))]
    pub reason: Option<String>,

    /// Condition of the returned item.
    #[validate(length(max = 100, message = "Condition description too long"))]
    pub condition: Option<String>,

    /// Timestamp when the return line item was created.
    pub created_date: DateTime<Utc>,

    /// Timestamp when the return line item was last updated.
    pub updated_date: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::r#return::Entity",
        from = "Column::Return",
        to = "super::r#return::Column::Id"
    )]
    Return,
}

impl Related<super::r#return::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Return.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Calculates the total refund amount for this line item
    pub fn calculate_refund_amount(&self) -> Decimal {
        let base_amount = self.sale_price * Decimal::from(self.quantity);
        base_amount - self.seller_discount
    }

    /// Applies a discount to the line item
    pub fn apply_discount(&mut self, discount: Decimal) -> Result<(), String> {
        if discount < Decimal::ZERO {
            return Err("Discount cannot be negative".to_string());
        }

        let max_discount = self.sale_price * Decimal::from(self.quantity);
        let new_total_discount = self.seller_discount + discount;

        if new_total_discount > max_discount {
            return Err("Total discount cannot exceed the item's sale price".to_string());
        }

        self.seller_discount = new_total_discount;
        Ok(())
    }

    /// Checks if the line item is eligible for return based on various criteria
    pub fn is_eligible_for_return(&self) -> bool {
        // Basic eligibility check - can be extended with more business rules
        matches!(
            self.status,
            ReturnLineItemStatus::Pending | ReturnLineItemStatus::Inspected
        )
    }

    /// Updates the status of the return line item
    pub fn update_status(&mut self, new_status: ReturnLineItemStatus) {
        self.status = new_status;
        self.updated_date = Some(Utc::now());
    }
}

/// Custom validator for decimal values to ensure they are non-negative
fn validate_decimal_non_negative(value: &Decimal) -> Result<(), ValidationError> {
    if *value < Decimal::ZERO {
        return Err(ValidationError::new("Amount must be non-negative"));
    }
    Ok(())
}

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;

    #[test]
    fn test_calculate_refund_amount() {
        let line_item = Model {
            id: Uuid::new_v4(),
            r#return: Uuid::new_v4(),
            product_id: Uuid::new_v4(),
            quantity: 2,
            sale_price: dec!(50.00),
            original_price: dec!(60.00),
            seller_discount: dec!(10.00),
            unit: "pcs".to_string(),
            product_code: "PROD123".to_string(),
            brand: "TestBrand".to_string(),
            stock_code: "STK123".to_string(),
            size: "M".to_string(),
            color: "Blue".to_string(),
            weight: Some(1.5),
            status: ReturnLineItemStatus::Pending,
            reason: Some("Defective".to_string()),
            condition: Some("Damaged".to_string()),
            created_date: Utc::now(),
            updated_date: None,
        };

        let refund_amount = line_item.calculate_refund_amount();
        assert_eq!(refund_amount, dec!(90.00)); // (50.00 * 2) - 10.00
    }

    #[test]
    fn test_apply_discount() {
        let mut line_item = Model {
            id: Uuid::new_v4(),
            r#return: Uuid::new_v4(),
            product_id: Uuid::new_v4(),
            quantity: 1,
            sale_price: dec!(100.00),
            original_price: dec!(120.00),
            seller_discount: dec!(20.00),
            unit: "pcs".to_string(),
            product_code: "PROD123".to_string(),
            brand: "TestBrand".to_string(),
            stock_code: "STK123".to_string(),
            size: "L".to_string(),
            color: "Red".to_string(),
            weight: Some(2.0),
            status: ReturnLineItemStatus::Pending,
            reason: Some("Not as described".to_string()),
            condition: Some("Good".to_string()),
            created_date: Utc::now(),
            updated_date: None,
        };

        // Apply a valid discount
        let result = line_item.apply_discount(dec!(15.00));
        assert!(result.is_ok());
        assert_eq!(line_item.seller_discount, dec!(35.00));

        // Attempt to apply a negative discount
        let result = line_item.apply_discount(dec!(-10.00));
        assert!(result.is_err());
        assert_eq!(line_item.seller_discount, dec!(35.00));

        // Attempt to apply a discount that would result in a negative total discount
        let result = line_item.apply_discount(dec!(-40.00));
        assert!(result.is_err());
        assert_eq!(line_item.seller_discount, dec!(35.00));
    }
}
