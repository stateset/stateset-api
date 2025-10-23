use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, ConnectionTrait};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "manufacturing_bom_components")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub bom_id: Uuid,
    pub component_product_id: Option<Uuid>,
    pub component_item_id: Option<i64>,
    pub quantity: Decimal,
    pub unit_of_measure: String,
    pub position: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::bom::Entity",
        from = "Column::BomId",
        to = "super::bom::Column::Id"
    )]
    Bom,
    #[sea_orm(has_many = "super::work_order_material::Entity")]
    WorkOrderMaterials,
}

impl Related<super::bom::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Bom.def()
    }
}

impl Related<super::work_order_material::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WorkOrderMaterials.def()
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

        self.updated_at = ActiveValue::Set(now);

        Ok(self)
    }
}
