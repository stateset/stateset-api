use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "warranty_line_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub amount: Decimal,
    pub condition: Option<String>,
    pub flat_rate_shipping: bool,
    pub item_id: String,
    pub parent_id: String,
    pub product_id: String,
    pub product_title: String,
    pub quantity: i32,
    pub restock: bool,
    pub send_replacement: bool,
    pub warranty_id: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::warranty::Entity",
        from = "Column::WarrantyId",
        to = "super::warranty::Column::Id"
    )]
    Warranty,
}

impl Related<super::warranty::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Warranty.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn new(
        warranty_id: i32,
        amount: Decimal,
        item_id: String,
        parent_id: String,
        product_id: String,
        product_title: String,
        quantity: i32,
    ) -> Result<Self, ValidationError> {
        let now = Utc::now();
        let item = Self {
            id: 0, // Will be set by database
            amount,
            condition: None,
            flat_rate_shipping: false,
            item_id,
            parent_id,
            product_id,
            product_title,
            quantity,
            restock: false,
            send_replacement: false,
            warranty_id,
            created_at: now,
            updated_at: now,
        };
        item.validate().map_err(|_| ValidationError::new("Warranty line item validation failed"))?;
        Ok(item)
    }
}
