use sea_orm::entity::prelude::*;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use serde::{Serialize, Deserialize};

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
    #[sea_orm(belongs_to = "super::warranty::Entity", from = "Column::WarrantyId", to = "super::warranty::Column::Id")]
    Warranty,
}

impl Related<super::warranty::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Warranty.def()
    }
}

impl ActiveModelBehavior for ActiveModel {
    // Add business logic here
    fn before_save(mut self, insert: bool) -> Result<Self, DbErr> {
        // Set default status for new warranty claims
        if insert && !matches!(self.status, sea_orm::ActiveValue::Set(_)) {
            self.status = Set("submitted".to_string());
        }
        
        // Validate resolved date and resolution
        if let sea_orm::ActiveValue::Set(status) = &self.status {
            if status == "resolved" {
                // Check if resolved date is provided
                match &self.resolved_date {
                    sea_orm::ActiveValue::Set(_) => {}, // It's set, so we're good
                    _ => return Err(DbErr::Custom("Resolved date is required for resolved claims".to_string()))
                }
                
                // Check if resolution is provided
                match &self.resolution {
                    sea_orm::ActiveValue::Set(_) => {}, // It's set, so we're good
                    _ => return Err(DbErr::Custom("Resolution notes are required for resolved claims".to_string()))
                }
            }
        }
        
        Ok(self)
    }
}