use sea_orm::entity::prelude::*;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use rust_decimal::Decimal;

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
    #[sea_orm(belongs_to = "super::order::Entity", from = "Column::OrderId", to = "super::order::Column::Id")]
    Order,
}

impl Related<super::order::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Order.def()
    }
}

impl ActiveModelBehavior for ActiveModel {
    // Add business logic here, such as calculating the total price
    fn before_save(mut self, insert: bool) -> Result<Self, DbErr> {
        // Calculate total price on inserts or when quantity or unit_price changes
        if insert || 
           matches!(self.quantity, sea_orm::ActiveValue::Set(_)) || 
           matches!(self.unit_price, sea_orm::ActiveValue::Set(_)) 
        {
            if let (sea_orm::ActiveValue::Set(qty), sea_orm::ActiveValue::Set(price)) = 
                (&self.quantity, &self.unit_price) 
            {
                let quantity_dec = Decimal::from(*qty);
                
                let discount = match &self.discount {
                    sea_orm::ActiveValue::Set(d) => *d,
                    _ => Decimal::from(0)
                };
                
                // Calculate line item total
                let total = *price * quantity_dec - discount;
                self.total_price = Set(total);
                
                // Calculate tax
                if let sea_orm::ActiveValue::Set(tax_rate) = &self.tax_rate {
                    let tax_amount = total * *tax_rate / Decimal::from(100);
                    self.tax_amount = Set(tax_amount);
                }
            }
        }
        
        Ok(self)
    }
}