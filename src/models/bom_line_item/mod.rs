use crate::models::billofmaterials::{LineItemStatus, LineType, SupplyType};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::{Validate, ValidationError};

/// Line item model for Bill of Materials
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "bill_of_materials_line_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,

    #[validate(length(
        min = 1,
        max = 50,
        message = "BOM number must be between 1-50 characters"
    ))]
    pub bill_of_materials_number: String,

    pub line_type: LineType,

    #[validate(length(
        min = 1,
        max = 50,
        message = "Part number must be between 1-50 characters"
    ))]
    pub part_number: String,

    #[validate(length(
        min = 1,
        max = 100,
        message = "Part name must be between 1-100 characters"
    ))]
    pub part_name: String,

    pub purchase_supply_type: SupplyType,

    #[validate(range(min = 0.0, message = "Quantity must be non-negative"))]
    pub quantity: f64,

    pub status: LineItemStatus,

    pub bill_of_materials_id: i32,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::models::billofmaterials::Entity",
        from = "Column::BillOfMaterialsId",
        to = "crate::models::billofmaterials::Column::Id"
    )]
    BillOfMaterials,
}

impl Related<crate::models::billofmaterials::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BillOfMaterials.def()
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C: ConnectionTrait>(self, _db: &C, insert: bool) -> Result<Self, DbErr> {
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
    /// Create a new line item
    pub fn new(
        bill_of_materials_number: String,
        line_type: LineType,
        part_number: String,
        part_name: String,
        purchase_supply_type: SupplyType,
        quantity: f64,
        status: LineItemStatus,
        bill_of_materials_id: i32,
    ) -> Result<Self, ValidationError> {
        let now = Utc::now();
        let item = Self {
            id: 0,
            bill_of_materials_number,
            line_type,
            part_number,
            part_name,
            purchase_supply_type,
            quantity,
            status,
            bill_of_materials_id,
            created_at: now,
            updated_at: now,
        };
        item.validate()
            .map_err(|_| ValidationError::new("Validation failed"))?;
        Ok(item)
    }
}
