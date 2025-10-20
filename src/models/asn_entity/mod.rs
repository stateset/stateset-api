use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;
use validator::{Validate, ValidationError};

/// ASN Status enumeration
#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum ASNStatus {
    #[sea_orm(string_value = "Draft")]
    Draft,

    #[sea_orm(string_value = "Submitted")]
    Submitted,

    #[sea_orm(string_value = "InTransit")]
    InTransit,

    #[sea_orm(string_value = "Delivered")]
    Delivered,

    #[sea_orm(string_value = "Completed")]
    Completed,

    #[sea_orm(string_value = "Cancelled")]
    Cancelled,

    #[sea_orm(string_value = "OnHold")]
    OnHold,
}

impl fmt::Display for ASNStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ASNStatus::Draft => write!(f, "Draft"),
            ASNStatus::Submitted => write!(f, "Submitted"),
            ASNStatus::InTransit => write!(f, "InTransit"),
            ASNStatus::Delivered => write!(f, "Delivered"),
            ASNStatus::Completed => write!(f, "Completed"),
            ASNStatus::Cancelled => write!(f, "Cancelled"),
            ASNStatus::OnHold => write!(f, "OnHold"),
        }
    }
}

/// Carrier Type enumeration
#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum CarrierType {
    #[sea_orm(string_value = "UPS")]
    UPS,

    #[sea_orm(string_value = "FedEx")]
    FedEx,

    #[sea_orm(string_value = "USPS")]
    USPS,

    #[sea_orm(string_value = "DHL")]
    DHL,

    #[sea_orm(string_value = "Other")]
    Other,
}

/// Advanced Shipping Notice (ASN) entity model.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "asns")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,

    #[validate(length(
        min = 1,
        max = 50,
        message = "ASN number must be between 1 and 50 characters"
    ))]
    pub asn_number: String,

    pub status: ASNStatus,

    #[sea_orm(column_type = "Uuid")]
    pub supplier_id: Uuid,

    pub supplier_name: String,

    pub expected_delivery_date: Option<DateTime<Utc>>,

    pub shipping_date: Option<DateTime<Utc>>,

    pub carrier_type: Option<CarrierType>,

    pub tracking_number: Option<String>,

    pub shipping_address: String,

    pub notes: Option<String>,

    pub created_at: DateTime<Utc>,

    pub updated_at: DateTime<Utc>,

    pub created_by: Option<String>,

    pub version: i32,
}

/// ASN entity relations
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "crate::models::asn_item_entity::Entity")]
    ASNItems,

    #[sea_orm(has_many = "crate::models::asn_note_entity::Entity")]
    ASNNotes,
}

impl Related<crate::models::asn_item_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ASNItems.def()
    }
}

impl Related<crate::models::asn_note_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ASNNotes.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Creates a new ASN.
    pub fn new(
        asn_number: String,
        supplier_id: Uuid,
        supplier_name: String,
        shipping_address: String,
        expected_delivery_date: Option<DateTime<Utc>>,
        notes: Option<String>,
        created_by: Option<String>,
    ) -> Result<Self, ValidationError> {
        let now = Utc::now();

        let asn = Self {
            id: Uuid::new_v4(),
            asn_number,
            status: ASNStatus::Draft,
            supplier_id,
            supplier_name,
            expected_delivery_date,
            shipping_date: None,
            carrier_type: None,
            tracking_number: None,
            shipping_address,
            notes,
            created_at: now,
            updated_at: now,
            created_by,
            version: 1,
        };

        // Validate the new ASN
        asn.validate()
            .map_err(|_| ValidationError::new("ASN validation failed"))?;

        Ok(asn)
    }

    /// Updates the ASN status.
    pub fn update_status(&mut self, new_status: ASNStatus) -> Result<(), ValidationError> {
        self.status = new_status;
        self.updated_at = Utc::now();
        self.version += 1;

        // Revalidate after update
        self.validate()
            .map_err(|_| ValidationError::new("ASN validation failed"))?;

        Ok(())
    }

    /// Sets shipping information.
    pub fn set_shipping_info(
        &mut self,
        shipping_date: DateTime<Utc>,
        carrier_type: CarrierType,
        tracking_number: String,
    ) -> Result<(), ValidationError> {
        self.shipping_date = Some(shipping_date);
        self.carrier_type = Some(carrier_type);
        self.tracking_number = Some(tracking_number);
        self.updated_at = Utc::now();
        self.version += 1;

        // Revalidate after update
        self.validate()
            .map_err(|_| ValidationError::new("ASN validation failed"))?;

        Ok(())
    }

    /// Mark ASN as in transit.
    pub fn mark_in_transit(&mut self) -> Result<(), ValidationError> {
        if self.status != ASNStatus::Submitted {
            return Err(ValidationError::new(
                "ASN must be in Submitted status to mark as in transit",
            ));
        }

        if self.shipping_date.is_none()
            || self.carrier_type.is_none()
            || self.tracking_number.is_none()
        {
            return Err(ValidationError::new(
                "Shipping information must be provided before marking ASN as in transit",
            ));
        }

        self.status = ASNStatus::InTransit;
        self.updated_at = Utc::now();
        self.version += 1;

        // Revalidate after update
        self.validate()
            .map_err(|_| ValidationError::new("ASN validation failed"))?;

        Ok(())
    }

    /// Mark ASN as delivered.
    pub fn mark_delivered(&mut self) -> Result<(), ValidationError> {
        if self.status != ASNStatus::InTransit {
            return Err(ValidationError::new(
                "ASN must be in InTransit status to mark as delivered",
            ));
        }

        self.status = ASNStatus::Delivered;
        self.updated_at = Utc::now();
        self.version += 1;

        // Revalidate after update
        self.validate()
            .map_err(|_| ValidationError::new("ASN validation failed"))?;

        Ok(())
    }

    /// Place ASN on hold.
    pub fn place_on_hold(&mut self) -> Result<(), ValidationError> {
        if self.status == ASNStatus::Completed || self.status == ASNStatus::Cancelled {
            return Err(ValidationError::new(
                "Cannot place completed or cancelled ASN on hold",
            ));
        }

        self.status = ASNStatus::OnHold;
        self.updated_at = Utc::now();
        self.version += 1;

        // Revalidate after update
        self.validate()
            .map_err(|_| ValidationError::new("ASN validation failed"))?;

        Ok(())
    }

    /// Release ASN from hold.
    pub fn release_from_hold(&mut self, new_status: ASNStatus) -> Result<(), ValidationError> {
        if self.status != ASNStatus::OnHold {
            return Err(ValidationError::new("ASN must be on hold to release"));
        }

        if new_status == ASNStatus::OnHold || new_status == ASNStatus::Completed {
            return Err(ValidationError::new(
                "Invalid status transition from OnHold",
            ));
        }

        self.status = new_status;
        self.updated_at = Utc::now();
        self.version += 1;

        // Revalidate after update
        self.validate()
            .map_err(|_| ValidationError::new("ASN validation failed"))?;

        Ok(())
    }

    /// Cancel the ASN.
    pub fn cancel(&mut self) -> Result<(), ValidationError> {
        if self.status == ASNStatus::Completed || self.status == ASNStatus::Delivered {
            return Err(ValidationError::new(
                "Cannot cancel completed or delivered ASN",
            ));
        }

        self.status = ASNStatus::Cancelled;
        self.updated_at = Utc::now();
        self.version += 1;

        // Revalidate after update
        self.validate()
            .map_err(|_| ValidationError::new("ASN validation failed"))?;

        Ok(())
    }
}
