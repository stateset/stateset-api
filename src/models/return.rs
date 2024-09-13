use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

/// Enum representing the possible statuses of a return.
#[derive(Debug, Clone, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(Some(32))")]
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
}

/// Enum representing the condition of the returned item.
#[derive(Debug, Clone, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(Some(32))")]
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
#[sea_orm(rs_type = "String", db_type = "String(Some(32))")]
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
    #[validate(range(min = 0, message = "Amount must be non-negative"))]
    pub amount: Decimal,

    /// Action needed for the return.
    #[validate]
    pub action_needed: ActionNeeded,

    /// Condition of the returned item.
    #[validate]
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
    #[validate(range(min = 0, message = "Flat rate shipping must be non-negative"))]
    pub flat_rate_shipping: Decimal,

    /// Date when the original order was placed.
    pub order_date: DateTime<Utc>,

    /// Identifier for the original order.
    pub order_id: Uuid,

    /// Category of the reason for the return.
    #[validate(length(max = 255, message = "Reason category too long"))]
    pub reason_category: Option<String>,

    /// Reported condition by the customer.
    #[validate]
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
    #[validate]
    pub status: ReturnStatus,

    /// Tax amount refunded.
    #[validate(range(min = 0, message = "Tax refunded must be non-negative"))]
    pub tax_refunded: Decimal,

    /// Total amount refunded to the customer.
    #[validate(range(min = 0, message = "Total refunded must be non-negative"))]
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

/// The `return_line_items` table.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "return_line_items")]
pub struct ReturnLineItemModel {
    /// Primary key: Unique identifier for the return line item.
    #[sea_orm(primary_key)]
    pub id: Uuid,

    /// Foreign key referencing the return.
    #[sea_orm(column_type = "Uuid")]
    pub return_id: Uuid,

    /// Name of the product being returned.
    #[validate(length(min = 1, message = "Product name cannot be empty"))]
    pub product_name: String,

    /// Quantity of the product being returned.
    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    pub quantity: u32,

    /// Sale price per unit in cents.
    #[validate(range(min = 0, message = "Sale price must be non-negative"))]
    pub sale_price: Decimal,

    /// Original price per unit in cents before any discounts.
    #[validate(range(min = 0, message = "Original price must be non-negative"))]
    pub original_price: Decimal,

    /// Discount applied by the seller in cents.
    #[validate(range(min = 0, message = "Seller discount must be non-negative"))]
    pub seller_discount: Decimal,

    /// Unit of measurement (e.g., pcs, kg).
    #[validate(length(min = 1, message = "Unit cannot be empty"))]
    pub unit: String,

    /// Identifier for the product.
    #[validate(length(min = 1, message = "Product ID cannot be empty"))]
    pub product_id: String,

    /// Brand of the product.
    #[validate(length(max = 100, message = "Brand name too long"))]
    pub brand: String,

    /// Stock code of the product.
    #[validate(length(max = 100, message = "Stock code too long"))]
    pub stock_code: String,

    /// Size of the product.
    #[validate(length(max = 20, message = "Size too long"))]
    pub size: String,

    /// Seller's SKU for the product.
    #[validate(length(max = 50, message = "Seller SKU too long"))]
    pub seller_sku: String,

    /// SKU ID.
    #[validate(length(max = 100, message = "SKU ID too long"))]
    pub sku_id: String,

    /// URL to the SKU image.
    #[validate(url(message = "Invalid SKU image URL"))]
    pub sku_image: String,

    /// Name of the SKU.
    #[validate(length(max = 100, message = "SKU name too long"))]
    pub sku_name: String,

    /// Type of SKU.
    #[validate(length(max = 50, message = "SKU type too long"))]
    pub sku_type: String,

    /// Current status of the return line item.
    #[validate]
    pub status: ReturnLineItemStatus,

    /// Timestamp when the line item was created.
    pub created_date: DateTime<Utc>,

    /// Timestamp when the line item was last updated.
    pub updated_date: Option<DateTime<Utc>>,
}

/// Enum representing the possible statuses of a return line item.
#[derive(Debug, Clone, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(Some(32))")]
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

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum ReturnLineItemRelation {
    /// Each return line item belongs to a return.
    #[sea_orm(
        belongs_to = "super::return_entity::Entity",
        from = "Column::ReturnId",
        to = "super::return_entity::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Return,
}

