use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "pick_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(column_type = "Uuid")]
    pub pick_id: Uuid,
    pub item_number: String,
    pub item_description: String,
    pub quantity_to_pick: f64,
    pub quantity_picked: f64,
    pub pick_location: String,
    pub status: PickItemStatus,
    pub inventory_id: Option<i32>,
    pub uom: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::picks::Entity",
        from = "Column::PickId",
        to = "super::picks::Column::Id"
    )]
    Pick,
}

impl Related<super::picks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Pick.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum PickItemStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,
    #[sea_orm(string_value = "Partially Picked")]
    PartiallyPicked,
    #[sea_orm(string_value = "Picked")]
    Picked,
    #[sea_orm(string_value = "Cancelled")]
    Cancelled,
}

impl std::fmt::Display for PickItemStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PickItemStatus::Pending => write!(f, "Pending"),
            PickItemStatus::PartiallyPicked => write!(f, "Partially Picked"),
            PickItemStatus::Picked => write!(f, "Picked"),
            PickItemStatus::Cancelled => write!(f, "Cancelled"),
        }
    }
}

impl Model {
    pub fn new(
        pick_id: Uuid,
        item_number: String,
        item_description: String,
        quantity_to_pick: f64,
        pick_location: String,
        uom: String,
        inventory_id: Option<i32>,
    ) -> Self {
        Self {
            id: 0, // Will be assigned by the database
            pick_id,
            item_number,
            item_description,
            quantity_to_pick,
            quantity_picked: 0.0,
            pick_location,
            status: PickItemStatus::Pending,
            inventory_id,
            uom,
        }
    }

    pub fn pick(&mut self, quantity: f64) -> Result<(), String> {
        if quantity <= 0.0 {
            return Err("Quantity must be positive".to_string());
        }

        if self.quantity_picked + quantity > self.quantity_to_pick {
            return Err("Cannot pick more than the requested quantity".to_string());
        }

        self.quantity_picked += quantity;

        if self.quantity_picked == self.quantity_to_pick {
            self.status = PickItemStatus::Picked;
        } else if self.quantity_picked > 0.0 {
            self.status = PickItemStatus::PartiallyPicked;
        }

        Ok(())
    }

    pub fn cancel(&mut self) -> Result<(), String> {
        if self.status != PickItemStatus::Picked {
            self.status = PickItemStatus::Cancelled;
            Ok(())
        } else {
            Err("Cannot cancel already picked items".to_string())
        }
    }

    pub fn get_remaining_quantity(&self) -> f64 {
        self.quantity_to_pick - self.quantity_picked
    }
}
