use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Enum representing the possible statuses of an order.
#[derive(
    Clone, Debug, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, strum::Display,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum OrderStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,
    #[sea_orm(string_value = "Processing")]
    Processing,
    #[sea_orm(string_value = "OnHold")]
    OnHold,
    #[sea_orm(string_value = "Shipped")]
    Shipped,
    #[sea_orm(string_value = "Delivered")]
    Delivered,
    #[sea_orm(string_value = "Cancelled")]
    Cancelled,
    #[sea_orm(string_value = "Archived")]
    Archived,
    #[sea_orm(string_value = "Exchanged")]
    Exchanged,
}

/// Enum representing the possible fulfillment types of an order.
#[derive(Clone, Debug, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum FulfillmentType {
    #[sea_orm(string_value = "Standard")]
    Standard,
    #[sea_orm(string_value = "Express")]
    Express,
}

/// Enum representing the possible delivery types of an order.
#[derive(Clone, Debug, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum DeliveryType {
    #[sea_orm(string_value = "Home")]
    Home,
    #[sea_orm(string_value = "Pickup")]
    Pickup,
    #[sea_orm(string_value = "Locker")]
    Locker,
}

/// The `orders` table.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "orders")]
pub struct Model {
    /// Primary key: Unique identifier for the order.
    #[sea_orm(primary_key)]
    pub id: Uuid,

    /// Unique order number.
    #[validate(length(
        min = 1,
        max = 50,
        message = "Order number must be between 1 and 50 characters"
    ))]
    pub order_number: String,

    /// Name of the customer who placed the order.
    #[validate(length(
        min = 1,
        max = 100,
        message = "Customer name must be between 1 and 100 characters"
    ))]
    pub customer_name: String,

    /// Email of the customer.
    #[validate(email(message = "Invalid email format"))]
    pub customer_email: String,

    /// Delivery address for the order.
    #[validate(length(
        min = 1,
        max = 500,
        message = "Delivery address must be between 1 and 500 characters"
    ))]
    pub delivery_address: String,

    /// Optional notes associated with the order.
    #[validate(length(max = 1000, message = "Notes must not exceed 1000 characters"))]
    pub notes: Option<String>,

    /// Foreign key referencing the warehouse handling the order.
    #[sea_orm(column_type = "Uuid")]
    pub warehouse_id: Uuid,

    /// Current status of the order.
    pub order_status: OrderStatus,

    /// Type of fulfillment for the order.
    pub fulfillment_type: FulfillmentType,

    /// Type of delivery for the order.
    pub delivery_type: DeliveryType,

    /// Indicates if the order is Cash on Delivery (COD).
    pub is_cod: bool,

    /// Indicates if the order is a replacement.
    pub is_replacement_order: bool,

    /// Tracking number for the order shipment.
    #[validate(length(max = 100, message = "Tracking number must not exceed 100 characters"))]
    pub tracking_number: Option<String>,

    /// Optional seller notes.
    #[validate(length(max = 500, message = "Seller note must not exceed 500 characters"))]
    pub seller_note: Option<String>,

    /// Source from which the order was placed (e.g., website, mobile app).
    #[validate(length(max = 50, message = "Source must not exceed 50 characters"))]
    pub source: Option<String>,

    /// Timestamp when the order was created.
    pub created_date: DateTime<Utc>,

    /// Timestamp when the order was last updated.
    pub updated_date: Option<DateTime<Utc>>,

    /// Expected delivery date for the order.
    pub delivery_date: Option<DateTime<Utc>>,

    /// Service Level Agreement (SLA) time for order cancellation.
    pub cancel_order_sla_time: Option<DateTime<Utc>>,

    /// Reason for order cancellation.
    #[validate(length(max = 500, message = "Cancel reason must not exceed 500 characters"))]
    pub cancel_reason: Option<String>,

    /// Initiator of the order cancellation.
    #[validate(length(
        max = 100,
        message = "Cancellation initiator must not exceed 100 characters"
    ))]
    pub cancellation_initiator: Option<String>,
}

/// Define relations for the `orders` table.
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    /// An order has many line items.
    #[sea_orm(has_many = "super::order_line_item::Entity")]
    OrderLineItems,
    /// An order has many invoices.
    #[sea_orm(has_many = "super::invoices::Entity")]
    Invoices,
    /// An order has many payments.
    #[sea_orm(has_many = "super::payment::Entity")]
    Payments,

    /// An order belongs to a warehouse.
    #[sea_orm(
        belongs_to = "super::warehouse::Entity",
        from = "Column::WarehouseId",
        to = "super::warehouse::Column::Id",
        on_update = "Cascade",
        on_delete = "Restrict"
    )]
    Warehouse,
}

impl Related<super::order_line_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OrderLineItems.def()
    }
}

impl Related<super::invoices::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Invoices.def()
    }
}

impl Related<super::payment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Payments.def()
    }
}

