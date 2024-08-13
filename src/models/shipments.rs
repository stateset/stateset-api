use diesel::prelude::*;
use serde::{Serialize, Deserialize};
use validator::Validate;
use crate::schema::shipments;

#[derive(Debug, Serialize, Deserialize, Queryable, Insertable, AsChangeset, Validate)]
#[table_name = "shipments"]
pub struct Shipment {
    pub id: i32,
    pub order_id: i32,
    #[validate(length(min = 1, max = 100))]
    pub tracking_number: String,
    pub carrier: ShippingCarrier,
    pub status: ShipmentStatus,
    #[validate(length(min = 0, max = 255))]
    pub shipping_address: String,
    pub shipped_at: Option<chrono::NaiveDateTime>,
    pub estimated_delivery: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, Insertable, Validate)]
#[table_name = "shipments"]
pub struct NewShipment {
    pub order_id: i32,
    #[validate(length(min = 1, max = 100))]
    pub tracking_number: String,
    pub carrier: ShippingCarrier,
    #[validate(length(min = 0, max = 255))]
    pub shipping_address: String,
    pub estimated_delivery: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[sql_type = "diesel::sql_types::Text"]
pub enum ShippingCarrier {
    UPS,
    FedEx,
    USPS,
    DHL,
}

#[derive(Debug, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[sql_type = "diesel::sql_types::Text"]
pub enum ShipmentStatus {
    Processing,
    Shipped,
    InTransit,
    Delivered,
    Returned,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShipmentSearchParams {
    pub order_id: Option<i32>,
    pub tracking_number: Option<String>,
    pub carrier: Option<ShippingCarrier>,
    pub status: Option<ShipmentStatus>,
    pub shipped_after: Option<chrono::NaiveDateTime>,
    pub shipped_before: Option<chrono::NaiveDateTime>,
    pub limit: i64,
    pub offset: i64,
}