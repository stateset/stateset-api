use chrono::{DateTime, Utc};
use async_trait::async_trait;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, Set};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
use validator::{Validate, ValidationError};
use uuid::Uuid;

/// Shipping carrier enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum ShippingCarrier {
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

impl fmt::Display for ShippingCarrier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShippingCarrier::UPS => write!(f, "UPS"),
            ShippingCarrier::FedEx => write!(f, "FedEx"),
            ShippingCarrier::USPS => write!(f, "USPS"),
            ShippingCarrier::DHL => write!(f, "DHL"),
            ShippingCarrier::Other => write!(f, "Other"),
        }
    }
}

/// Shipping method enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum ShippingMethod {
    #[sea_orm(string_value = "Standard")]
    Standard,

    #[sea_orm(string_value = "Express")]
    Express,

    #[sea_orm(string_value = "Overnight")]
    Overnight,

    #[sea_orm(string_value = "TwoDay")]
    TwoDay,

    #[sea_orm(string_value = "International")]
    International,

    #[sea_orm(string_value = "Custom")]
    Custom,
}

impl fmt::Display for ShippingMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShippingMethod::Standard => write!(f, "Standard"),
            ShippingMethod::Express => write!(f, "Express"),
            ShippingMethod::Overnight => write!(f, "Overnight"),
            ShippingMethod::TwoDay => write!(f, "Two-Day"),
            ShippingMethod::International => write!(f, "International"),
            ShippingMethod::Custom => write!(f, "Custom"),
        }
    }
}

/// Shipment status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum ShipmentStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,
    #[sea_orm(string_value = "InTransit")]
    InTransit,
    #[sea_orm(string_value = "Delivered")]
    Delivered,
    #[sea_orm(string_value = "Cancelled")]
    Cancelled,
    #[sea_orm(string_value = "OnHold")]
    OnHold,
}

impl fmt::Display for ShipmentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShipmentStatus::Processing => write!(f, "Processing"),
            ShipmentStatus::ReadyToShip => write!(f, "Ready to Ship"),
            ShipmentStatus::Shipped => write!(f, "Shipped"),
            ShipmentStatus::InTransit => write!(f, "In Transit"),
            ShipmentStatus::OutForDelivery => write!(f, "Out for Delivery"),
            ShipmentStatus::Delivered => write!(f, "Delivered"),
            ShipmentStatus::Failed => write!(f, "Failed"),
            ShipmentStatus::Returned => write!(f, "Returned"),
            ShipmentStatus::Cancelled => write!(f, "Cancelled"),
            ShipmentStatus::OnHold => write!(f, "On Hold"),
            ShipmentStatus::Pending => write!(f, "Pending"),
        }
    }
}

/// Custom error type for shipment operations
#[derive(Error, Debug)]
pub enum ShipmentError {
    #[error("Database error: {0}")]
    Database(#[from] DbErr),

    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

/// Shipment entity model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "shipments")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,

    pub order_id: Uuid,

    #[validate(length(
        min = 1,
        max = 100,
        message = "Tracking number must be between 1 and 100 characters"
    ))]
    pub tracking_number: String,

    pub carrier: ShippingCarrier,

    pub status: ShipmentStatus,

    #[validate(length(
        min = 1,
        max = 255,
        message = "Shipping address must be between 1 and 255 characters"
    ))]
    pub shipping_address: String,

    pub shipping_method: ShippingMethod,

    pub weight_kg: Option<f32>,

    pub dimensions_cm: Option<String>,

    #[validate(length(max = 500, message = "Notes cannot exceed 500 characters"))]
    pub notes: Option<String>,

    pub shipped_at: Option<DateTime<Utc>>,

    pub estimated_delivery: Option<DateTime<Utc>>,

    pub delivered_at: Option<DateTime<Utc>>,

    pub created_at: DateTime<Utc>,

    pub updated_at: DateTime<Utc>,

    pub created_by: Option<String>,

    pub recipient_name: String,

    #[validate(email(message = "Invalid email format"))]
    pub recipient_email: Option<String>,

    pub recipient_phone: Option<String>,

    pub tracking_url: Option<String>,

    pub shipping_cost: Option<f64>,

    pub insurance_amount: Option<f64>,

    pub is_signature_required: bool,
}

/// Database relations
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::order::Entity",
        from = "Column::OrderId",
        to = "super::order::Column::Id",
        on_delete = "Cascade"
    )]
    Order,

    #[sea_orm(has_many = "super::shipment_item::Entity")]
    ShipmentItems,

    #[sea_orm(has_many = "super::shipment_event::Entity")]
    ShipmentEvents,
}

