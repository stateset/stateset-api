use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Inventory Level entity model.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "inventory_levels")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub product_id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub warehouse_id: Uuid,

    pub product_name: String,

    pub product_sku: String,

    pub on_hand_quantity: i32,

    pub reserved_quantity: i32,

    pub allocated_quantity: i32,

    pub available_quantity: i32,

    pub minimum_quantity: i32,

    pub maximum_quantity: i32,

    pub reorder_point: i32,

    pub reorder_quantity: i32,

    pub status: String,

    pub last_count_date: Option<DateTime<Utc>>,

    pub created_at: DateTime<Utc>,

    pub updated_at: DateTime<Utc>,
}

/// Inventory Level entity relations
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "crate::models::inventory_transaction_entity::Entity")]
    Transactions,

    #[sea_orm(has_many = "crate::models::inventory_allocation_entity::Entity")]
    Allocations,

    #[sea_orm(has_many = "crate::models::inventory_reservation_entity::Entity")]
    Reservations,
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Creates a new Inventory Level.
    pub fn new(
        product_id: Uuid,
        warehouse_id: Uuid,
        product_name: String,
        product_sku: String,
        on_hand_quantity: i32,
        minimum_quantity: i32,
        maximum_quantity: i32,
        reorder_point: i32,
        reorder_quantity: i32,
    ) -> Self {
        let now = Utc::now();

        Self {
            id: Uuid::new_v4(),
            product_id,
            warehouse_id,
            product_name,
            product_sku,
            on_hand_quantity,
            reserved_quantity: 0,
            allocated_quantity: 0,
            available_quantity: on_hand_quantity,
            minimum_quantity,
            maximum_quantity,
            reorder_point,
            reorder_quantity,
            status: "Active".to_string(),
            last_count_date: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Adjusts the on-hand quantity and recalculates available quantity.
    pub fn adjust_on_hand(&mut self, adjustment: i32) {
        self.on_hand_quantity += adjustment;
        self.recalculate_available();
        self.updated_at = Utc::now();
    }

    /// Allocates inventory and recalculates available quantity.
    pub fn allocate(&mut self, quantity: i32) -> bool {
        if quantity <= self.available_quantity {
            self.allocated_quantity += quantity;
            self.recalculate_available();
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Deallocates inventory and recalculates available quantity.
    pub fn deallocate(&mut self, quantity: i32) {
        self.allocated_quantity = (self.allocated_quantity - quantity).max(0);
        self.recalculate_available();
        self.updated_at = Utc::now();
    }

    /// Reserves inventory and recalculates available quantity.
    pub fn reserve(&mut self, quantity: i32) -> bool {
        if quantity <= self.available_quantity {
            self.reserved_quantity += quantity;
            self.recalculate_available();
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Releases reserved inventory and recalculates available quantity.
    pub fn release_reservation(&mut self, quantity: i32) {
        self.reserved_quantity = (self.reserved_quantity - quantity).max(0);
        self.recalculate_available();
        self.updated_at = Utc::now();
    }

    /// Recalculates the available quantity.
    fn recalculate_available(&mut self) {
        self.available_quantity =
            self.on_hand_quantity - self.reserved_quantity - self.allocated_quantity;
    }

    /// Sets the last count date.
    pub fn set_last_count_date(&mut self, date: DateTime<Utc>) {
        self.last_count_date = Some(date);
        self.updated_at = Utc::now();
    }

    /// Updates the reorder parameters.
    pub fn update_reorder_params(
        &mut self,
        minimum_quantity: i32,
        maximum_quantity: i32,
        reorder_point: i32,
        reorder_quantity: i32,
    ) {
        self.minimum_quantity = minimum_quantity;
        self.maximum_quantity = maximum_quantity;
        self.reorder_point = reorder_point;
        self.reorder_quantity = reorder_quantity;
        self.updated_at = Utc::now();
    }

    /// Checks if the inventory level is below the reorder point.
    pub fn needs_reorder(&self) -> bool {
        self.available_quantity <= self.reorder_point
    }
}
