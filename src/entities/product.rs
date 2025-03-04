use sea_orm::entity::prelude::*;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use rust_decimal::Decimal;
use validator::Validate;

/// Product entity
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "products")]
pub struct Model {
    /// Primary key
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,

    /// Product name
    #[validate(length(min = 1, max = 255, message = "Product name must be between 1 and 255 characters"))]
    pub name: String,

    /// Product description
    #[validate(length(max = 2000, message = "Description cannot exceed 2000 characters"))]
    pub description: Option<String>,

    /// SKU (Stock Keeping Unit)
    #[validate(length(min = 1, max = 100, message = "SKU must be between 1 and 100 characters"))]
    pub sku: String,

    /// Product base price
    #[validate(range(min = 0, message = "Price cannot be negative"))]
    pub price: Decimal,

    /// Currency for the price (e.g., USD, EUR)
    #[validate(length(min = 3, max = 3, message = "Currency must be a 3-letter code"))]
    pub currency: String,

    /// Weight in kilograms
    pub weight_kg: Option<Decimal>,

    /// Dimensions in cm (e.g., "10x20x30" for length x width x height)
    pub dimensions_cm: Option<String>,

    /// Barcode or UPC
    pub barcode: Option<String>,

    /// Product brand
    pub brand: Option<String>,

    /// Manufacturer
    pub manufacturer: Option<String>,

    /// Is the product active
    pub is_active: bool,

    /// Is the product digital (non-physical)
    pub is_digital: bool,

    /// URL to product image (primary)
    #[validate(url(message = "Image URL must be a valid URL"))]
    pub image_url: Option<String>,

    /// Product category ID
    pub category_id: Option<Uuid>,

    /// Minimum quantity for reorder
    pub reorder_point: Option<i32>,

    /// Tax rate as a decimal (e.g., 0.07 for 7%)
    pub tax_rate: Option<Decimal>,

    /// Cost price (used for margin calculations)
    pub cost_price: Option<Decimal>,

    /// MSRP (Manufacturer's Suggested Retail Price)
    pub msrp: Option<Decimal>,

    /// Tags for the product (comma-separated)
    pub tags: Option<String>,

    /// Meta title for SEO
    pub meta_title: Option<String>,

    /// Meta description for SEO
    pub meta_description: Option<String>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: Option<DateTime<Utc>>,
}

/// Product entity relations
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::inventory_items::Entity")]
    InventoryItems,
}

impl Related<super::inventory_items::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InventoryItems.def()
    }
}

impl ActiveModelBehavior for ActiveModel {
    /// Validates and prepares the model before saving
    fn before_save(mut self, insert: bool) -> Result<Self, DbErr> {
        // Set is_active to true by default for new products
        if insert && self.is_active.as_ref().is_none() {
            self.is_active = Set(true);
        }

        // Set is_digital to false by default for new products
        if insert && self.is_digital.as_ref().is_none() {
            self.is_digital = Set(false);
        }

        // Set created_at and updated_at timestamps
        if insert {
            self.created_at = Set(Utc::now());
        }
        self.updated_at = Set(Some(Utc::now()));

        // Validate the product model
        let model: Model = self.clone().try_into().map_err(|_| {
            DbErr::Custom("Failed to convert ActiveModel to Model for validation".to_string())
        })?;

        if let Err(err) = model.validate() {
            return Err(DbErr::Custom(format!("Validation error: {}", err)));
        }

        Ok(self)
    }
}