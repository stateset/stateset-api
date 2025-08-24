use crate::errors::ServiceError;
use axum::{
    extract::{Json, Path, Query, State},
    response::IntoResponse,
    routing::{delete, get, post, put},
    Router,
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use std::sync::Arc;

// Generic trait for shipments handler state
pub trait ShipmentsAppState: Clone + Send + Sync + 'static {}
impl<T> ShipmentsAppState for T where T: Clone + Send + Sync + 'static {}

#[derive(Debug, Serialize, Deserialize)]
pub struct Shipment {
    pub id: String,
    pub order_id: String,
    pub tracking_number: Option<String>,
    pub carrier: String,
    pub service_type: String,
    pub status: String,
    pub estimated_delivery: Option<DateTime<Utc>>,
    pub actual_delivery: Option<DateTime<Utc>>,
    pub shipping_address: Address,
    pub items: Vec<ShipmentItem>,
    pub weight: Option<f64>,
    pub dimensions: Option<Dimensions>,
    pub cost: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub shipped_at: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShipmentItem {
    pub id: String,
    pub order_item_id: String,
    pub product_id: String,
    pub quantity: i32,
    pub serial_numbers: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Address {
    pub street1: String,
    pub street2: Option<String>,
    pub city: String,
    pub state: String,
    pub postal_code: String,
    pub country: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Dimensions {
    pub length: f64,
    pub width: f64,
    pub height: f64,
    pub unit: String, // "in", "cm"
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrackingEvent {
    pub id: String,
    pub shipment_id: String,
    pub status: String,
    pub description: String,
    pub location: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub carrier_event_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateShipmentRequest {
    pub order_id: String,
    pub carrier: String,
    pub service_type: String,
    pub shipping_address: Address,
    pub items: Vec<CreateShipmentItemRequest>,
    pub weight: Option<f64>,
    pub dimensions: Option<Dimensions>,
}

#[derive(Debug, Deserialize)]
pub struct CreateShipmentItemRequest {
    pub order_item_id: String,
    pub quantity: i32,
    pub serial_numbers: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateShipmentRequest {
    pub status: Option<String>,
    pub tracking_number: Option<String>,
    pub estimated_delivery: Option<DateTime<Utc>>,
    pub carrier: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TrackingUpdateRequest {
    pub status: String,
    pub description: String,
    pub location: Option<String>,
    pub carrier_event_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ShipmentFilters {
    pub status: Option<String>,
    pub carrier: Option<String>,
    pub order_id: Option<String>,
    pub tracking_number: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Create the shipments router
pub fn shipments_router<S>() -> Router<S> 
where 
    S: ShipmentsAppState,
{
    Router::new()
        .route("/", get(list_shipments::<S>).post(create_shipment::<S>))
        .route("/{id}", get(get_shipment::<S>).put(update_shipment::<S>).delete(delete_shipment::<S>))
        .route("/{id}/ship", post(mark_shipped::<S>))
        .route("/{id}/deliver", post(mark_delivered::<S>))
        .route("/{id}/track", get(track_shipment::<S>))
        .route("/{id}/tracking", post(add_tracking_event::<S>))
        .route("/track/:tracking_number", get(track_by_number::<S>))
}

/// List shipments with optional filtering
pub async fn list_shipments<S>(
    State(_state): State<S>,
    Query(filters): Query<ShipmentFilters>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: ShipmentsAppState,
{
    // Mock data for now - replace with actual database queries
    let mut shipments = vec![
        Shipment {
            id: "ship_001".to_string(),
            order_id: "order_001".to_string(),
            tracking_number: Some("1Z123456789".to_string()),
            carrier: "UPS".to_string(),
            service_type: "Ground".to_string(),
            status: "in_transit".to_string(),
            estimated_delivery: Some(Utc::now() + chrono::Duration::days(2)),
            actual_delivery: None,
            shipping_address: Address {
                street1: "123 Main St".to_string(),
                street2: None,
                city: "Anytown".to_string(),
                state: "CA".to_string(),
                postal_code: "90210".to_string(),
                country: "US".to_string(),
            },
            items: vec![
                ShipmentItem {
                    id: "ship_item_001".to_string(),
                    order_item_id: "order_item_001".to_string(),
                    product_id: "prod_abc".to_string(),
                    quantity: 1,
                    serial_numbers: None,
                }
            ],
            weight: Some(2.5),
            dimensions: Some(Dimensions {
                length: 12.0,
                width: 8.0,
                height: 4.0,
                unit: "in".to_string(),
            }),
            cost: Some(15.99),
            created_at: Utc::now() - chrono::Duration::days(1),
            updated_at: Utc::now() - chrono::Duration::hours(2),
            shipped_at: Some(Utc::now() - chrono::Duration::hours(12)),
            delivered_at: None,
        },
        Shipment {
            id: "ship_002".to_string(),
            order_id: "order_002".to_string(),
            tracking_number: Some("1Z987654321".to_string()),
            carrier: "FedEx".to_string(),
            service_type: "Express".to_string(),
            status: "delivered".to_string(),
            estimated_delivery: Some(Utc::now() - chrono::Duration::hours(6)),
            actual_delivery: Some(Utc::now() - chrono::Duration::hours(3)),
            shipping_address: Address {
                street1: "456 Oak Ave".to_string(),
                street2: Some("Apt 2B".to_string()),
                city: "Another City".to_string(),
                state: "NY".to_string(),
                postal_code: "10001".to_string(),
                country: "US".to_string(),
            },
            items: vec![
                ShipmentItem {
                    id: "ship_item_002".to_string(),
                    order_item_id: "order_item_002".to_string(),
                    product_id: "prod_def".to_string(),
                    quantity: 2,
                    serial_numbers: Some(vec!["SN001".to_string(), "SN002".to_string()]),
                }
            ],
            weight: Some(5.0),
            dimensions: Some(Dimensions {
                length: 16.0,
                width: 12.0,
                height: 8.0,
                unit: "in".to_string(),
            }),
            cost: Some(29.99),
            created_at: Utc::now() - chrono::Duration::days(2),
            updated_at: Utc::now() - chrono::Duration::hours(3),
            shipped_at: Some(Utc::now() - chrono::Duration::days(1)),
            delivered_at: Some(Utc::now() - chrono::Duration::hours(3)),
        },
    ];

    // Apply filters
    if let Some(status) = &filters.status {
        shipments.retain(|s| &s.status == status);
    }
    if let Some(carrier) = &filters.carrier {
        shipments.retain(|s| &s.carrier == carrier);
    }
    if let Some(order_id) = &filters.order_id {
        shipments.retain(|s| &s.order_id == order_id);
    }
    if let Some(tracking_number) = &filters.tracking_number {
        shipments.retain(|s| s.tracking_number.as_ref() == Some(tracking_number));
    }

    let response = json!({
        "shipments": shipments,
        "total": shipments.len(),
        "limit": filters.limit.unwrap_or(50),
        "offset": filters.offset.unwrap_or(0)
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Create a new shipment
pub async fn create_shipment<S>(
    State(_state): State<S>,
    Json(payload): Json<CreateShipmentRequest>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: ShipmentsAppState,
{
    let shipment_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    
    let shipment = Shipment {
        id: shipment_id.clone(),
        order_id: payload.order_id,
        tracking_number: None, // Will be assigned by carrier
        carrier: payload.carrier,
        service_type: payload.service_type,
        status: "created".to_string(),
        estimated_delivery: None, // Will be calculated based on service
        actual_delivery: None,
        shipping_address: payload.shipping_address,
        items: payload.items.into_iter().enumerate().map(|(i, item)| ShipmentItem {
            id: format!("{}_item_{}", shipment_id, i),
            order_item_id: item.order_item_id,
            product_id: "prod_abc".to_string(), // Mock - get from order item
            quantity: item.quantity,
            serial_numbers: item.serial_numbers,
        }).collect(),
        weight: payload.weight,
        dimensions: payload.dimensions,
        cost: None, // Will be calculated
        created_at: now,
        updated_at: now,
        shipped_at: None,
        delivered_at: None,
    };

    Ok((StatusCode::CREATED, Json(shipment)))
}

/// Get a specific shipment by ID
pub async fn get_shipment<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: ShipmentsAppState,
{
    let shipment = Shipment {
        id: id.clone(),
        order_id: "order_001".to_string(),
        tracking_number: Some("1Z123456789".to_string()),
        carrier: "UPS".to_string(),
        service_type: "Ground".to_string(),
        status: "in_transit".to_string(),
        estimated_delivery: Some(Utc::now() + chrono::Duration::days(2)),
        actual_delivery: None,
        shipping_address: Address {
            street1: "123 Main St".to_string(),
            street2: None,
            city: "Anytown".to_string(),
            state: "CA".to_string(),
            postal_code: "90210".to_string(),
            country: "US".to_string(),
        },
        items: vec![
            ShipmentItem {
                id: format!("{}_item_1", id),
                order_item_id: "order_item_001".to_string(),
                product_id: "prod_abc".to_string(),
                quantity: 1,
                serial_numbers: None,
            }
        ],
        weight: Some(2.5),
        dimensions: Some(Dimensions {
            length: 12.0,
            width: 8.0,
            height: 4.0,
            unit: "in".to_string(),
        }),
        cost: Some(15.99),
        created_at: Utc::now() - chrono::Duration::days(1),
        updated_at: Utc::now() - chrono::Duration::hours(2),
        shipped_at: Some(Utc::now() - chrono::Duration::hours(12)),
        delivered_at: None,
    };

    Ok((StatusCode::OK, Json(shipment)))
}

/// Update a shipment
pub async fn update_shipment<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateShipmentRequest>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: ShipmentsAppState,
{
    let shipment = Shipment {
        id: id.clone(),
        order_id: "order_001".to_string(),
        tracking_number: payload.tracking_number.or(Some("1Z123456789".to_string())),
        carrier: payload.carrier.unwrap_or_else(|| "UPS".to_string()),
        service_type: "Ground".to_string(),
        status: payload.status.unwrap_or_else(|| "in_transit".to_string()),
        estimated_delivery: payload.estimated_delivery.or(Some(Utc::now() + chrono::Duration::days(2))),
        actual_delivery: None,
        shipping_address: Address {
            street1: "123 Main St".to_string(),
            street2: None,
            city: "Anytown".to_string(),
            state: "CA".to_string(),
            postal_code: "90210".to_string(),
            country: "US".to_string(),
        },
        items: vec![],
        weight: Some(2.5),
        dimensions: None,
        cost: Some(15.99),
        created_at: Utc::now() - chrono::Duration::days(1),
        updated_at: Utc::now(),
        shipped_at: Some(Utc::now() - chrono::Duration::hours(12)),
        delivered_at: None,
    };

    Ok((StatusCode::OK, Json(shipment)))
}

/// Delete a shipment
pub async fn delete_shipment<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: ShipmentsAppState,
{
    let response = json!({
        "message": format!("Shipment {} has been deleted", id),
        "deleted_id": id
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Mark shipment as shipped
async fn mark_shipped<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: ShipmentsAppState,
{
    let response = json!({
        "message": format!("Shipment {} has been marked as shipped", id),
        "shipment_id": id,
        "status": "shipped",
        "shipped_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Mark shipment as delivered
async fn mark_delivered<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: ShipmentsAppState,
{
    let response = json!({
        "message": format!("Shipment {} has been marked as delivered", id),
        "shipment_id": id,
        "status": "delivered",
        "delivered_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Track shipment by ID
pub async fn track_shipment<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: ShipmentsAppState,
{
    let tracking_events = vec![
        TrackingEvent {
            id: "track_001".to_string(),
            shipment_id: id.clone(),
            status: "label_created".to_string(),
            description: "Shipping label created".to_string(),
            location: Some("Origin Facility".to_string()),
            timestamp: Utc::now() - chrono::Duration::days(1),
            carrier_event_code: Some("MP".to_string()),
        },
        TrackingEvent {
            id: "track_002".to_string(),
            shipment_id: id.clone(),
            status: "picked_up".to_string(),
            description: "Package picked up by carrier".to_string(),
            location: Some("Origin Facility".to_string()),
            timestamp: Utc::now() - chrono::Duration::hours(18),
            carrier_event_code: Some("PU".to_string()),
        },
        TrackingEvent {
            id: "track_003".to_string(),
            shipment_id: id.clone(),
            status: "in_transit".to_string(),
            description: "Package in transit".to_string(),
            location: Some("Distribution Center".to_string()),
            timestamp: Utc::now() - chrono::Duration::hours(12),
            carrier_event_code: Some("IT".to_string()),
        },
    ];

    let response = json!({
        "shipment_id": id,
        "tracking_events": tracking_events,
        "current_status": "in_transit",
        "estimated_delivery": Utc::now() + chrono::Duration::days(1)
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Track shipment by tracking number
async fn track_by_number<S>(
    State(_state): State<S>,
    Path(tracking_number): Path<String>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: ShipmentsAppState,
{
    let tracking_events = vec![
        TrackingEvent {
            id: "track_001".to_string(),
            shipment_id: "ship_001".to_string(),
            status: "label_created".to_string(),
            description: "Shipping label created".to_string(),
            location: Some("Origin Facility".to_string()),
            timestamp: Utc::now() - chrono::Duration::days(1),
            carrier_event_code: Some("MP".to_string()),
        },
        TrackingEvent {
            id: "track_002".to_string(),
            shipment_id: "ship_001".to_string(),
            status: "in_transit".to_string(),
            description: "Package in transit".to_string(),
            location: Some("Distribution Center".to_string()),
            timestamp: Utc::now() - chrono::Duration::hours(12),
            carrier_event_code: Some("IT".to_string()),
        },
    ];

    let response = json!({
        "tracking_number": tracking_number,
        "shipment_id": "ship_001",
        "tracking_events": tracking_events,
        "current_status": "in_transit",
        "estimated_delivery": Utc::now() + chrono::Duration::days(1)
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Add tracking event to shipment
async fn add_tracking_event<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<TrackingUpdateRequest>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: ShipmentsAppState,
{
    let tracking_event = TrackingEvent {
        id: Uuid::new_v4().to_string(),
        shipment_id: id.clone(),
        status: payload.status,
        description: payload.description,
        location: payload.location,
        timestamp: Utc::now(),
        carrier_event_code: payload.carrier_event_code,
    };

    Ok((StatusCode::CREATED, Json(tracking_event)))
}

/// Update shipment status
pub async fn update_shipment_status<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: ShipmentsAppState,
{
    let new_status = payload.get("status").and_then(|s| s.as_str()).unwrap_or("unknown");
    
    let response = json!({
        "message": format!("Shipment {} status updated to {}", id, new_status),
        "shipment_id": id,
        "status": new_status,
        "updated_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}
