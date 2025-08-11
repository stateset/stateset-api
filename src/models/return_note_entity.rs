use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum ReturnNoteType {
    #[sea_orm(string_value = "Internal")]
    Internal,
    #[sea_orm(string_value = "Customer")]
    Customer,
    #[sea_orm(string_value = "System")]
    System,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "return_notes")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub return_id: Uuid,
    pub note_type: ReturnNoteType,
    pub content: String,
    pub created_by: Option<Uuid>,
    pub is_visible_to_customer: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::models::return_entity::Entity",
        from = "Column::ReturnId",
        to = "crate::models::return_entity::Column::Id"
    )]
    Return,
}

impl Related<crate::models::return_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Return.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
