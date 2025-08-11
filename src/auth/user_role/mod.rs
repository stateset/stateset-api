use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Database entity for user-role associations
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "user_roles")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(column_type = "Uuid")]
    pub user_id: Uuid,
    pub role_name: String,
    pub created_at: DateTime<Utc>,
}

/// Database relationships for UserRole entity
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::auth::user::Entity",
        from = "Column::UserId",
        to = "crate::auth::user::Column::Id"
    )]
    User,
}

impl Related<crate::auth::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
