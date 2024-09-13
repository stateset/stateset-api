use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Enum representing the possible statuses of an order.
#[derive(Clone, Debug, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(Some(32))")]
pub enum OrderStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,
    #[sea_orm(string_value = "Processing")]
    Processing,
    #[sea_orm(string_value = "Shipped")]
    Shipped,
    #[sea_orm(string_value = "Delivered")]
    Delivered,
    #[sea_orm(string_value = "Cancelled")]
    Cancelled,
}

/// Enum representing the possible fulfillment types of an order.
#[derive(Clone, Debug, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(Some(32))")]
pub enum FulfillmentType {
    #[sea_orm(string_value = "Standard")]
    Standard,
    #[sea_orm(string_value = "Express")]
    Express,
}

/// Enum representing the possible delivery types of an order.
#[derive(Clone, Debug, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(Some(32))")]
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
    #[validate(length(min = 1))]
    pub order_number: String,

    /// Name of the customer who placed the order.
    #[validate(length(min = 1))]
    pub customer_name: String,

    /// Email of the customer.
    #[validate(email)]
    pub customer_email: String,

    /// Delivery address for the order.
    #[validate(length(min = 10))]
    pub delivery_address: String,

    /// Optional notes associated with the order.
    #[validate(length(max = 500))]
    pub notes: Option<String>,

    /// Foreign key referencing the warehouse handling the order.
    #[sea_orm(column_type = "Uuid")]
    pub warehouse_id: Uuid,

    /// Current status of the order.
    #[validate]
    pub order_status: OrderStatus,

    /// Type of fulfillment for the order.
    #[validate]
    pub fulfillment_type: FulfillmentType,

    /// Type of delivery for the order.
    #[validate]
    pub delivery_type: DeliveryType,

    /// Indicates if the order is Cash on Delivery (COD).
    pub is_cod: bool,

    /// Indicates if the order is a replacement.
    pub is_replacement_order: bool,

    /// Tracking number for the order shipment.
    #[validate(length(max = 100))]
    pub tracking_number: Option<String>,

    /// Optional seller notes.
    #[validate(length(max = 255))]
    pub seller_note: Option<String>,

    /// Source from which the order was placed (e.g., website, mobile app).
    #[validate(url)]
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
    pub cancel_reason: Option<String>,

    /// Initiator of the order cancellation.
    pub cancellation_initiator: Option<String>,
}

/// Define relations for the `orders` table.
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    /// An order has many line items.
    #[sea_orm(has_many = "super::order_line_item::Entity")]
    OrderLineItems,

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

    // Additional methods as needed...
}

/// The `order_line_items` table.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "order_line_items")]
pub struct OrderLineItemModel {
    /// Primary key: Unique identifier for the order line item.
    #[sea_orm(primary_key)]
    pub id: Uuid,

    /// Foreign key referencing the order.
    #[sea_orm(column_type = "Uuid")]
    pub order_id: Uuid,

    /// Name of the product.
    #[validate(length(min = 1))]
    pub product_name: String,

    /// Quantity of the product ordered.
    #[validate(range(min = 1))]
    pub quantity: u32,

    /// Sale price per unit in cents.
    #[validate(range(min = 0))]
    pub sale_price: i32,

    /// Original price per unit in cents before any discounts.
    #[validate(range(min = 0))]
    pub original_price: i32,

    /// Discount applied by the seller in cents.
    #[validate(range(min = 0))]
    pub seller_discount: i32,

    /// Unit of measurement (e.g., pcs, kg).
    #[validate(length(min = 1))]
    pub unit: String,

    /// Identifier for the product.
    #[validate(length(min = 1))]
    pub product_id: String,

    /// Brand of the product.
    #[validate(length(max = 100))]
    pub brand: String,

    /// Stock code of the product.
    #[validate(length(max = 100))]
    pub stock_code: String,

    /// Size of the product.
    #[validate(length(max = 20))]
    pub size: String,

    /// Seller's SKU for the product.
    #[validate(length(max = 50))]
    pub seller_sku: String,

    /// SKU ID.
    #[validate(length(max = 100))]
    pub sku_id: String,

    /// URL to the SKU image.
    #[validate(url)]
    pub sku_image: String,

    /// Name of the SKU.
    #[validate(length(max = 100))]
    pub sku_name: String,

