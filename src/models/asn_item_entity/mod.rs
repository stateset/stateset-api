use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// ASN Item Status enumeration
#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum ASNItemStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,

    #[sea_orm(string_value = "Received")]
    Received,

    #[sea_orm(string_value = "PartiallyReceived")]
    PartiallyReceived,

    #[sea_orm(string_value = "Rejected")]
    Rejected,

    #[sea_orm(string_value = "Cancelled")]
    Cancelled,
}

/// Advanced Shipping Notice Item entity model.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "asn_items")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub asn_id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub product_id: Uuid,

    pub product_name: String,

    pub product_sku: String,

    #[validate(range(min = 1))]
    pub quantity_expected: i32,

    pub quantity_received: i32,

    pub unit_price: Option<f64>,

    pub status: ASNItemStatus,

    pub lot_numbers: Option<String>,

    pub serial_numbers: Option<String>,

    pub notes: Option<String>,

    pub created_at: DateTime<Utc>,

    pub updated_at: DateTime<Utc>,
}

/// ASN Item entity relations
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::models::asn_entity::Entity",
        from = "Column::AsnId",
        to = "crate::models::asn_entity::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    ASN,
}

impl Related<crate::models::asn_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ASN.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Creates a new ASN Item.
    pub fn new(
        asn_id: Uuid,
        product_id: Uuid,
        product_name: String,
        product_sku: String,
        quantity_expected: i32,
        unit_price: Option<f64>,
        lot_numbers: Option<String>,
        serial_numbers: Option<String>,
        notes: Option<String>,
    ) -> Self {
        let now = Utc::now();

        Self {
            id: Uuid::new_v4(),
            asn_id,
            product_id,
            product_name,
            product_sku,
            quantity_expected,
            quantity_received: 0,
            unit_price,
            status: ASNItemStatus::Pending,
            lot_numbers,
            serial_numbers,
            notes,
            created_at: now,
            updated_at: now,
        }
    }

    /// Updates the quantity received.
    pub fn update_quantity_received(&mut self, new_quantity: i32) {
        self.quantity_received = new_quantity;

        // Update the status based on the new quantity
        if new_quantity == 0 {
            self.status = ASNItemStatus::Pending;
        } else if new_quantity < self.quantity_expected {
            self.status = ASNItemStatus::PartiallyReceived;
        } else {
            self.status = ASNItemStatus::Received;
        }

        self.updated_at = Utc::now();
    }

    /// Marks the item as rejected.
    pub fn reject(&mut self, notes: Option<String>) {
        self.status = ASNItemStatus::Rejected;

        if let Some(rejection_notes) = notes {
            match &self.notes {
                Some(existing_notes) => {
                    self.notes = Some(format!(
                        "{}\nRejection: {}",
                        existing_notes, rejection_notes
                    ));
                }
                None => {
                    self.notes = Some(format!("Rejection: {}", rejection_notes));
                }
            }
        }

        self.updated_at = Utc::now();
    }

    /// Cancels the ASN Item.
    pub fn cancel(&mut self) {
        self.status = ASNItemStatus::Cancelled;
        self.updated_at = Utc::now();
    }
}
