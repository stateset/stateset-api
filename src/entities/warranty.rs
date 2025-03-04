use sea_orm::entity::prelude::*;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "warranties")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub warranty_number: String,
    pub product_id: Uuid,
    pub customer_id: Uuid,
    pub order_id: Option<Uuid>,
    pub status: String,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub description: Option<String>,
    pub terms: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::warranty_claim::Entity")]
    WarrantyClaims,
}

impl Related<super::warranty_claim::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WarrantyClaims.def()
    }
}

impl ActiveModelBehavior for ActiveModel {
    // Add business logic here
    fn before_save(mut self, insert: bool) -> Result<Self, DbErr> {
        // Validate dates - end date must be after start date
        if let (sea_orm::ActiveValue::Set(start), sea_orm::ActiveValue::Set(end)) = 
            (&self.start_date, &self.end_date) 
        {
            if end <= start {
                return Err(DbErr::Custom("Warranty end date must be after start date".to_string()));
            }
        }
        
        // Set default status for new warranties
        if insert && !matches!(self.status, sea_orm::ActiveValue::Set(_)) {
            self.status = Set("active".to_string());
        }
        
        Ok(self)
    }
}