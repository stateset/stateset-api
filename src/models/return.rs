use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::{Validate, ValidationError};

/// Enum representing the possible statuses of a return.
#[derive(Debug, Clone, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum ReturnStatus {
    #[sea_orm(string_value = "Requested")]
    Requested,
    #[sea_orm(string_value = "Approved")]
    Approved,
    #[sea_orm(string_value = "Rejected")]
    Rejected,
    #[sea_orm(string_value = "Received")]
    Received,
    #[sea_orm(string_value = "Refunded")]
    Refunded,
    #[sea_orm(string_value = "Cancelled")]
    Cancelled,
}

/// Enum representing the condition of the returned item.
#[derive(Debug, Clone, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum Condition {
    #[sea_orm(string_value = "New")]
    New,
    #[sea_orm(string_value = "Used")]
    Used,
    #[sea_orm(string_value = "Damaged")]
    Damaged,
    #[sea_orm(string_value = "Defective")]
    Defective,
}

/// Enum representing actions needed for the return.
#[derive(Debug, Clone, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum ActionNeeded {
    #[sea_orm(string_value = "None")]
    None,
    #[sea_orm(string_value = "Inspection")]
    Inspection,
    #[sea_orm(string_value = "Refund")]
    Refund,
    #[sea_orm(string_value = "Replacement")]
    Replacement,
}

/// The `returns` table.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "returns")]
pub struct Model {
    /// Primary key: Unique identifier for the return.
    #[sea_orm(primary_key)]
    pub id: Uuid,

    /// Timestamp when the return was created.
    pub created_date: DateTime<Utc>,

    /// Total amount to be refunded.
    #[validate(custom = "validate_decimal_non_negative")]
    pub amount: Decimal,

    /// Action needed for the return.
    pub action_needed: ActionNeeded,

    /// Condition of the returned item.
    pub condition: Condition,

    /// Customer's email address.
    #[validate(email(message = "Invalid email format"))]
    pub customer_email: String,

    /// Identifier for the customer.
    pub customer_id: Uuid,

    /// Description of the return.
    #[validate(length(max = 1000, message = "Description too long"))]
    pub description: Option<String>,

    /// Identifier for who entered the return.
    pub entered_by: Option<Uuid>,

    /// Flat rate shipping cost refunded.
    #[validate(custom = "validate_decimal_non_negative")]
    pub flat_rate_shipping: Decimal,

    /// Date when the original order was placed.
    pub order_date: DateTime<Utc>,

    /// Identifier for the original order.
    pub order_id: Uuid,

    /// Category of the reason for the return.
    #[validate(length(max = 255, message = "Reason category too long"))]
    pub reason_category: Option<String>,

    /// Reported condition by the customer.
    pub reported_condition: Option<Condition>,

    /// Date when the return was requested.
    pub requested_date: DateTime<Utc>,

    /// Return Merchandise Authorization (RMA) number.
    #[validate(length(min = 1, message = "RMA cannot be empty"))]
    pub rma: String,

    /// Serial number of the returned item.
    #[validate(length(max = 100, message = "Serial number too long"))]
    pub serial_number: Option<String>,

    /// Date when the item was shipped back.
    pub shipped_date: Option<DateTime<Utc>>,

    /// Current status of the return.
    pub status: ReturnStatus,

    /// Tax amount refunded.
    #[validate(custom = "validate_decimal_non_negative")]
    pub tax_refunded: Decimal,

    /// Total amount refunded to the customer.
    #[validate(custom = "validate_decimal_non_negative")]
    pub total_refunded: Decimal,

    /// Tracking number for the return shipment.
    #[validate(length(max = 100, message = "Tracking number too long"))]
    pub tracking_number: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    /// A return has many return line items.
    #[sea_orm(has_many = "super::return_line_item::Entity")]
    ReturnLineItems,