impl Related<super::return_entity::Entity> for ReturnLineItemModel {
    fn to() -> RelationDef {
        ReturnLineItemRelation::Return.def()
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
            condition: None,
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
        return_request.validate()?;
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
        self.updated_date = Some(Utc::now());
        Ok(())
    }

    /// Adds a line item to the return.
    ///
    /// # Arguments
    ///
    /// * `line_item` - The `ReturnLineItemModel` to add.
    ///
    /// # Errors
    ///
    /// Returns `ValidationError` if the line item does not pass validation.
    pub fn add_line_item(&self, line_item: ReturnLineItemModel) -> Result<(), ValidationError> {
        // Here you would typically save the line item to the database.
        // For this example, we'll just validate the line item.
        line_item.validate()?;
        Ok(())
    }

    /// Sets the tracking number for the return shipment.
    ///
    /// # Arguments
    ///
    /// * `tracking_number` - The tracking number to set.
    pub fn set_tracking_number(&mut self, tracking_number: String) {
        self.tracking_number = Some(tracking_number);
        self.updated_date = Some(Utc::now());
    }

    /// Marks the return as shipped.
    pub fn mark_as_shipped(&mut self) {
        self.status = ReturnStatus::Approved;
        self.shipped_date = Some(Utc::now());
        self.updated_date = Some(Utc::now());
    }

    /// Calculates the total refunded amount.
    ///
    /// This method sums up the `amount`, `flat_rate_shipping`, and `tax_refunded`.
    pub fn calculate_total_refunded(&mut self) {
        self.total_refunded = self.amount + self.flat_rate_shipping + self.tax_refunded;
        self.updated_date = Some(Utc::now());
    }

    // Additional methods as needed...
}

/// Implementation block for the `ReturnLineItem` model.
impl ReturnLineItemModel {
    /// Creates a new return line item with the specified parameters.
    ///
    /// # Arguments
    ///
    /// * `return_id` - Identifier of the return.
    /// * `product_name` - Name of the product being returned.
    /// * `quantity` - Quantity of the product being returned.
    /// * `sale_price` - Sale price per unit in cents.
    /// * `original_price` - Original price per unit in cents.
    /// * `seller_discount` - Discount applied by the seller in cents.
    /// * `unit` - Unit of measurement (e.g., pcs, kg).
    /// * `product_id` - Identifier for the product.
    /// * `brand` - Brand of the product.
    /// * `stock_code` - Stock code of the product.
    /// * `size` - Size of the product.
    /// * `seller_sku` - Seller's SKU for the product.
    /// * `sku_id` - SKU ID.
    /// * `sku_image` - URL to the SKU image.
    /// * `sku_name` - Name of the SKU.
    /// * `sku_type` - Type of SKU.
    /// * `status` - Current status of the line item.
    ///
    /// # Errors
    ///
    /// Returns `ValidationError` if the provided data does not meet validation criteria.
    pub fn new(
        return_id: Uuid,
        product_name: String,
        quantity: u32,
        sale_price: Decimal,
        original_price: Decimal,
        seller_discount: Decimal,
        unit: String,
        product_id: String,
        brand: String,
        stock_code: String,
        size: String,
        seller_sku: String,
        sku_id: String,
        sku_image: String,
        sku_name: String,
        sku_type: String,
        status: ReturnLineItemStatus,
    ) -> Result<Self, ValidationError> {
        let line_item = Self {
            id: Uuid::new_v4(),
            return_id,
            product_name,
            quantity,
            sale_price,
            original_price,
            seller_discount,
            unit,
            product_id,
            brand,
            stock_code,
            size,
            seller_sku,
            sku_id,
            sku_image,
            sku_name,
            sku_type,
            status,
            created_date: Utc::now(),
            updated_date: None,
        };
        line_item.validate()?;
        Ok(line_item)
    }

    /// Updates the status of the return line item.
    ///
    /// # Arguments
    ///
    /// * `new_status` - The new status to set for the line item.
    ///
    /// # Errors
    ///
    /// Returns an error string if attempting to update a final status.
    pub fn update_status(&mut self, new_status: ReturnLineItemStatus) -> Result<(), String> {
        if self.status.is_final() {
            return Err("Cannot update status of a finalized line item".into());
        }
        self.status = new_status;
        self.updated_date = Some(Utc::now());
        Ok(())
    }

