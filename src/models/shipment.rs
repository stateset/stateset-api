use diesel::prelude::*;
use serde::{Serialize, Deserialize};
use validator::{Validate, ValidationError};
use chrono::{NaiveDateTime, Utc};
use crate::schema::shipments;

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Insertable, AsChangeset, Validate)]
#[table_name = "shipments"]
pub struct Shipment {
    pub id: i32,
    #[validate(range(min = 1, message = "Order ID must be positive"))]
    pub order_id: i32,
    #[validate(length(min = 1, max = 100, message = "Tracking number must be between 1 and 100 characters"))]
    pub tracking_number: String,
    pub carrier: ShippingCarrier,
    pub status: ShipmentStatus,
    #[validate(length(min = 1, max = 255, message = "Shipping address must be between 1 and 255 characters"))]
    pub shipping_address: String,
    pub shipped_at: Option<NaiveDateTime>,
    pub estimated_delivery: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, Insertable, Validate)]
#[table_name = "shipments"]
pub struct NewShipment {
    #[validate(range(min = 1, message = "Order ID must be positive"))]
    pub order_id: i32,
    #[validate(length(min = 1, max = 100, message = "Tracking number must be between 1 and 100 characters"))]
    pub tracking_number: String,
    pub carrier: ShippingCarrier,
    #[validate(length(min = 1, max = 255, message = "Shipping address must be between 1 and 255 characters"))]
    pub shipping_address: String,
    pub estimated_delivery: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[sql_type = "diesel::sql_types::Text"]
pub enum ShippingCarrier {
    UPS,
    FedEx,
    USPS,
    DHL,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[sql_type = "diesel::sql_types::Text"]
pub enum ShipmentStatus {
    Processing,
    Shipped,
    InTransit,
    Delivered,
    Returned,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ShipmentSearchParams {
    #[validate(range(min = 1, message = "Order ID must be positive"))]
    pub order_id: Option<i32>,
    #[validate(length(min = 1, max = 100, message = "Tracking number must be between 1 and 100 characters"))]
    pub tracking_number: Option<String>,
    pub carrier: Option<ShippingCarrier>,
    pub status: Option<ShipmentStatus>,
    pub shipped_after: Option<NaiveDateTime>,
    pub shipped_before: Option<NaiveDateTime>,
    #[validate(range(min = 1, max = 1000, message = "Limit must be between 1 and 1000"))]
    pub limit: i64,
    #[validate(range(min = 0, message = "Offset must be non-negative"))]
    pub offset: i64,
}

impl Shipment {
    pub fn new(new_shipment: NewShipment) -> Result<Self, ValidationError> {
        let now = Utc::now().naive_utc();
        let shipment = Self {
            id: 0, // Assuming database will auto-increment this
            order_id: new_shipment.order_id,
            tracking_number: new_shipment.tracking_number,
            carrier: new_shipment.carrier,
            status: ShipmentStatus::Processing,
            shipping_address: new_shipment.shipping_address,
            shipped_at: None,
            estimated_delivery: new_shipment.estimated_delivery,
            created_at: now,
            updated_at: now,
        };
        shipment.validate()?;
        Ok(shipment)
    }

    pub fn update_status(&mut self, new_status: ShipmentStatus) -> Result<(), String> {
        if self.status == ShipmentStatus::Delivered || self.status == ShipmentStatus::Returned {
            return Err("Cannot update status of a delivered or returned shipment".into());
        }
        self.status = new_status;
        if new_status == ShipmentStatus::Shipped {
            self.shipped_at = Some(Utc::now().naive_utc());
        }
        self.updated_at = Utc::now().naive_utc();
        Ok(())
    }
}

impl ShippingCarrier {
    pub fn as_str(&self) -> &'static str {
        match self {
            ShippingCarrier::UPS => "UPS",
            ShippingCarrier::FedEx => "FedEx",
            ShippingCarrier::USPS => "USPS",
            ShippingCarrier::DHL => "DHL",
        }
    }
}

impl ShipmentStatus {
    pub fn is_final(&self) -> bool {
        matches!(self, ShipmentStatus::Delivered | ShipmentStatus::Returned)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ShipmentStatus::Processing => "Processing",
            ShipmentStatus::Shipped => "Shipped",
            ShipmentStatus::InTransit => "In Transit",
            ShipmentStatus::Delivered => "Delivered",
            ShipmentStatus::Returned => "Returned",
        }
    }
}

impl ShipmentSearchParams {
    pub fn new(limit: i64, offset: i64) -> Result<Self, ValidationError> {
        let params = Self {
            order_id: None,
            tracking_number: None,
            carrier: None,
            status: None,
            shipped_after: None,
            shipped_before: None,
            limit,
            offset,
        };
        params.validate()?;
        Ok(params)
    }
}