    /// A return belongs to an order.
    #[sea_orm(
        belongs_to = "super::order::Entity",
        from = "Column::OrderId",
        to = "super::order::Column::Id",
        on_update = "Cascade",
        on_delete = "Restrict"
    )]
    Order,
}

impl Related<super::return_line_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ReturnLineItems.def()
    }
}

impl Related<super::order::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Order.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Implementation block for the `Return` model.
impl Model {
    /// Creates a new return request with the specified parameters.
    ///
    /// # Arguments
    ///
    /// * `order_id` - Identifier of the original order.
    /// * `customer_id` - Identifier of the customer.
    /// * `customer_email` - Email of the customer.
    /// * `amount` - Total amount to be refunded.
    /// * `rma` - Return Merchandise Authorization number.
    ///
    /// # Errors
    ///
    /// Returns `ValidationError` if the provided data does not meet validation criteria.
    pub fn new(
        order_id: Uuid,
        customer_id: Uuid,
        customer_email: String,
        amount: Decimal,
        rma: String,
    ) -> Result<Self, ValidationError> {
        let now = Utc::now();
        let return_request = Self {
            id: Uuid::new_v4(),
            created_date: now,
            amount,
            action_needed: ActionNeeded::None,
            condition: Condition::New,
            customer_email,
            customer_id,
            description: None,
            entered_by: None,
            flat_rate_shipping: Decimal::new(0, 2),
            order_date: now, // Ideally, set this to the actual order date from the order entity
            order_id,
            reason_category: None,
            reported_condition: None,
            requested_date: now,
            rma,
            serial_number: None,
            shipped_date: None,
            status: ReturnStatus::Requested,
            tax_refunded: Decimal::new(0, 2),
            total_refunded: Decimal::new(0, 2),
            tracking_number: None,
        };
        return_request.validate().map_err(|_| ValidationError::new("Return validation failed"))?;
        Ok(return_request)
    }

    /// Updates the status of the return.
    ///
    /// # Arguments
    ///
    /// * `new_status` - The new status to set for the return.
    ///
    /// # Errors
    ///
    /// Returns an error string if attempting to update a final status.
    pub fn update_status(&mut self, new_status: ReturnStatus) -> Result<(), String> {
        if self.status.is_final() {
            return Err("Cannot update status of a finalized return".into());
        }
        self.status = new_status;
        // Note: updated_date field not present in Return model
        Ok(())
    }

    /// Sets the tracking number for the return shipment.
    ///
    /// # Arguments
    ///
    /// * `tracking_number` - The tracking number to set.
    pub fn set_tracking_number(&mut self, tracking_number: String) {
        self.tracking_number = Some(tracking_number);
        // Note: updated_date field not present in Return model
    }

    /// Marks the return as shipped.
    pub fn mark_as_shipped(&mut self) {
        self.status = ReturnStatus::Approved;
        self.shipped_date = Some(Utc::now());
        // Note: updated_date field not present in Return model
    }

    /// Calculates the total refunded amount.
    ///
    /// This method sums up the `amount`, `flat_rate_shipping`, and `tax_refunded`.
    pub fn calculate_total_refunded(&mut self) {
        self.total_refunded = self.amount + self.flat_rate_shipping + self.tax_refunded;
        // Note: updated_date field not present in Return model
    }

    // Additional methods as needed...
}

impl ReturnStatus {
    /// Checks if the status is final and cannot be changed.
    pub fn is_final(&self) -> bool {
        matches!(self, ReturnStatus::Rejected | ReturnStatus::Refunded | ReturnStatus::Cancelled)
    }

    /// Returns the status as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            ReturnStatus::Requested => "Requested",
            ReturnStatus::Approved => "Approved",
            ReturnStatus::Rejected => "Rejected",
            ReturnStatus::Received => "Received",
            ReturnStatus::Refunded => "Refunded",
            ReturnStatus::Cancelled => "Cancelled",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use sea_orm::{ActiveValue::Set, DbBackend, EntityTrait, QueryTrait};

