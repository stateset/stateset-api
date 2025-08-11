use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Database entity for API key permissions
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "api_key_permissions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(column_type = "Uuid")]
    pub api_key_id: Uuid,
    pub permission: String,
    pub created_at: DateTime<Utc>,
}

/// Database relationships for ApiKeyPermission entity
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::auth::api_key::Entity",
        from = "Column::ApiKeyId",
        to = "crate::auth::api_key::Column::Id"
    )]
    ApiKey,
}

impl Related<crate::auth::api_key::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ApiKey.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
