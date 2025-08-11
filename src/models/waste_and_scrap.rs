use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::{Validate, ValidationError};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "waste_and_scrap")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    #[sea_orm(column_type = "Uuid", nullable)]
    pub work_order_id: Option<Uuid>,
    #[sea_orm(column_type = "Uuid", nullable)]
    pub part_number: Option<Uuid>,
    pub quantity: Option<i32>,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::models::work_order::Entity",
        from = "Column::WorkOrderId",
        to = "crate::models::work_order::Column::Id"
    )]
    WorkOrder,
    #[sea_orm(
        belongs_to = "crate::models::part::Entity",
        from = "Column::PartNumber",
        to = "crate::models::part::Column::Id"
    )]
    Part,
}

impl Related<crate::models::work_order::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WorkOrder.def()
    }
}

impl Related<crate::models::part::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Part.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String")]
pub enum ScrapReason {
    #[sea_orm(string_value = "Defective Material")]
    DefectiveMaterial,
    #[sea_orm(string_value = "Machine Malfunction")]
    MachineMalfunction,
    #[sea_orm(string_value = "Operator Error")]
    OperatorError,
    #[sea_orm(string_value = "Quality Control Rejection")]
    QualityControlRejection,
    #[sea_orm(string_value = "Process Experimentation")]
    ProcessExperimentation,
    #[sea_orm(string_value = "Other")]
    Other,
}

impl Model {
    pub fn new(
        work_order_id: Option<Uuid>,
        part_number: Option<Uuid>,
        quantity: Option<i32>,
        reason: ScrapReason,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            work_order_id,
            part_number,
            quantity,
            reason: reason.to_string(),
            created_at: Utc::now(),
        }
    }

    pub fn update_quantity(&mut self, new_quantity: i32) {
        self.quantity = Some(new_quantity);
    }

    pub fn update_reason(&mut self, new_reason: ScrapReason) {
        self.reason = new_reason.to_string();
    }

    pub async fn get_work_order(
        &self,
        db: &DatabaseConnection,
    ) -> Result<Option<crate::models::work_order::Model>, DbErr> {
        if let Some(work_order_id) = self.work_order_id {
            self.find_related(crate::models::work_order::Entity).one(db).await
        } else {
            Ok(None)
        }
    }

    pub async fn get_part(&self, db: &DatabaseConnection) -> Result<Option<crate::models::part::Model>, DbErr> {
        if let Some(part_number) = self.part_number {
            self.find_related(crate::models::part::Entity).one(db).await
        } else {
            Ok(None)
        }
    }
}

// You might want to implement these in separate files
// Work order entity is defined in work_order.rs
// Parts entity should be defined in a separate file
// Part entity should be in its own file part.rs
