use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, Set};
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};
use chrono::{DateTime, Utc};
use crate::models::billofmaterials::{LineType, SupplyType, LineItemStatus};

/// Line item model for Bill of Materials
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "bill_of_materials_line_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    
    #[validate(length(min = 1, max = 50, message = "BOM number must be between 1-50 characters"))]
    pub bill_of_materials_number: String,
    
    pub line_type: LineType,
    
    #[validate(length(min = 1, max = 50, message = "Part number must be between 1-50 characters"))]
    pub part_number: String,
    
    #[validate(length(min = 1, max = 100, message = "Part name must be between 1-100 characters"))]
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

impl ActiveModelBehavior for ActiveModel {
    /// Hook that is triggered before insert/update
    fn before_save(mut self, insert: bool) -> Result<Self, DbErr> {
        let now = Utc::now();
        self.updated_at = Set(now);
        
        if insert {
            self.created_at = Set(now);
        }
        
        Ok(self)
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
        item.validate()?;
        Ok(item)
    }
}