use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "return_history")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub return_id: Uuid,
    pub status_from: String,
    pub status_to: String,
    pub changed_by: Option<Uuid>,
    pub change_reason: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
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
