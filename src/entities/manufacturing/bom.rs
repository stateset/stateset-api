use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, ConnectionTrait};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "manufacturing_boms")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub product_id: Uuid,
    pub item_master_id: Option<i64>,
    pub bom_number: String,
    pub name: String,
    pub description: Option<String>,
    pub revision: String,
    pub lifecycle_status: String,
    #[sea_orm(column_type = "JsonBinary")]
    pub metadata: Option<Value>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::bom_component::Entity")]
    Components,
    #[sea_orm(has_many = "super::bom_audit::Entity")]
    Audits,
    #[sea_orm(has_many = "super::work_order::Entity")]
    WorkOrders,
}

impl Related<super::bom_component::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Components.def()
    }
}

impl Related<super::bom_audit::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Audits.def()
    }
}

impl Related<super::work_order::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WorkOrders.def()
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let now = Utc::now();

        if insert {
            if let ActiveValue::NotSet = self.id {
                self.id = ActiveValue::Set(Uuid::new_v4());
            }

            if let ActiveValue::NotSet = self.created_at {
                self.created_at = ActiveValue::Set(now);
            }
        }

        if let ActiveValue::NotSet = self.lifecycle_status {
            self.lifecycle_status = ActiveValue::Set("draft".to_string());
        }

        self.updated_at = ActiveValue::Set(now);

        Ok(self)
    }
}