impl Related<super::order::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Order.def()
    }
}

impl Related<super::shipment_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ShipmentItems.def()
    }
}

impl Related<super::shipment_event::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ShipmentEvents.def()
    }
}

/// Active model behavior for database hooks
#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    /// Hook that is triggered before insert/update
    async fn before_save<C: ConnectionTrait>(
        self,
        _db: &C,
        insert: bool,
    ) -> Result<Self, DbErr> {
        let mut active_model = self;
        if insert {
            active_model.id = Set(Uuid::new_v4());
        }
        Ok(active_model)
    }
}

impl Model {
    /// Create a new shipment
    pub fn new(
        order_id: Uuid,
        tracking_number: String,
        carrier: ShippingCarrier,
        shipping_address: String,
        shipping_method: ShippingMethod,
        recipient_name: String,
    ) -> Result<Self, ValidationError> {
        let now = Utc::now();
        let shipment = Self {
            id: Uuid::nil(), // Will be set by before_save
            order_id,
            tracking_number,
            carrier,
            status: ShipmentStatus::Processing,
            shipping_address,
            shipping_method,
            weight_kg: None,
            dimensions_cm: None,
            notes: None,
            shipped_at: None,
            estimated_delivery: None,
            delivered_at: None,
            created_at: now,
            updated_at: now,
            created_by: None,
            recipient_name,
            recipient_email: None,
            recipient_phone: None,
            tracking_url: None,
            shipping_cost: None,
            insurance_amount: None,
            is_signature_required: false,
        };

        shipment.validate().map_err(|_| ValidationError::new("Shipment validation failed"))?;
        Ok(shipment)
    }

    /// Generate tracking URL based on carrier and tracking number
    pub fn generate_tracking_url(&mut self) {
        let base_url = match self.carrier {
            ShippingCarrier::UPS => "https://www.ups.com/track?tracknum=",
            ShippingCarrier::FedEx => "https://www.fedex.com/apps/fedextrack/?tracknumbers=",
            ShippingCarrier::USPS => "https://tools.usps.com/go/TrackConfirmAction?tLabels=",
            ShippingCarrier::DHL => "https://www.dhl.com/us-en/home/tracking/tracking-express.html?submit=1&tracking-id=",
            ShippingCarrier::Other => return, // No URL for other carriers
        };

        self.tracking_url = Some(format!("{}{}", base_url, self.tracking_number));
    }

    /// Mark shipment as shipped
    pub fn mark_as_shipped(
        &mut self,
        estimated_delivery: Option<DateTime<Utc>>,
    ) -> Result<(), ShipmentError> {
        if self.status != ShipmentStatus::Processing && self.status != ShipmentStatus::ReadyToShip {
            return Err(ShipmentError::InvalidOperation(format!(
                "Cannot mark shipment as shipped from {} status",
                self.status
            )));
        }

        self.status = ShipmentStatus::Shipped;
        self.shipped_at = Some(Utc::now());
        self.estimated_delivery = estimated_delivery;
        self.updated_at = Utc::now();

        // Generate tracking URL if not already set
        if self.tracking_url.is_none() {
            self.generate_tracking_url();
        }

        Ok(())
    }

    /// Update shipment status
    pub fn update_status(&mut self, new_status: ShipmentStatus) -> Result<(), ShipmentError> {
        // Validate status transitions
        match (self.status, new_status) {
            // Valid transitions
            (ShipmentStatus::Processing, ShipmentStatus::ReadyToShip) => {}
            (ShipmentStatus::Processing, ShipmentStatus::Cancelled) => {}
            (ShipmentStatus::ReadyToShip, ShipmentStatus::Shipped) => {
                self.shipped_at = Some(Utc::now());
            }
            (ShipmentStatus::ReadyToShip, ShipmentStatus::Cancelled) => {}
            (ShipmentStatus::Shipped, ShipmentStatus::InTransit) => {}
            (ShipmentStatus::InTransit, ShipmentStatus::OutForDelivery) => {}
            (ShipmentStatus::InTransit, ShipmentStatus::Failed) => {}
            (ShipmentStatus::OutForDelivery, ShipmentStatus::Delivered) => {
                self.delivered_at = Some(Utc::now());
            }
            (ShipmentStatus::OutForDelivery, ShipmentStatus::Failed) => {}
            (ShipmentStatus::Failed, ShipmentStatus::InTransit) => {}
            (ShipmentStatus::Failed, ShipmentStatus::Returned) => {}
            (ShipmentStatus::Delivered, ShipmentStatus::Returned) => {}
            // Invalid transitions
            (current, new) => {
                return Err(ShipmentError::InvalidOperation(format!(
                    "Invalid status transition from {} to {}",
                    current, new
                )));
            }
        }

        self.status = new_status;
        self.updated_at = Utc::now();

        Ok(())
    }

