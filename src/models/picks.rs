use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::NaiveDate;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "picks")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    pub number: Option<i32>,
    pub work_order_number: Option<i32>,
    pub bill_of_materials_number: Option<i32>,
    pub location: Option<String>,
    pub pick_method: Option<String>,
    pub date_created: Option<NaiveDate>,
    pub site: Option<String>,
    pub status: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::work_order::Entity",
        from = "Column::WorkOrderNumber",
        to = "super::work_order::Column::Number"
    )]
    WorkOrder,
    #[sea_orm(
        belongs_to = "super::bill_of_materials::Entity",
        from = "Column::BillOfMaterialsNumber",
        to = "super::bill_of_materials::Column::Number"
    )]
    BillOfMaterials,
    #[sea_orm(has_many = "super::pick_item::Entity")]
    PickItems,
}

impl Related<super::work_order::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WorkOrder.def()
    }
}

impl Related<super::bill_of_materials::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BillOfMaterials.def()
    }
}

impl Related<super::pick_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PickItems.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String")]
pub enum PickStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,
    #[sea_orm(string_value = "In Progress")]
    InProgress,
    #[sea_orm(string_value = "Completed")]
    Completed,
    #[sea_orm(string_value = "Cancelled")]
    Cancelled,
}

#[derive(Debug, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String")]
pub enum PickMethod {
    #[sea_orm(string_value = "Single Order")]
    SingleOrder,
    #[sea_orm(string_value = "Batch")]
    Batch,
    #[sea_orm(string_value = "Zone")]
    Zone,
    #[sea_orm(string_value = "Wave")]
    Wave,
}

impl Model {
    pub fn new(
        work_order_number: Option<i32>,
        bill_of_materials_number: Option<i32>,
        location: Option<String>,
        pick_method: Option<String>,
        site: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            number: None,
            work_order_number,
            bill_of_materials_number,
            location,
            pick_method,
            date_created: Some(chrono::Local::now().naive_local().date()),
            site,
            status: Some(PickStatus::Pending.to_string()),
        }
    }

    pub fn start_pick(&mut self) -> Result<(), String> {
        if self.status == Some(PickStatus::Pending.to_string()) {
            self.status = Some(PickStatus::InProgress.to_string());
            Ok(())
        } else {
            Err("Pick can only be started from Pending status".to_string())
        }
    }

    pub fn complete_pick(&mut self) -> Result<(), String> {
        if self.status == Some(PickStatus::InProgress.to_string()) {
            self.status = Some(PickStatus::Completed.to_string());
            Ok(())
        } else {
            Err("Pick can only be completed from In Progress status".to_string())
        }
    }

    pub fn cancel_pick(&mut self) -> Result<(), String> {
        if self.status != Some(PickStatus::Completed.to_string()) {
            self.status = Some(PickStatus::Cancelled.to_string());
            Ok(())
        } else {
            Err("Completed picks cannot be cancelled".to_string())
        }
    }

    pub async fn get_pick_items(&self, db: &DatabaseConnection) -> Result<Vec<super::pick_item::Model>, DbErr> {
        super::pick_item::Entity::find()
            .filter(super::pick_item::Column::PickId.eq(self.id))
            .all(db)
            .await
    }
}

// PickItem entity has been moved to its own file: pick_item.rs