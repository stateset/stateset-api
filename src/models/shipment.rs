use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};
use chrono::Utc;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "shipments")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    
    #[validate(range(min = 1, message = "Order ID must be positive"))]
    pub order_id: i32,
    
    #[validate(length(min = 1, max = 100, message = "Tracking number must be between 1 and 100 characters"))]
    pub tracking_number: String,
    
    pub carrier: ShippingCarrier,
    
    pub status: ShipmentStatus,
    
    #[validate(length(min = 1, max = 255, message = "Shipping address must be between 1 and 255 characters"))]
    pub shipping_address: String,
    
    pub shipped_at: Option<DateTimeWithTimeZone>,
    
    pub estimated_delivery: Option<DateTimeWithTimeZone>,
    
    pub created_at: DateTimeWithTimeZone,
    
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}


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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum ShipmentStatus {
    #[sea_orm(string_value = "Processing")]
    Processing,
    
    #[sea_orm(string_value = "Shipped")]
    Shipped,
    
    #[sea_orm(string_value = "InTransit")]
    InTransit,
    
    #[sea_orm(string_value = "Delivered")]
    Delivered,
    
    #[sea_orm(string_value = "Returned")]
    Returned,
}