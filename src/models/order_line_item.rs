use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// The `order_line_items` table.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "order_line_items")]
pub struct Model {
    /// Primary key: Unique identifier for the order line item.
    #[sea_orm(primary_key)]
    pub id: Uuid,

    /// Foreign key referencing the order.
    #[sea_orm(column_type = "Uuid")]
    pub order_id: Uuid,

    /// Name of the product.
    
    pub product_name: String,

    /// Quantity of the product ordered.
    
    pub quantity: u32,

    /// Sale price per unit in cents.
    
    pub sale_price: i32,

    /// Original price per unit in cents before any discounts.
    
    pub original_price: i32,

    /// Discount applied by the seller in cents.
    
    pub seller_discount: i32,

    /// Unit of measurement (e.g., pcs, kg).
    
    pub unit: String,

    /// Identifier for the product.
    
    pub product_id: String,

    /// Brand of the product.
    
    pub brand: String,

    /// Stock code of the product.
    
    pub stock_code: String,

    /// Size of the product.
    
    pub size: String,

    /// Seller's SKU for the product.
    
    pub seller_sku: String,

    /// SKU ID.
    
    pub sku_id: String,

    /// URL to the SKU image.
    
    pub sku_image: String,

    /// Name of the SKU.
    
    pub sku_name: String,

    /// Type of SKU.
    
    pub sku_type: String,

    /// Timestamp when the line item was created.
    pub created_date: DateTime<Utc>,

    /// Timestamp when the line item was last updated.
    pub updated_date: Option<DateTime<Utc>>,

    /// Current status of the line item.
    
    pub status: OrderLineItemStatus,
}

/// Enum representing the possible statuses of an order line item.
#[derive(Clone, Debug, PartialEq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
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
pub enum Relation {
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

impl Related<super::order::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Order.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Implementation of methods for the order line item
impl Model {
    /// Creates a new order line item with the specified parameters.
    ///
    /// #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]Arguments
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
    /// #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]Arguments
    ///
    /// * `new_status` - The new status to set for the line item.
    pub fn update_status(&mut self, new_status: OrderLineItemStatus) {
        self.status = new_status;
        self.updated_date = Some(Utc::now());
    }

    /// Applies a discount to the order line item.
    ///
    /// #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]Arguments
    ///
    /// * `discount` - The discount amount in cents to apply.
    pub fn apply_discount(&mut self, discount: i32) {
        self.seller_discount += discount;
        self.updated_date = Some(Utc::now());
    }

    // Additional methods as needed...
}