impl Related<super::warehouse::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Warehouse.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Implementation block for the `Order` model.
impl Model {
    /// Creates a new order with the specified parameters.
    ///
    /// # Arguments
    ///
    /// * `order_number` - A unique identifier for the order.
    /// * `customer_name` - The name of the customer placing the order.
    /// * `customer_email` - The email of the customer.
    /// * `delivery_address` - The address where the order should be delivered.
    /// * `warehouse_id` - The UUID of the warehouse handling the order.
    /// * `order_status` - The current status of the order.
    /// * `fulfillment_type` - The type of fulfillment for the order.
    /// * `delivery_type` - The type of delivery for the order.
    /// * `is_cod` - Indicates if the order is Cash on Delivery.
    /// * `is_replacement_order` - Indicates if the order is a replacement.
    /// * `source` - The source from which the order was placed.
    pub fn new(
        order_number: String,
        customer_name: String,
        customer_email: String,
        delivery_address: String,
        warehouse_id: Uuid,
        order_status: OrderStatus,
        fulfillment_type: FulfillmentType,
        delivery_type: DeliveryType,
        is_cod: bool,
        is_replacement_order: bool,
        source: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            order_number,
            customer_name,
            customer_email,
            delivery_address,
            notes: None,
            warehouse_id,
            order_status,
            fulfillment_type,
            delivery_type,
            is_cod,
            is_replacement_order,
            tracking_number: None,
            seller_note: None,
            source,
            created_date: Utc::now(),
            updated_date: None,
            delivery_date: None,
            cancel_order_sla_time: None,
            cancel_reason: None,
            cancellation_initiator: None,
        }
    }

    /// Updates the status of the order.
    ///
    /// # Arguments
    ///
    /// * `new_status` - The new status to set for the order.
    pub fn update_status(&mut self, new_status: OrderStatus) {
        self.order_status = new_status;
        self.updated_date = Some(Utc::now());
    }

    /// Adds a note to the order.
    ///
    /// # Arguments
    ///
    /// * `note` - The note to add.
    pub fn add_note(&mut self, note: String) {
        self.notes = Some(note);
        self.updated_date = Some(Utc::now());
    }

    /// Sets the tracking number for the order.
    ///
    /// # Arguments
    ///
    /// * `tracking_number` - The tracking number to set.
    pub fn set_tracking_number(&mut self, tracking_number: String) {
        self.tracking_number = Some(tracking_number);
        self.updated_date = Some(Utc::now());
    }

    /// Validates the order data.
    pub fn validate_order(&self) -> Result<(), validator::ValidationErrors> {
        self.validate()?;
        Ok(())
    }

    /// Checks if the order can be cancelled based on its current status.
    pub fn can_be_cancelled(&self) -> bool {
        matches!(
            self.order_status,
            OrderStatus::Pending | OrderStatus::Processing
        )
    }

    /// Checks if the order can be shipped based on its current status.
    pub fn can_be_shipped(&self) -> bool {
        matches!(self.order_status, OrderStatus::Processing)
    }

    /// Calculates the total value of the order (placeholder - would need line items).
    pub fn calculate_total(&self) -> rust_decimal::Decimal {
        // This would normally sum up all line items
        // For now, return zero
        rust_decimal::Decimal::ZERO
    }
}

