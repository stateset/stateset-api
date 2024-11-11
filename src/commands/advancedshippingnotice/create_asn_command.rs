use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        asn_entity::{self, Entity as ASN},
        asn_item_entity::{self, Entity as ASNItem},
        ASNStatus,
    },
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;
use prometheus::IntCounter;
use lazy_static::lazy_static;
use chrono::{DateTime, Utc};

lazy_static! {
    static ref ASN_CREATIONS: IntCounter = 
        IntCounter::new("asn_creations_total", "Total number of ASNs created")
            .expect("metric can be created");

    static ref ASN_CREATION_FAILURES: IntCounter = 
        IntCounter::new("asn_creation_failures_total", "Total number of failed ASN creations")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateASNCommand {
    pub purchase_order_id: Uuid,
    pub supplier_id: Uuid,
    pub expected_delivery_date: DateTime<Utc>,
    #[validate]
    pub shipping_address: ShippingAddress,
    #[validate]
    pub carrier_details: CarrierDetails,
    #[validate(length(min = 1, message = "At least one item is required"))]
    pub items: Vec<ASNItem>,
    #[validate]
    pub packages: Vec<Package>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ShippingAddress {
    #[validate(length(min = 1))]
    pub street: String,
    #[validate(length(min = 1))]
    pub city: String,
    #[validate(length(min = 1))]
    pub state: String,
    #[validate(length(min = 1))]
    pub postal_code: String,
    #[validate(length(min = 2))]
    pub country: String,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CarrierDetails {
    #[validate(length(min = 1))]
    pub carrier_name: String,
    pub tracking_number: Option<String>,
    pub service_level: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ASNItem {
    pub purchase_order_item_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
    pub package_number: Option<String>,
    pub lot_number: Option<String>,
    pub serial_numbers: Option<Vec<String>>,
    pub expiration_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct Package {
    #[validate(length(min = 1))]
    pub package_number: String,
    #[validate(range(min = 0.0))]
    pub weight: f64,
    pub weight_unit: WeightUnit,
    pub dimensions: Option<Dimensions>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Dimensions {
    pub length: f64,
    pub width: f64,
    pub height: f64,
    pub unit: DimensionUnit,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum WeightUnit {
    KG,
    LB,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DimensionUnit {
    CM,
    IN,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateASNResult {
    pub id: Uuid,
    pub purchase_order_id: Uuid,
    pub supplier_id: Uuid,
    pub status: String,
    pub expected_delivery_date: DateTime<Utc>,
    pub shipping_address: ShippingAddress,
    pub carrier_details: CarrierDetails,
    pub created_at: DateTime<Utc>,
    pub items: Vec<ASNItem>,
    pub packages: Vec<Package>,
}

#[async_trait::async_trait]
impl Command for CreateASNCommand {
    type Result = CreateASNResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            ASN_CREATION_FAILURES.inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        // Validate purchase order exists and is in valid state
        self.validate_purchase_order(db).await?;

        let saved_asn = self.create_asn(db).await?;

        self.log_and_trigger_event(&event_sender, &saved_asn).await?;

        ASN_CREATIONS.inc();

        Ok(CreateASNResult {
            id: saved_asn.id,
            purchase_order_id: saved_asn.purchase_order_id,
            supplier_id: saved_asn.supplier_id,
            status: saved_asn.status,
            expected_delivery_date: saved_asn.expected_delivery_date.and_utc(),
            shipping_address: self.shipping_address.clone(),
            carrier_details: self.carrier_details.clone(),
            created_at: saved_asn.created_at.and_utc(),
            items: self.items.clone(),
            packages: self.packages.clone(),
        })
    }
}

impl CreateASNCommand {
    async fn validate_purchase_order(
        &self,
        db: &DatabaseConnection,
    ) -> Result<(), ServiceError> {
        // Implementation to validate purchase order exists and is in valid state
        // This would query the purchase_orders table
        Ok(()) // Simplified for example
    }

    async fn create_asn(
        &self,
        db: &DatabaseConnection,
    ) -> Result<asn_entity::Model, ServiceError> {
        db.transaction::<_, asn_entity::Model, ServiceError>(|txn| {
            Box::pin(async move {
                let new_asn = asn_entity::ActiveModel {
                    purchase_order_id: Set(self.purchase_order_id),
                    supplier_id: Set(self.supplier_id),
                    status: Set(ASNStatus::Draft.to_string()),
                    expected_delivery_date: Set(self.expected_delivery_date.naive_utc()),
                    shipping_address: Set(serde_json::to_value(&self.shipping_address).unwrap()),
                    carrier_details: Set(serde_json::to_value(&self.carrier_details).unwrap()),
                    created_at: Set(Utc::now().naive_utc()),
                    updated_at: Set(Utc::now().naive_utc()),
                    ..Default::default()
                };

                let saved_asn = new_asn.insert(txn).await.map_err(|e| {
                    let msg = format!("Failed to save ASN: {}", e);
                    error!("{}", msg);
                    ServiceError::DatabaseError(msg)
                })?;

                // Save packages
                for package in &self.packages {
                    let new_package = asn_package_entity::ActiveModel {
                        asn_id: Set(saved_asn.id),
                        package_number: Set(package.package_number.clone()),
                        weight: Set(package.weight),
                        weight_unit: Set(package.weight_unit.to_string()),
                        dimensions: Set(package.dimensions.as_ref().map(|d| serde_json::to_value(d).unwrap())),
                        ..Default::default()
                    };
                    new_package.insert(txn).await.map_err(|e| {
                        let msg = format!("Failed to save ASN package: {}", e);
                        error!("{}", msg);
                        ServiceError::DatabaseError(msg)
                    })?;
                }

                // Save items
                for item in &self.items {
                    let new_item = asn_item_entity::ActiveModel {
                        asn_id: Set(saved_asn.id),
                        purchase_order_item_id: Set(item.purchase_order_item_id),
                        quantity: Set(item.quantity),
                        package_number: Set(item.package_number.clone()),
                        lot_number: Set(item.lot_number.clone()),
                        serial_numbers: Set(item.serial_numbers.clone()),
                        expiration_date: Set(item.expiration_date.map(|d| d.naive_utc())),
                        status: Set("PENDING".to_string()),
                        ..Default::default()
                    };
                    new_item.insert(txn).await.map_err(|e| {
                        let msg = format!("Failed to save ASN item: {}", e);
                        error!("{}", msg);
                        ServiceError::DatabaseError(msg)
                    })?;
                }

                Ok(saved_asn)
            })
        }).await
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        saved_asn: &asn_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(
            asn_id = %saved_asn.id,
            purchase_order_id = %self.purchase_order_id,
            supplier_id = %self.supplier_id,
            items_count = %self.items.len(),
            packages_count = %self.packages.len(),
            "ASN created successfully"
        );

        event_sender
            .send(Event::ASNCreated(saved_asn.id))
            .await
            .map_err(|e| {
                ASN_CREATION_FAILURES.inc();
                let msg = format!("Failed to send event for created ASN: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}