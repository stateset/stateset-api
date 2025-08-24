use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{Set, ActiveValue};
use serde::{Deserialize, Serialize};
use std::fmt;
use validator::{Validate, ValidationError};
use async_trait::async_trait;
use uuid::Uuid;

/// Bill of Materials main entity
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "bill_of_materials")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,

    #[validate(length(
        min = 1,
        max = 50,
        message = "BOM number must be between 1-50 characters"
    ))]
    pub number: String,

    #[validate(length(min = 1, max = 100, message = "Name must be between 1-100 characters"))]
    pub name: String,

    #[validate(length(max = 500, message = "Description cannot exceed 500 characters"))]
    pub description: Option<String>,

    pub groups: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub valid: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    // TODO: Uncomment when bill_of_materials_line_item entity is implemented
    // #[sea_orm(has_many = "super::bill_of_materials_line_item::Entity")]
    // LineItems,
}

// TODO: Uncomment when bill_of_materials_line_item entity is implemented
// impl Related<super::bill_of_materials_line_item::Entity> for Entity {
//     fn to() -> RelationDef {
//         Relation::LineItems.def()
//     }
// }

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
            self.id = Set(Uuid::new_v4());
        }
    }
}

/// Line item type enumeration to replace string-based values
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum LineType {
    #[sea_orm(string_value = "Component")]
    Component,

    #[sea_orm(string_value = "Assembly")]
    Assembly,

    #[sea_orm(string_value = "Material")]
    Material,

    #[sea_orm(string_value = "Other")]
    Other,
}

impl fmt::Display for LineType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LineType::Component => write!(f, "Component"),
            LineType::Assembly => write!(f, "Assembly"),
            LineType::Material => write!(f, "Material"),
            LineType::Other => write!(f, "Other"),
        }
    }
}

/// Supply type enumeration to replace string-based values
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum SupplyType {
    #[sea_orm(string_value = "Purchase")]
    Purchase,

    #[sea_orm(string_value = "Manufacture")]
    Manufacture,

    #[sea_orm(string_value = "Transfer")]
    Transfer,
}

impl fmt::Display for SupplyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SupplyType::Purchase => write!(f, "Purchase"),
            SupplyType::Manufacture => write!(f, "Manufacture"),
            SupplyType::Transfer => write!(f, "Transfer"),
        }
    }
}

/// Line item status enumeration
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

impl fmt::Display for LineItemStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LineItemStatus::Active => write!(f, "Active"),
            LineItemStatus::Inactive => write!(f, "Inactive"),
            LineItemStatus::Pending => write!(f, "Pending"),
        }
    }
}

// LineItem model moved to models/bom_line_item/mod.rs

impl Model {
    /// Create a new Bill of Materials
    pub fn new(
        number: String,
        name: String,
        description: Option<String>,
        groups: Option<String>,
    ) -> Result<Self, ValidationError> {
        let now = Utc::now();
        let bill_of_materials = Self {
            id: 0,
            number,
            name,
            description,
            groups,
            created_at: now,
            updated_at: now,
            valid: true,
        };
        bill_of_materials.validate().map_err(|_| ValidationError::new("Validation failed"))?;
        Ok(bill_of_materials)
    }

    /// Add a line item to this BOM
    pub async fn add_line_item(
        &self,
        db: &DatabaseConnection,
        line_item: crate::models::bom_line_item::Model,
    ) -> Result<crate::models::bom_line_item::Model, Box<dyn std::error::Error>> {
        line_item.validate().map_err(|_| ValidationError::new("Validation failed"))?;

        let mut active_model: crate::models::bom_line_item::ActiveModel = line_item.into();
        active_model.bill_of_materials_id = Set(self.id);
        active_model.bill_of_materials_number = Set(self.number.clone());

        let result = active_model.insert(db).await?;
        Ok(result)
    }

    /// Invalidate this BOM
    pub async fn invalidate(
        &mut self,
        db: &DatabaseConnection,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        self.valid = false;
        self.updated_at = Utc::now();

        let mut active_model: ActiveModel = self.clone().into();
        active_model.valid = Set(false);
        active_model.updated_at = Set(Utc::now());

        let result = active_model.update(db).await?;
        Ok(result)
    }

    /// Find all line items for this BOM
    pub async fn find_line_items(
        &self,
        db: &DatabaseConnection,
    ) -> Result<Vec<crate::models::bom_line_item::Model>, DbErr> {
        crate::models::bom_line_item::Entity::find()
            .filter(crate::models::bom_line_item::Column::BillOfMaterialsId.eq(self.id))
            .all(db)
            .await
    }
}

// LineItemModel implementation moved to bom_line_item module
