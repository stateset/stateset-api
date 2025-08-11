use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// ASN Note Type enumeration
#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum ASNNoteType {
    #[sea_orm(string_value = "General")]
    General,

    #[sea_orm(string_value = "Status")]
    Status,

    #[sea_orm(string_value = "Issue")]
    Issue,

    #[sea_orm(string_value = "System")]
    System,
}

/// Advanced Shipping Notice Note entity model.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "asn_notes")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub asn_id: Uuid,

    pub note_text: String,

    pub note_type: ASNNoteType,

    pub created_by: Option<String>,

    pub created_at: DateTime<Utc>,
}

/// ASN Note entity relations
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
    /// Creates a new ASN Note.
    pub fn new(
        asn_id: Uuid,
        note_text: String,
        note_type: ASNNoteType,
        created_by: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            asn_id,
            note_text,
            note_type,
            created_by,
            created_at: Utc::now(),
        }
    }

    /// Creates a new status note.
    pub fn new_status_note(
        asn_id: Uuid,
        status_message: String,
        created_by: Option<String>,
    ) -> Self {
        Self::new(asn_id, status_message, ASNNoteType::Status, created_by)
    }

    /// Creates a new system note.
    pub fn new_system_note(asn_id: Uuid, system_message: String) -> Self {
        Self::new(asn_id, system_message, ASNNoteType::System, None)
    }
}
