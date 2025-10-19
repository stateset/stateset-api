use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Allocation Status enumeration
#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum AllocationStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,

    #[sea_orm(string_value = "Allocated")]
    Allocated,

    #[sea_orm(string_value = "PartiallyAllocated")]
    PartiallyAllocated,

    #[sea_orm(string_value = "Fulfilled")]
    Fulfilled,

    #[sea_orm(string_value = "Cancelled")]
    Cancelled,
}

impl fmt::Display for AllocationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AllocationStatus::Pending => write!(f, "Pending"),
            AllocationStatus::Allocated => write!(f, "Allocated"),
            AllocationStatus::PartiallyAllocated => write!(f, "PartiallyAllocated"),
            AllocationStatus::Fulfilled => write!(f, "Fulfilled"),
            AllocationStatus::Cancelled => write!(f, "Cancelled"),
        }
    }
}

/// Inventory Allocation entity model.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "inventory_allocations")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub inventory_level_id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub product_id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub warehouse_id: Uuid,

    pub reference_type: String,

    #[sea_orm(column_type = "Uuid")]
    pub reference_id: Uuid,

    pub quantity_allocated: i32,

    pub quantity_fulfilled: i32,

    pub status: AllocationStatus,

    pub created_at: DateTime<Utc>,

    pub updated_at: DateTime<Utc>,

    pub expires_at: Option<DateTime<Utc>>,
}

/// Inventory Allocation entity relations
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
    /// Creates a new Inventory Allocation.
    pub fn new(
        inventory_level_id: Uuid,
        product_id: Uuid,
        warehouse_id: Uuid,
        reference_type: String,
        reference_id: Uuid,
        quantity_allocated: i32,
        expires_at: Option<DateTime<Utc>>,
    ) -> Self {
        let now = Utc::now();

        Self {
            id: Uuid::new_v4(),
            inventory_level_id,
            product_id,
            warehouse_id,
            reference_type,
            reference_id,
            quantity_allocated,
            quantity_fulfilled: 0,
            status: AllocationStatus::Allocated,
            created_at: now,
            updated_at: now,
            expires_at,
        }
    }

    /// Updates the allocation with fulfilled quantity.
    pub fn fulfill(&mut self, quantity_fulfilled: i32) {
        self.quantity_fulfilled += quantity_fulfilled;

        // Update status based on fulfilled quantity
        if self.quantity_fulfilled >= self.quantity_allocated {
            self.status = AllocationStatus::Fulfilled;
        } else if self.quantity_fulfilled > 0 {
            self.status = AllocationStatus::PartiallyAllocated;
        }

        self.updated_at = Utc::now();
    }

    /// Updates the allocated quantity.
    pub fn update_allocation(&mut self, new_quantity: i32) {
        self.quantity_allocated = new_quantity;

        // Update status based on new quantity and fulfilled quantity
        if self.quantity_fulfilled >= new_quantity {
            self.status = AllocationStatus::Fulfilled;
        } else if self.quantity_fulfilled > 0 {
            self.status = AllocationStatus::PartiallyAllocated;
        } else {
            self.status = AllocationStatus::Allocated;
        }

        self.updated_at = Utc::now();
    }

    /// Cancels the allocation.
    pub fn cancel(&mut self) {
        self.status = AllocationStatus::Cancelled;
        self.updated_at = Utc::now();
    }

    /// Checks if the allocation is expired.
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            expires_at < Utc::now()
        } else {
            false
        }
    }

    /// Extends the expiration date.
    pub fn extend_expiration(&mut self, new_expiration: DateTime<Utc>) {
        self.expires_at = Some(new_expiration);
        self.updated_at = Utc::now();
    }
}