    /// Type of SKU.
    #[validate(length(max = 50))]
    pub sku_type: String,

    /// Timestamp when the line item was created.
    pub created_date: DateTime<Utc>,

    /// Timestamp when the line item was last updated.
    pub updated_date: Option<DateTime<Utc>>,

    /// Current status of the line item.
    #[validate]
    pub status: OrderLineItemStatus,
}

/// Enum representing the possible statuses of an order line item.
#[derive(Clone, Debug, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(Some(32))")]
pub enum OrderLineItemStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,
    #[sea_orm(string_value = "Shipped")]
    Shipped,
    #[sea_orm(string_value = "Delivered")]
    Delivered,
    #[sea_orm(string_value = "Cancelled")]
    Cancelled,
}

/// Define relations for the `order_line_items` table.
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum OrderLineItemRelation {
    /// Each line item belongs to an order.
    #[sea_orm(
        belongs_to = "super::order::Entity",
        from = "Column::OrderId",
        to = "super::order::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Order,
}

impl Related<super::order::Entity> for OrderLineItemModel {
    fn to() -> RelationDef {
        OrderLineItemRelation::Order.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Implementation block for the `OrderLineItem` model.
impl OrderLineItemModel {
    /// Creates a new order line item with the specified parameters.
    ///
    /// # Arguments
    ///
    /// * `order_id` - The UUID of the order this line item belongs to.
    /// * `product_name` - The name of the product.
    /// * `quantity` - The quantity of the product ordered.
    /// * `sale_price` - The sale price per unit in cents.
    /// * `original_price` - The original price per unit in cents.
    /// * `seller_discount` - The discount applied by the seller in cents.
    /// * `unit` - The unit of measurement (e.g., pcs, kg).
    /// * `product_id` - The identifier for the product.
    /// * `brand` - The brand of the product.
    /// * `stock_code` - The stock code of the product.
    /// * `size` - The size of the product.
    /// * `seller_sku` - The seller's SKU for the product.
    /// * `sku_id` - The SKU ID.
    /// * `sku_image` - The URL to the SKU image.
    /// * `sku_name` - The name of the SKU.
    /// * `sku_type` - The type of SKU.
    /// * `status` - The current status of the line item.
    pub fn new(
        order_id: Uuid,
        product_name: String,
        quantity: u32,
        sale_price: i32,
        original_price: i32,
        seller_discount: i32,
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
        status: OrderLineItemStatus,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            order_id,
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
            created_date: Utc::now(),
            updated_date: None,
            status,
        }
    }

    /// Updates the status of the order line item.
    ///
    /// # Arguments
    ///
    /// * `new_status` - The new status to set for the line item.
    pub fn update_status(&mut self, new_status: OrderLineItemStatus) {
        self.status = new_status;
        self.updated_date = Some(Utc::now());
    }

    /// Applies a discount to the order line item.
    ///
    /// # Arguments
    ///
    /// * `discount` - The discount amount in cents to apply.
    pub fn apply_discount(&mut self, discount: i32) {
        self.seller_discount += discount;
        self.updated_date = Some(Utc::now());
    }

    // Additional methods as needed...
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{MockDatabase, MockExecResult, DbBackend, EntityTrait, QueryTrait};
    use sea_orm::Set;
    use chrono::Duration;
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
    fn create_valid_line_item(order_id: Uuid) -> OrderLineItemModel {
        OrderLineItemModel::new(
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
            OrderLineItemStatus::Pending,
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
            "".to_string(),    // Invalid product name
            0,                 // Invalid quantity
            -100,              // Invalid sale price
            2000,
            -500,              // Invalid seller discount
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

    #[tokio::test]
    async fn test_order_uniqueness() {
        // Mock database with existing order_number
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_exec_results(vec![
                MockExecResult::affected_rows(1), // Existing order
            ])
            .into_connection();

        // Attempt to insert an order with a duplicate order_number
        let order = create_valid_order();
        let active_model: ActiveModel = order.clone().into();

        // Simulate unique constraint violation on order_number
        let db = db.expect_exec(move |exec| {
            exec.statement.contains("INSERT INTO orders")
        })
        .returning(|_| Err(sea_orm::DbErr::Exec("Duplicate order_number".to_string())));

        let result = Model::insert(active_model).exec(&db).await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("Duplicate order_number"));
        }
    }
}
