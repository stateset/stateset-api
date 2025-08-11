use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "warranty_claims")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub warranty_id: Uuid,
    pub claim_number: String,
    pub status: String,
    pub claim_date: DateTime<Utc>,
    pub description: String,
    pub resolution: Option<String>,
    pub resolved_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
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

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C: ConnectionTrait>(self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let mut active_model = self;
        let now = Utc::now();

        if insert {
            active_model.created_at = Set(now);
            if let ActiveValue::NotSet = active_model.status {
                active_model.status = Set("submitted".to_string());
            }
        }

        active_model.updated_at = Set(Some(now));
        Ok(active_model)
    }
}