    /// Helper function to create a valid return.
    fn create_valid_return() -> Model {
        Model::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "customer@example.com".to_string(),
            dec!(150.00),
            "RMA123456".to_string(),
        )
        .expect("Failed to create valid return")
    }

    #[tokio::test]
    async fn test_return_creation() {
        let return_request = create_valid_return();
        assert!(return_request.validate().is_ok());
        assert_eq!(return_request.status, ReturnStatus::Requested);
        assert_eq!(return_request.amount, dec!(150.00));
        assert_eq!(return_request.rma, "RMA123456");
        assert!(return_request.created_date <= Utc::now());
    }

    #[tokio::test]
    async fn test_return_validation_failure() {
        // Create a return with invalid email and negative amount
        let return_request = Model {
            id: Uuid::new_v4(),
            created_date: Utc::now(),
            amount: dec!(-50.00),
            action_needed: ActionNeeded::Inspection,
            condition: Condition::Damaged,
            customer_email: "invalid_email".to_string(),
            customer_id: Uuid::new_v4(),
            description: Some("Item was damaged upon arrival.".to_string()),
            entered_by: Some(Uuid::new_v4()),
            flat_rate_shipping: dec!(-10.00),
            order_date: Utc::now(),
            order_id: Uuid::new_v4(),
            reason_category: Some("Damaged".to_string()),
            reported_condition: Some(Condition::Damaged),
            requested_date: Utc::now(),
            rma: "".to_string(), // Invalid RMA
            serial_number: Some("SN1234567890".to_string()),
            shipped_date: None,
            status: ReturnStatus::Requested,
            tax_refunded: dec!(-5.00),
            total_refunded: dec!(0.00),
            tracking_number: Some("TRK1234567890".to_string()),
        };

        let validation = return_request.validate();
        assert!(validation.is_err());

        if let Err(e) = validation {
            assert!(e.field_errors().contains_key("amount"));
            assert!(e.field_errors().contains_key("customer_email"));
            assert!(e.field_errors().contains_key("flat_rate_shipping"));
            assert!(e.field_errors().contains_key("rma"));
            assert!(e.field_errors().contains_key("tax_refunded"));
        }
    }

    #[tokio::test]
    async fn test_return_status_update() {
        let mut return_request = create_valid_return();
        assert_eq!(return_request.status, ReturnStatus::Requested);

        // Update status to Approved
        let result = return_request.update_status(ReturnStatus::Approved);
        assert!(result.is_ok());
        assert_eq!(return_request.status, ReturnStatus::Approved);
        // Note: updated_date field not present in Return model

        // Attempt to update status to Refunded (final status)
        let result = return_request.update_status(ReturnStatus::Refunded);
        assert!(result.is_ok());
        assert_eq!(return_request.status, ReturnStatus::Refunded);

        // Attempt to update status after finalization
        let result = return_request.update_status(ReturnStatus::Rejected);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Cannot update status of a finalized return"
        );
    }

    // NOTE: This test is disabled because MockDatabase and MockExecResult 
    // are no longer available in SeaORM 1.0.0
    // #[tokio::test]
    // async fn test_return_relationships() {
    //     // Mock database interactions using SeaORM's MockDatabase
    //     let db = MockDatabase::new(DbBackend::Postgres)
    //         .append_exec_results(vec![
    //             MockExecResult::new_with_affected_rows(1), // Insert return
    //         ])
    //         .into_connection();

    //     let return_request = create_valid_return();
    //     let active_model: ActiveModel = return_request.clone().into();

    //     // Simulate inserting the return
    //     let insert_return = Entity::insert(active_model).exec(&db).await;
    //     assert!(insert_return.is_ok());

    //     // Return relationship test completed
    // }
}

/// Custom validator for decimal values to ensure they are non-negative
fn validate_decimal_non_negative(value: &Decimal) -> Result<(), ValidationError> {
    if *value < Decimal::ZERO {
        return Err(ValidationError::new("Amount must be non-negative"));
    }
    Ok(())
}
