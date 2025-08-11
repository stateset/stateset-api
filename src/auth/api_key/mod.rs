use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Database entity for API keys
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "api_keys")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    #[sea_orm(column_type = "Text")]
    pub key_hash: String,
    #[sea_orm(column_type = "Uuid")]
    pub user_id: Uuid,
    pub tenant_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked: bool,
}

/// Database relationships for ApiKey entity
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::auth::user::Entity",
        from = "Column::UserId",
        to = "crate::auth::user::Column::Id"
    )]
    User,
    #[sea_orm(has_many = "crate::auth::api_key_permission::Entity")]
    ApiKeyPermission,
}

impl Related<crate::auth::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl Related<crate::auth::api_key_permission::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ApiKeyPermission.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