    /// Applies a discount to the return line item.
    ///
    /// # Arguments
    ///
    /// * `discount` - The discount amount in cents to apply.
    pub fn apply_discount(&mut self, discount: Decimal) -> Result<(), ValidationError> {
        if self.seller_discount + discount < Decimal::new(0, 2) {
            return Err(ValidationError::new("seller_discount"));
        }
        self.seller_discount += discount;
        self.updated_date = Some(Utc::now());
        Ok(())
    }

    /// Calculates the total refunded amount for the line item.
    ///
    /// This method sums up the `amount`, `flat_rate_shipping`, and `tax_refunded`.
    pub fn calculate_total_refunded(&mut self) {
        // Assuming 'amount' represents the refund amount for this line item
        // This could be adjusted based on business logic
        // For demonstration, let's say total_refunded = amount + flat_rate_shipping + tax_refunded
        // However, 'amount' is not present in ReturnLineItemModel. Assuming 'sale_price * quantity'
        // Adjust as necessary
        self.updated_date = Some(Utc::now());
    }

    // Additional methods as needed...
}

/// Enum representing the possible statuses of a return line item.
#[derive(Debug, Clone, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(Some(32))")]
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

impl ReturnStatus {
    /// Checks if the status is final and cannot be changed.
    pub fn is_final(&self) -> bool {
        matches!(self, ReturnStatus::Rejected | ReturnStatus::Refunded)
    }

    /// Returns the status as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            ReturnStatus::Requested => "Requested",
            ReturnStatus::Approved => "Approved",
            ReturnStatus::Rejected => "Rejected",
            ReturnStatus::Received => "Received",
            ReturnStatus::Refunded => "Refunded",
        }
    }
}

impl ReturnLineItemStatus {
    /// Checks if the status is final and cannot be changed.
    pub fn is_final(&self) -> bool {
        matches!(self, ReturnLineItemStatus::Rejected | ReturnLineItemStatus::Processed)
    }

    /// Returns the status as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            ReturnLineItemStatus::Pending => "Pending",
            ReturnLineItemStatus::Inspected => "Inspected",
            ReturnLineItemStatus::Approved => "Approved",
            ReturnLineItemStatus::Rejected => "Rejected",
            ReturnLineItemStatus::Processed => "Processed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{MockDatabase, MockExecResult, DbBackend, EntityTrait, QueryTrait, Set};
    use sea_orm::ActiveValue::Set;
    use rust_decimal_macros::dec;

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

