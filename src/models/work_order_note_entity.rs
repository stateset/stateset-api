use chrono::{DateTime, NaiveDateTime, Utc};
use async_trait::async_trait;
use sea_orm::entity::prelude::*;
use sea_orm::Set;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};
use uuid::Uuid;

/// Work Order Note entity model.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "work_order_notes")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,

    pub work_order_id: i32,

    #[validate(length(
        min = 1,
        max = 1000,
        message = "Note must be between 1 and 1000 characters"
    ))]
    pub note: String,

    pub created_at: DateTime<Utc>,

    pub created_by: Option<String>,
}

/// Work Order Note entity relations
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

impl Model {
    /// Creates a new work order note.
    pub fn new(
        work_order_id: i32,
        note: String,
        created_by: Option<String>,
    ) -> Result<Self, ValidationError> {
        let now = Utc::now();

        let work_order_note = Self {
            id: 0, // This will be set by the database
            work_order_id,
            note,
            created_at: now,
            created_by,
        };

        // Validate the new work order note
        work_order_note.validate().map_err(|_| ValidationError::new("Work order note validation failed"))?;

        Ok(work_order_note)
    }
}

/// Convenience alias for the NewWorkOrderNote type
pub type NewWorkOrderNote = ActiveModel;
