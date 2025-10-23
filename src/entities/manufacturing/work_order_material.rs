use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, ConnectionTrait};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "manufacturing_work_order_materials")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub work_order_id: Uuid,
    pub component_id: Uuid,
    pub reserved_quantity: Decimal,
    pub consumed_quantity: Decimal,
    pub inventory_reservation_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::work_order::Entity",
        from = "Column::WorkOrderId",
        to = "super::work_order::Column::Id"
    )]
    WorkOrder,
    #[sea_orm(
        belongs_to = "super::bom_component::Entity",
        from = "Column::ComponentId",
        to = "super::bom_component::Column::Id"
    )]
    BomComponent,
}

impl Related<super::work_order::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WorkOrder.def()
    }
}

impl Related<super::bom_component::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BomComponent.def()
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

            if let ActiveValue::NotSet = self.reserved_quantity {
                self.reserved_quantity = ActiveValue::Set(Decimal::ZERO);
            }

            if let ActiveValue::NotSet = self.consumed_quantity {
                self.consumed_quantity = ActiveValue::Set(Decimal::ZERO);
            }

            if let ActiveValue::NotSet = self.created_at {
                self.created_at = ActiveValue::Set(now);
            }
        }

        self.updated_at = ActiveValue::Set(now);

        Ok(self)
    }
}
