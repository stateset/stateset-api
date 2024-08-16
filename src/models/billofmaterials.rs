use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::{DateTime, Utc};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "bill_of_materials")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub number: String,
    pub name: String,
    pub description: Option<String>,
    pub groups: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub valid: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::bill_of_materials_line_item::Entity")]
    BillOfMaterialsLineItems,
}

impl Related<super::bill_of_materials_line_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BillOfMaterialsLineItems.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "bill_of_materials_line_items")]
pub struct BillOfMaterialsLineItem {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub bill_of_materials_number: String,
    pub line_type: String,
    pub part_number: String,
    pub part_name: String,
    pub purchase_supply_type: String,
    #[validate(range(min = 0.0, message = "Quantity must be non-negative"))]
    pub quantity: f64,
    pub status: LineItemStatus,
    pub bill_of_materials_id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum BillOfMaterialsLineItemRelation {
    #[sea_orm(
        belongs_to = "super::bill_of_materials::Entity",
        from = "Column::BillOfMaterialsId",
        to = "super::bill_of_materials::Column::Id"
    )]
    BillOfMaterials,
}

impl Related<super::bill_of_materials::Entity> for BillOfMaterialsLineItem {
    fn to() -> RelationDef {
        BillOfMaterialsLineItemRelation::BillOfMaterials.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum LineItemStatus {
    #[sea_orm(string_value = "Active")]
    Active,
    #[sea_orm(string_value = "Inactive")]
    Inactive,
    #[sea_orm(string_value = "Pending")]
    Pending,
}

impl Model {
    pub fn new(
        number: String,
        name: String,
        description: Option<String>,
        groups: Option<String>,
    ) -> Result<Self, ValidationError> {
        let now = Utc::now();
        let bill_of_materials = Self {
            id: 0, // Assuming database will auto-increment this
            number,
            name,
            description,
            groups,
            created_at: now,
            updated_at: now,
            valid: true,
        };
        bill_of_materials.validate()?;
        Ok(bill_of_materials)
    }

    pub fn add_line_item(&self, line_item: BillOfMaterialsLineItem) -> Result<(), ValidationError> {
        // Here you would typically save the line item to the database
        // For this example, we'll just validate the line item
        line_item.validate()?;
        Ok(())
    }

    pub fn invalidate(&mut self) {
        self.valid = false;
        self.updated_at = Utc::now();
    }
}

impl BillOfMaterialsLineItem {
    pub fn new(
        bill_of_materials_number: String,
        line_type: String,
        part_number: String,
        part_name: String,
        purchase_supply_type: String,
        quantity: f64,
        status: LineItemStatus,
        bill_of_materials_id: i32,
    ) -> Result<Self, ValidationError> {
        let item = Self {
            id: 0, // Assuming database will auto-increment this
            bill_of_materials_number,
            line_type,
            part_number,
            part_name,
            purchase_supply_type,
            quantity,
            status,
            bill_of_materials_id,
        };
        item.validate()?;
        Ok(item)
    }

    pub fn update_status(&mut self, new_status: LineItemStatus) {
        self.status = new_status;
    }
}