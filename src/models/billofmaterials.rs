use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::{DateTime, Utc};
use std::fmt;

/// Bill of Materials main entity
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "bill_of_materials")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    
    #[validate(length(min = 1, max = 50, message = "BOM number must be between 1-50 characters"))]
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
    #[sea_orm(has_many = "super::bill_of_materials_line_item::Entity")]
    LineItems,
}

impl Related<super::bill_of_materials_line_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LineItems.def()
    }
}

impl ActiveModelBehavior for ActiveModel {
    /// Hook that is triggered before insert
    fn before_save(mut self, insert: bool) -> Result<Self, DbErr> {
        let now = Utc::now();
        self.updated_at = Set(now);
        
        if insert {
            self.created_at = Set(now);
            self.valid = Set(true);
        }
        
        Ok(self)
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

/// Line item model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "bill_of_materials_line_items")]
pub struct LineItemModel {
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
pub enum LineItemRelation {
    #[sea_orm(
        belongs_to = "super::bill_of_materials::Entity",
        from = "Column::BillOfMaterialsId",
        to = "super::bill_of_materials::Column::Id"
    )]
    BillOfMaterials,
}

impl Related<super::bill_of_materials::Entity> for LineItemEntity {
    fn to() -> RelationDef {
        LineItemRelation::BillOfMaterials.def()
    }
}

impl ActiveModelBehavior for LineItemActiveModel {
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
        bill_of_materials.validate()?;
        Ok(bill_of_materials)
    }

    /// Add a line item to this BOM
    pub async fn add_line_item(
        &self, 
        db: &DatabaseConnection,
        line_item: LineItemModel
    ) -> Result<LineItemModel, Box<dyn std::error::Error>> {
        line_item.validate()?;
        
        let mut active_model: LineItemActiveModel = line_item.into();
        active_model.bill_of_materials_id = Set(self.id);
        active_model.bill_of_materials_number = Set(self.number.clone());
        
        let result = active_model.insert(db).await?;
        Ok(result)
    }

    /// Invalidate this BOM
    pub async fn invalidate(
        &mut self,
        db: &DatabaseConnection
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
        db: &DatabaseConnection
    ) -> Result<Vec<LineItemModel>, DbErr> {
        LineItemEntity::find()
            .filter(LineItemColumn::BillOfMaterialsId.eq(self.id))
            .all(db)
            .await
    }
}

impl LineItemModel {
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

    /// Update the status of this line item
    pub async fn update_status(
        &mut self,
        db: &DatabaseConnection,
        new_status: LineItemStatus
    ) -> Result<Self, DbErr> {
        self.status = new_status;
        self.updated_at = Utc::now();
        
        let mut active_model: LineItemActiveModel = self.clone().into();
        active_model.status = Set(new_status);
        active_model.updated_at = Set(Utc::now());
        
        let result = active_model.update(db).await?;
        Ok(result)
    }
}