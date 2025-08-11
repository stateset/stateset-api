use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "work_order_tasks")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    
    pub work_order_id: Uuid,
    
    #[validate(length(min = 1, max = 200))]
    pub task_name: String,
    
    pub description: Option<String>,
    
    pub status: String,
    
    pub assigned_to: Option<String>,
    
    pub estimated_hours: Option<f64>,
    
    pub actual_hours: Option<f64>,
    
    pub created_at: DateTime<Utc>,
    
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}