use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use sea_orm::Set;
use serde::{Deserialize, Serialize};
use validator::Validate;
use uuid::Uuid;
use async_trait::async_trait;

/// Manufacturing Cost entity model.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "manufacturing_costs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,

    pub work_order_id: i32,

    #[validate(length(
        min = 1,
        max = 100,
        message = "Cost type must be between 1 and 100 characters"
    ))]
    pub cost_type: String,

    pub cost_amount: Decimal,

    pub created_at: DateTime<Utc>,

    pub updated_at: Option<DateTime<Utc>>,
}

/// Manufacturing Cost entity relations
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::models::work_order::Entity",
        from = "Column::WorkOrderId",
        to = "crate::models::work_order::Column::Id"
    )]
    WorkOrder,
}

impl Related<crate::models::work_order::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WorkOrder.def()
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C: ConnectionTrait>(
        self,
        _db: &C,
        insert: bool,
    ) -> Result<Self, DbErr> {
        let mut active_model = self;
        if insert {
            active_model.set_id_if_needed();
        }
        Ok(active_model)
    }
}

impl ActiveModel {
    fn set_id_if_needed(&mut self) {
        if self.id.is_not_set() {
            // i32 primary key: let the database assign it
        }
    }
}
