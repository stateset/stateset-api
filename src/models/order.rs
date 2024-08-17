use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::{DateTime, Utc};
use uuid::Uuid;

// Order Model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "orders")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    pub name: Option<String>,
    pub order_number: Option<String>,
    pub created_date: Option<DateTime<Utc>>,
    pub updated_date: Option<DateTime<Utc>>,
    pub order_status: Option<String>,
    pub imported_status: Option<String>,
    pub delivery_date: Option<DateTime<Utc>>,
    pub ordered_by: Option<String>,
    pub delivery_address: Option<String>,
    pub notes: Option<String>,
    pub imported_date: Option<DateTime<Utc>>,
    pub customer_number: Option<String>,
    pub customer_name: Option<String>,
    pub import: Option<bool>,
    pub customer_email: Option<String>,
    pub source: Option<String>,
    pub buyer_email: Option<String>,
    pub buyer_message: Option<String>,
    pub cancel_order_sla_time: Option<DateTime<Utc>>,
    pub cancel_reason: Option<String>,
    pub cancellation_initiator: Option<String>,
    pub fulfillment_type: Option<String>,
    pub delivery_type: Option<String>,
    pub is_cod: Option<bool>,
    pub is_replacement_order: Option<bool>,
    pub seller_note: Option<String>,
    pub status: Option<String>,
    pub tracking_number: Option<String>,
    #[sea_orm(column_type = "Uuid")]
    pub warehouse_id: Option<Uuid>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::order_line_item::Entity")]
    OrderLineItems,
}

impl Related<super::order_line_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OrderLineItems.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// Order Line Item Model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "order_line_items")]
pub struct OrderLineItem {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    #[sea_orm(column_type = "Uuid")]
    pub wholesale_order_id: Uuid,
    pub product_name: Option<String>,
    pub quantity: Option<String>,
    pub created_date: Option<DateTime<Utc>>,
    pub updated_date: Option<DateTime<Utc>>,
    pub unit: Option<String>,
    pub product_id: Option<String>,
    pub brand: Option<String>,
    pub stock_code: Option<String>,
    pub size: Option<String>,
    pub status: Option<String>,
    pub sale_price: Option<i32>,
    pub seller_discount: Option<i32>,
    pub seller_sku: Option<String>,
    pub sku_id: Option<String>,
    pub sku_image: Option<String>,
    pub sku_name: Option<String>,
    pub sku_type: Option<String>,
    pub original_price: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum OrderLineItemRelation {
    #[sea_orm(
        belongs_to = "super::order::Entity",
        from = "Column::WholesaleOrderId",
        to = "super::order::Column::Id"
    )]
    Order,
}

impl Related<super::order::Entity> for OrderLineItem {
    fn to() -> RelationDef {
        OrderLineItemRelation::Order.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn new(
        name: Option<String>,
        order_number: Option<String>,
        order_status: Option<String>,
        customer_name: Option<String>,
        customer_email: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            order_number,
            created_date: Some(Utc::now()),
            updated_date: None,
            order_status,
            imported_status: None,
            delivery_date: None,
            ordered_by: None,
            delivery_address: None,
            notes: None,
            imported_date: None,
            customer_number: None,
            customer_name,
            import: None,
            customer_email,
            source: Some("email".to_string()),
            buyer_email: None,
            buyer_message: None,
            cancel_order_sla_time: None,
            cancel_reason: None,
            cancellation_initiator: None,
            fulfillment_type: None,
            delivery_type: None,
            is_cod: None,
            is_replacement_order: None,
            seller_note: None,
            status: None,
            tracking_number: None,
            warehouse_id: None,
        }
    }

    pub fn add_line_item(&self, line_item: OrderLineItem) -> Result<(), ValidationError> {
        // Here you would typically save the line item to the database
        // For this example, we'll just validate the line item
        line_item.validate()?;
        Ok(())
    }

    pub fn update_status(&mut self, new_status: String) {
        self.status = Some(new_status);
        self.updated_date = Some(Utc::now());
    }
}

impl OrderLineItem {
    pub fn new(
        wholesale_order_id: Uuid,
        product_name: Option<String>,
        quantity: Option<String>,
        sale_price: Option<i32>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            wholesale_order_id,
            product_name,
            quantity,
            created_date: Some(Utc::now()),
            updated_date: None,
            unit: None,
            product_id: None,
            brand: None,
            stock_code: None,
            size: None,
            status: None,
            sale_price,
            seller_discount: None,
            seller_sku: None,
            sku_id: None,
            sku_image: None,
            sku_name: None,
            sku_type: None,
            original_price: None,
        }
    }

    pub fn update_status(&mut self, new_status: String) {
        self.status = Some(new_status);
        self.updated_date = Some(Utc::now());
    }
}