    /// Add or update recipient information
    pub fn update_recipient(
        &mut self,
        name: String,
        email: Option<String>,
        phone: Option<String>,
    ) -> Result<(), ValidationError> {
        self.recipient_name = name;
        self.recipient_email = email;
        self.recipient_phone = phone;
        self.updated_at = Utc::now();

        self.validate().map_err(|_| ValidationError::new("Shipment validation failed"))?;
        Ok(())
    }

    /// Add shipping details like weight and dimensions
    pub fn add_shipping_details(
        &mut self,
        weight_kg: Option<f32>,
        dimensions_cm: Option<String>,
        shipping_cost: Option<f64>,
        insurance_amount: Option<f64>,
        signature_required: bool,
    ) -> Result<(), ValidationError> {
        self.weight_kg = weight_kg;
        self.dimensions_cm = dimensions_cm;
        self.shipping_cost = shipping_cost;
        self.insurance_amount = insurance_amount;
        self.is_signature_required = signature_required;
        self.updated_at = Utc::now();

        self.validate().map_err(|_| ValidationError::new("Shipment validation failed"))?;
        Ok(())
    }

    /// Add notes to the shipment
    pub fn add_notes(&mut self, notes: String) -> Result<(), ValidationError> {
        self.notes = Some(notes);
        self.updated_at = Utc::now();

        self.validate().map_err(|_| ValidationError::new("Shipment validation failed"))?;
        Ok(())
    }

    /// Save the shipment to database
    pub async fn save(&self, db: &DatabaseConnection) -> Result<Model, ShipmentError> {
        // Validate before saving
        self.validate().map_err(|_| ValidationError::new("Shipment validation failed"))?;

        let model: ActiveModel = self.clone().into();
        let result = match self.id {
            Uuid::nil() => model.insert(db).await?,
            _ => model.update(db).await?,
        };

        Ok(result)
    }

    /// Find a shipment by ID
    pub async fn find_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<Model>, DbErr> {
        Entity::find_by_id(id).one(db).await
    }

    /// Find shipments by order ID
    pub async fn find_by_order_id(
        db: &DatabaseConnection,
        order_id: Uuid,
    ) -> Result<Vec<Model>, DbErr> {
        Entity::find()
            .filter(Column::OrderId.eq(order_id))
            .all(db)
            .await
    }

    /// Find a shipment by tracking number
    pub async fn find_by_tracking(
        db: &DatabaseConnection,
        tracking_number: &str,
    ) -> Result<Option<Model>, DbErr> {
        Entity::find()
            .filter(Column::TrackingNumber.eq(tracking_number))
            .one(db)
            .await
    }

    /// Add a shipment event (for tracking history)
    pub async fn add_event(
        &self,
        db: &DatabaseConnection,
        event_type: &str,
        location: Option<String>,
        description: String,
    ) -> Result<(), ShipmentError> {
        // This assumes you have a ShipmentEvent entity defined elsewhere
        let event = super::shipment_event::ActiveModel {
            shipment_id: Set(self.id),
            event_type: Set(event_type.to_string()),
            location: Set(location),
            description: Set(Some(description)),
            timestamp: Set(Utc::now()),
            ..Default::default()
        };

        event.insert(db).await?;
        Ok(())
    }

    /// Calculate delivery time in days (if both shipped and delivered)
    pub fn delivery_time_days(&self) -> Option<f64> {
        match (self.shipped_at, self.delivered_at) {
            (Some(shipped), Some(delivered)) => {
                let duration = delivered.signed_duration_since(shipped);
                Some(duration.num_seconds() as f64 / 86400.0) // Convert seconds to days
            }
            _ => None,
        }
    }

    /// Check if delivery is late based on estimated delivery
    pub fn is_delivery_late(&self) -> bool {
        match (self.status, self.estimated_delivery) {
            (ShipmentStatus::Delivered, Some(estimated)) => {
                if let Some(delivered) = self.delivered_at {
                    delivered > estimated
                } else {
                    false
                }
            }
            (_, Some(estimated)) => {
                Utc::now() > estimated && self.status != ShipmentStatus::Delivered
            }
            _ => false,
        }
    }
}
