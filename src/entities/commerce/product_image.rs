use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "product_images")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub product_id: Uuid,

    pub url: String,
    pub alt_text: Option<String>,
    pub sort_order: i32,
    pub is_primary: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::super::product::Entity",
        from = "Column::ProductId",
        to = "super::super::product::Column::Id"
    )]
    Product,
}

impl Related<super::super::product::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Product.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