// Order line items moved to separate file: order_line_item.rs

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    use super::*;
    use chrono::Duration;
    use sea_orm::Set;
    use sea_orm::{DbBackend, EntityTrait, QueryTrait};
    use validator::Validate;

    /// Helper function to create a valid order.
    fn create_valid_order() -> Model {
        Model::new(
            "ORD001".to_string(),
            "Alice Smith".to_string(),
            "alice@example.com".to_string(),
            "123 Maple Street, Springfield, USA".to_string(),
            Uuid::new_v4(),
            OrderStatus::Pending,
            FulfillmentType::Standard,
            DeliveryType::Home,
            false,
            false,
            Some("website".to_string()),
        )
    }

    /// Helper function to create a valid order line item.
    fn create_valid_line_item(order_id: Uuid) -> super::order_line_item::Model {
        super::order_line_item::Model::new(
            order_id,
            "Widget".to_string(),
            3,
            1500, // $15.00
            2000, // $20.00
            500,  // $5.00 discount
            "pcs".to_string(),
            "WGT-001".to_string(),
            "WidgetsCo".to_string(),
            "STK-WGT-001".to_string(),
            "L".to_string(),
            "SKU-WGT-001".to_string(),
            "SKUID-WGT-001".to_string(),
            "http://example.com/widget.jpg".to_string(),
            "Widget Pro".to_string(),
            "TypeA".to_string(),
            super::order_line_item::OrderLineItemStatus::Pending,
        )
    }

    #[tokio::test]
    async fn test_order_creation() {
        let order = create_valid_order();
        assert!(order.validate().is_ok());
        assert_eq!(order.order_number, "ORD001");
        assert_eq!(order.customer_name, "Alice Smith");
        assert_eq!(order.customer_email, "alice@example.com");
        assert_eq!(order.delivery_address, "123 Maple Street, Springfield, USA");
        assert_eq!(order.order_status, OrderStatus::Pending);
        assert_eq!(order.fulfillment_type, FulfillmentType::Standard);
        assert_eq!(order.delivery_type, DeliveryType::Home);
        assert_eq!(order.is_cod, false);
        assert_eq!(order.is_replacement_order, false);
        assert_eq!(order.source, Some("website".to_string()));
        assert!(order.created_date <= Utc::now());
    }

    #[tokio::test]
    async fn test_order_validation_failure() {
        // Create an order with invalid email
        let order = Model::new(
            "ORD002".to_string(),
            "Bob Johnson".to_string(),
            "invalid_email".to_string(), // Invalid email
            "456 Oak Avenue, Metropolis, USA".to_string(),
            Uuid::new_v4(),
            OrderStatus::Processing,
            FulfillmentType::Express,
            DeliveryType::Pickup,
            true,
            false,
            Some("mobile_app".to_string()),
        );

        let validation = order.validate();
        assert!(validation.is_err());

        if let Err(e) = validation {
            assert!(e.field_errors().contains_key("customer_email"));
        }
    }

    #[tokio::test]
    async fn test_order_line_item_creation() {
        let order_id = Uuid::new_v4();
        let line_item = create_valid_line_item(order_id);
        assert!(line_item.validate().is_ok());
        assert_eq!(line_item.product_name, "Widget");
        assert_eq!(line_item.quantity, 3);
        assert_eq!(line_item.sale_price, 1500);
        assert_eq!(line_item.original_price, 2000);
        assert_eq!(line_item.seller_discount, 500);
        assert_eq!(line_item.unit, "pcs");
        assert_eq!(line_item.product_id, "WGT-001");
        assert_eq!(line_item.brand, "WidgetsCo");
        assert_eq!(line_item.stock_code, "STK-WGT-001");
        assert_eq!(line_item.size, "L");
        assert_eq!(line_item.seller_sku, "SKU-WGT-001");
        assert_eq!(line_item.sku_id, "SKUID-WGT-001");
        assert_eq!(line_item.sku_image, "http://example.com/widget.jpg");
        assert_eq!(line_item.sku_name, "Widget Pro");
        assert_eq!(line_item.sku_type, "TypeA");
        assert_eq!(line_item.status, OrderLineItemStatus::Pending);
        assert!(line_item.created_date <= Utc::now());
    }

    #[tokio::test]
    async fn test_order_status_update() {
        let mut order = create_valid_order();
        order.update_status(OrderStatus::Shipped);
        assert_eq!(order.order_status, OrderStatus::Shipped);
        assert!(order.updated_date.is_some());
        assert!(order.updated_date.unwrap() <= Utc::now());
    }

    #[tokio::test]
    async fn test_order_line_item_status_update() {
        let order_id = Uuid::new_v4();
        let mut line_item = create_valid_line_item(order_id);
        line_item.update_status(OrderLineItemStatus::Shipped);
        assert_eq!(line_item.status, OrderLineItemStatus::Shipped);
        assert!(line_item.updated_date.is_some());
        assert!(line_item.updated_date.unwrap() <= Utc::now());
    }

    #[tokio::test]
    async fn test_add_line_item_to_order() {
        let order = create_valid_order();
        let line_item = create_valid_line_item(order.id);
        // In a real application, you'd save the line item to the database here.
        // For testing, we'll just validate the operation.
        assert!(line_item.validate().is_ok());
    }

    #[tokio::test]
    async fn test_invalid_order_line_item_creation() {
        let order_id = Uuid::new_v4();
        let line_item = OrderLineItemModel::new(
            order_id,
            "".to_string(), // Invalid product name
            0,              // Invalid quantity
            -100,           // Invalid sale price
            2000,
            -500, // Invalid seller discount
            "pcs".to_string(),
            "WGT-002".to_string(),
            "WidgetsCo".to_string(),
            "STK-WGT-002".to_string(),
            "M".to_string(),
            "SKU-WGT-002".to_string(),
            "SKUID-WGT-002".to_string(),
            "invalid_url".to_string(), // Invalid SKU image URL
            "Widget Mini".to_string(),
            "TypeB".to_string(),
            OrderLineItemStatus::Pending,
        );

        let validation = line_item.validate();
        assert!(validation.is_err());

        if let Err(e) = validation {
            assert!(e.field_errors().contains_key("product_name"));
            assert!(e.field_errors().contains_key("quantity"));
            assert!(e.field_errors().contains_key("sale_price"));
            assert!(e.field_errors().contains_key("seller_discount"));
            assert!(e.field_errors().contains_key("sku_image"));
        }
    }
}
