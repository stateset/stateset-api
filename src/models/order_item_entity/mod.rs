use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Order Item Status enumeration
#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum OrderItemStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,

    #[sea_orm(string_value = "Shipped")]
    Shipped,

    #[sea_orm(string_value = "Delivered")]
    Delivered,

    #[sea_orm(string_value = "Cancelled")]
    Cancelled,

    #[sea_orm(string_value = "Returned")]
    Returned,

    #[sea_orm(string_value = "Refunded")]
    Refunded,
}

/// Order Item entity model.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "order_items")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub order_id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub product_id: Uuid,

    pub product_name: String,

    pub product_sku: String,

    #[validate(range(min = 1))]
    pub quantity: i32,

    pub unit_price: f64,

    pub total_price: f64,

    pub discount_amount: f64,

    pub tax_amount: f64,

    pub status: OrderItemStatus,

    pub notes: Option<String>,

    pub created_at: DateTime<Utc>,

    pub updated_at: DateTime<Utc>,
}

/// Order Item entity relations
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::models::order_entity::Entity",
        from = "Column::OrderId",
        to = "crate::models::order_entity::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Order,
}

impl Related<crate::models::order_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Order.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Creates a new Order Item.
    pub fn new(
        order_id: Uuid,
        product_id: Uuid,
        product_name: String,
        product_sku: String,
        quantity: i32,
        unit_price: f64,
        discount_amount: f64,
        tax_amount: f64,
        notes: Option<String>,
    ) -> Self {
        let now = Utc::now();
        let total_price = (unit_price * quantity as f64) - discount_amount + tax_amount;

        Self {
            id: Uuid::new_v4(),
            order_id,
            product_id,
            product_name,
            product_sku,
            quantity,
            unit_price,
            total_price,
            discount_amount,
            tax_amount,
            status: OrderItemStatus::Pending,
            notes,
            created_at: now,
            updated_at: now,
        }
    }

    /// Updates the item status.
    pub fn update_status(&mut self, new_status: OrderItemStatus) {
        self.status = new_status;
        self.updated_at = Utc::now();
    }

    /// Updates the quantity and recalculates the total price.
    pub fn update_quantity(&mut self, new_quantity: i32) {
        self.quantity = new_quantity;
        self.total_price =
            (self.unit_price * new_quantity as f64) - self.discount_amount + self.tax_amount;
        self.updated_at = Utc::now();
    }

    /// Applies a discount to the item and recalculates the total price.
    pub fn apply_discount(&mut self, discount_amount: f64) {
        self.discount_amount = discount_amount;
        self.total_price =
            (self.unit_price * self.quantity as f64) - discount_amount + self.tax_amount;
        self.updated_at = Utc::now();
    }
}
