use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::prelude::*;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "order_items")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub order_id: Uuid,
    pub product_id: Uuid,
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    pub unit_price: Decimal,
    pub total_price: Decimal,
    pub discount: Decimal,
    pub tax_rate: Decimal,
    pub tax_amount: Decimal,
    pub status: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::order::Entity",
        from = "Column::OrderId",
        to = "super::order::Column::Id"
    )]
    Order,
}

impl Related<super::order::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Order.def()
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
        }
        
        if let ActiveValue::NotSet = active_model.updated_at {
            active_model.updated_at = Set(Some(now));
        }
        
        Ok(active_model)
    }
}