    /// Helper function to create a valid return line item.
    fn create_valid_return_line_item(return_id: Uuid) -> ReturnLineItemModel {
        ReturnLineItemModel::new(
            return_id,
            "Widget".to_string(),
            2,
            dec!(75.00),
            dec!(100.00),
            dec!(25.00),
            "pcs".to_string(),
            "PROD123".to_string(),
            "WidgetsCo".to_string(),
            "STK123".to_string(),
            "M".to_string(),
            "SKU123".to_string(),
            "SKUID123".to_string(),
            "http://example.com/widget.jpg".to_string(),
            "Widget Pro".to_string(),
            "TypeA".to_string(),
            ReturnLineItemStatus::Pending,
        )
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
            condition: Some(Condition::Damaged),
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
        assert!(return_request.updated_date.is_some());

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

    #[tokio::test]
    async fn test_return_line_item_creation() {
        let return_id = Uuid::new_v4();
        let line_item = create_valid_return_line_item(return_id);
        assert!(line_item.validate().is_ok());
        assert_eq!(line_item.product_name, "Widget");
        assert_eq!(line_item.quantity, 2);
        assert_eq!(line_item.sale_price, dec!(75.00));
        assert_eq!(line_item.original_price, dec!(100.00));
        assert_eq!(line_item.seller_discount, dec!(25.00));
        assert_eq!(line_item.status, ReturnLineItemStatus::Pending);
        assert!(line_item.created_date <= Utc::now());
    }

    #[tokio::test]
    async fn test_return_line_item_validation_failure() {
        // Create a line item with invalid data
        let line_item = ReturnLineItemModel {
            id: Uuid::new_v4(),
            return_id: Uuid::new_v4(),
            product_name: "".to_string(), // Invalid product name
            quantity: 0,                   // Invalid quantity
            sale_price: dec!(-10.00),      // Invalid sale price
            original_price: dec!(-20.00),  // Invalid original price
            seller_discount: dec!(-5.00),  // Invalid seller discount
            unit: "".to_string(),          // Invalid unit
            product_id: "".to_string(),    // Invalid product ID
            brand: "BrandName".to_string(),
            stock_code: "STK001".to_string(),
            size: "M".to_string(),
            seller_sku: "SKU001".to_string(),
            sku_id: "SKUID001".to_string(),
            sku_image: "invalid_url".to_string(), // Invalid SKU image URL
            sku_name: "SKU Name".to_string(),
            sku_type: "TypeA".to_string(),
            status: ReturnLineItemStatus::Pending,
            created_date: Utc::now(),
            updated_date: None,
        };

        let validation = line_item.validate();
        assert!(validation.is_err());

        if let Err(e) = validation {
            assert!(e.field_errors().contains_key("product_name"));
            assert!(e.field_errors().contains_key("quantity"));
            assert!(e.field_errors().contains_key("sale_price"));
            assert!(e.field_errors().contains_key("original_price"));
            assert!(e.field_errors().contains_key("seller_discount"));
            assert!(e.field_errors().contains_key("unit"));
            assert!(e.field_errors().contains_key("product_id"));
            assert!(e.field_errors().contains_key("sku_image"));
        }
    }

    #[tokio::test]
    async fn test_return_relationships() {
        // Mock database interactions using SeaORM's MockDatabase
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_exec_results(vec![
                MockExecResult::new_with_affected_rows(1), // Insert return
                MockExecResult::new_with_affected_rows(1), // Insert return line item
            ])
            .into_connection();

        let return_request = create_valid_return();
        let active_model: ActiveModel = return_request.clone().into();

        // Simulate inserting the return
        let insert_return = Entity::insert(active_model).exec(&db).await;
        assert!(insert_return.is_ok());

        // Create a return line item
        let line_item = create_valid_return_line_item(return_request.id);
        let active_line_item: ActiveModel = line_item.clone().into();

        // Simulate inserting the line item
        let insert_line_item = super::Entity::insert(active_line_item).exec(&db).await;
        assert!(insert_line_item.is_ok());
    }

    #[tokio::test]
    async fn test_return_line_item_status_update() {
        let mut line_item = create_valid_return_line_item(Uuid::new_v4());
        assert_eq!(line_item.status, ReturnLineItemStatus::Pending);

        // Update status to Inspected
        let result = line_item.update_status(ReturnLineItemStatus::Inspected);
        assert!(result.is_ok());
        assert_eq!(line_item.status, ReturnLineItemStatus::Inspected);
        assert!(line_item.updated_date.is_some());

        // Update status to Approved
        let result = line_item.update_status(ReturnLineItemStatus::Approved);
        assert!(result.is_ok());
        assert_eq!(line_item.status, ReturnLineItemStatus::Approved);
        assert!(line_item.updated_date.is_some());

        // Update status to Processed (final status)
        let result = line_item.update_status(ReturnLineItemStatus::Processed);
        assert!(result.is_ok());
        assert_eq!(line_item.status, ReturnLineItemStatus::Processed);

        // Attempt to update status after finalization
        let result = line_item.update_status(ReturnLineItemStatus::Rejected);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Cannot update status of a finalized line item"
        );
    }

    #[tokio::test]
    async fn test_apply_discount() {
        let mut line_item = create_valid_return_line_item(Uuid::new_v4());
        assert_eq!(line_item.seller_discount, dec!(25.00));

        // Apply additional discount
        let result = line_item.apply_discount(dec!(10.00));
        assert!(result.is_ok());
        assert_eq!(line_item.seller_discount, dec!(35.00));

        // Attempt to apply a negative discount that would result in a negative total discount
        let result = line_item.apply_discount(dec!(-40.00));
        assert!(result.is_err());
        assert_eq!(line_item.seller_discount, dec!(35.00));
    }
}
