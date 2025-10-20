use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Inventory Transaction Type enumeration
#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum InventoryTransactionType {
    #[sea_orm(string_value = "Adjustment")]
    Adjustment,

    #[sea_orm(string_value = "Receipt")]
    Receipt,

    #[sea_orm(string_value = "Sale")]
    Sale,

    #[sea_orm(string_value = "Return")]
    Return,

    #[sea_orm(string_value = "Transfer")]
    Transfer,

    #[sea_orm(string_value = "Scrap")]
    Scrap,

    #[sea_orm(string_value = "Count")]
    Count,

    #[sea_orm(string_value = "Production")]
    Production,
}

impl fmt::Display for InventoryTransactionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InventoryTransactionType::Adjustment => write!(f, "Adjustment"),
            InventoryTransactionType::Receipt => write!(f, "Receipt"),
            InventoryTransactionType::Sale => write!(f, "Sale"),
            InventoryTransactionType::Return => write!(f, "Return"),
            InventoryTransactionType::Transfer => write!(f, "Transfer"),
            InventoryTransactionType::Scrap => write!(f, "Scrap"),
            InventoryTransactionType::Count => write!(f, "Count"),
            InventoryTransactionType::Production => write!(f, "Production"),
        }
    }
}

/// Inventory Transaction entity model.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "inventory_transactions")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub inventory_level_id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub product_id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub warehouse_id: Uuid,

    pub transaction_type: InventoryTransactionType,

    pub quantity: i32,

    pub reference_type: Option<String>,

    pub reference_id: Option<Uuid>,

    pub notes: Option<String>,

    pub created_by: Option<String>,

    pub created_at: DateTime<Utc>,
}

/// Inventory Transaction entity relations
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::models::inventory_level_entity::Entity",
        from = "Column::InventoryLevelId",
        to = "crate::models::inventory_level_entity::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    InventoryLevel,
}

impl Related<crate::models::inventory_level_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InventoryLevel.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Creates a new Inventory Transaction.
    pub fn new(
        inventory_level_id: Uuid,
        product_id: Uuid,
        warehouse_id: Uuid,
        transaction_type: InventoryTransactionType,
        quantity: i32,
        reference_type: Option<String>,
        reference_id: Option<Uuid>,
        notes: Option<String>,
        created_by: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            inventory_level_id,
            product_id,
            warehouse_id,
            transaction_type,
            quantity,
            reference_type,
            reference_id,
            notes,
            created_by,
            created_at: Utc::now(),
        }
    }

    /// Creates a new adjustment transaction.
    pub fn new_adjustment(
        inventory_level_id: Uuid,
        product_id: Uuid,
        warehouse_id: Uuid,
        quantity: i32,
        notes: Option<String>,
        created_by: Option<String>,
    ) -> Self {
        Self::new(
            inventory_level_id,
            product_id,
            warehouse_id,
            InventoryTransactionType::Adjustment,
            quantity,
            None,
            None,
            notes,
            created_by,
        )
    }

    /// Creates a new receipt transaction.
    pub fn new_receipt(
        inventory_level_id: Uuid,
        product_id: Uuid,
        warehouse_id: Uuid,
        quantity: i32,
        purchase_order_id: Option<Uuid>,
        notes: Option<String>,
        created_by: Option<String>,
    ) -> Self {
        Self::new(
            inventory_level_id,
            product_id,
            warehouse_id,
            InventoryTransactionType::Receipt,
            quantity,
            Some("PurchaseOrder".to_string()),
            purchase_order_id,
            notes,
            created_by,
        )
    }

    /// Creates a new sale transaction.
    pub fn new_sale(
        inventory_level_id: Uuid,
        product_id: Uuid,
        warehouse_id: Uuid,
        quantity: i32,
        order_id: Uuid,
        notes: Option<String>,
        created_by: Option<String>,
    ) -> Self {
        Self::new(
            inventory_level_id,
            product_id,
            warehouse_id,
            InventoryTransactionType::Sale,
            quantity,
            Some("Order".to_string()),
            Some(order_id),
            notes,
            created_by,
        )
    }

    /// Creates a new return transaction.
    pub fn new_return(
        inventory_level_id: Uuid,
        product_id: Uuid,
        warehouse_id: Uuid,
        quantity: i32,
        return_id: Uuid,
        notes: Option<String>,
        created_by: Option<String>,
    ) -> Self {
        Self::new(
            inventory_level_id,
            product_id,
            warehouse_id,
            InventoryTransactionType::Return,
            quantity,
            Some("Return".to_string()),
            Some(return_id),
            notes,
            created_by,
        )
    }

    /// Creates a new count transaction.
    pub fn new_count(
        inventory_level_id: Uuid,
        product_id: Uuid,
        warehouse_id: Uuid,
        quantity: i32,
        cycle_count_id: Option<Uuid>,
        notes: Option<String>,
        created_by: Option<String>,
    ) -> Self {
        Self::new(
            inventory_level_id,
            product_id,
            warehouse_id,
            InventoryTransactionType::Count,
            quantity,
            Some("CycleCount".to_string()),
            cycle_count_id,
            notes,
            created_by,
        )
    }
}
