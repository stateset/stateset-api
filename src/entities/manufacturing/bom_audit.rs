use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, ConnectionTrait};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "manufacturing_bom_audits")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub bom_id: Uuid,
    pub event_type: String,
    pub user_id: Option<Uuid>,
    pub notes: Option<String>,
    pub event_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::bom::Entity",
        from = "Column::BomId",
        to = "super::bom::Column::Id"
    )]
    Bom,
}

impl Related<super::bom::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Bom.def()
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if insert {
            if let ActiveValue::NotSet = self.id {
                self.id = ActiveValue::Set(Uuid::new_v4());
            }

            if let ActiveValue::NotSet = self.event_at {
                self.event_at = ActiveValue::Set(Utc::now());
            }
        }

        Ok(self)
    }
}
