use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};
use chrono::{DateTime, Utc, NaiveDateTime};
use uuid::Uuid;

/// Order Note entity model.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "order_notes")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    
    #[sea_orm(column_type = "Uuid")]
    pub order_id: Uuid,
    
    #[validate(length(min = 1, max = 1000, message = "Note must be between 1 and 1000 characters"))]
    pub note: String,
    
    pub created_at: NaiveDateTime,
    
    pub created_by: Option<String>,
}

/// Order Note entity relations
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::models::order_entity::Entity",
        from = "Column::OrderId",
        to = "crate::models::order_entity::Column::Id"
    )]
    Order,
}

impl Related<crate::models::order_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Order.def()
    }
}

impl ActiveModelBehavior for ActiveModel {
    /// Before save hook
    fn before_save(mut self, insert: bool) -> Result<Self, DbErr> {
        // Set timestamps for new records
        if insert {
            self.created_at = Set(Utc::now().naive_utc());
        }
        
        Ok(self)
    }
}

impl Model {
    /// Creates a new order note.
    pub fn new(
        order_id: Uuid,
        note: String,
        created_by: Option<String>,
    ) -> Result<Self, ValidationError> {
        let now = Utc::now().naive_utc();
        
        let order_note = Self {
            id: 0, // This will be set by the database
            order_id,
            note,
            created_at: now,
            created_by,
        };
        
        // Validate the new order note
        order_note.validate()?;
        
        Ok(order_note)
    }
}