use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        asn_entity::{self, Entity as ASN},
        asn_item_entity::{self, Entity as CreateASNItem},
        asn_package_entity,
        ASNStatus,
    },
};
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use prometheus::IntCounter;
use sea_orm::{*, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref ASN_CREATIONS: IntCounter =
        IntCounter::new("asn_creations_total", "Total number of ASNs created")
            .expect("metric can be created");
    static ref ASN_CREATION_FAILURES: IntCounter = IntCounter::new(
        "asn_creation_failures_total",
        "Total number of failed ASN creations"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateASNCommand {
    pub purchase_order_id: Uuid,
    pub supplier_id: Uuid,
    pub supplier_name: String,
    pub expected_delivery_date: Option<DateTime<Utc>>,
    #[validate]
    pub shipping_address: ShippingAddress,
    #[validate]
    pub carrier_details: CarrierDetails,
    #[validate(length(min = 1, message = "At least one item is required"))]
    pub items: Vec<CreateASNItemRequest>,
    #[validate]
    pub packages: Vec<Package>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CarrierDetails {
    #[validate(length(min = 1))]
    pub carrier_name: String,
    pub tracking_number: Option<String>,
    pub service_level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateASNItemRequest {
    pub product_id: Uuid,
    pub product_name: String,
    pub product_sku: String,
    #[validate(range(min = 1))]
    pub quantity: i32,
    pub unit_price: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Package {
    #[validate(length(min = 1))]
    pub package_number: String,
    #[validate(range(min = 0.0))]
    pub weight: f64,
    pub weight_unit: WeightUnit,
    pub dimensions: Option<Dimensions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dimensions {
    pub length: f64,
    pub width: f64,
    pub height: f64,
    pub unit: DimensionUnit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WeightUnit {
    KG,
    LB,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DimensionUnit {
    CM,
    IN,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateASNResult {
    pub id: Uuid,
    pub asn_number: String,
    pub supplier_id: Uuid,
    pub supplier_name: String,
    pub status: String,
    pub expected_delivery_date: Option<DateTime<Utc>>,
    pub shipping_address: String,
    pub created_at: DateTime<Utc>,
    pub items: Vec<CreateASNItemRequest>,
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

        self.log_and_trigger_event(&event_sender, &saved_asn)
            .await?;

        ASN_CREATIONS.inc();

        Ok(CreateASNResult {
            id: saved_asn.id,
            asn_number: saved_asn.asn_number,
            supplier_id: saved_asn.supplier_id,
            supplier_name: saved_asn.supplier_name,
            status: saved_asn.status.to_string(),
            expected_delivery_date: saved_asn.expected_delivery_date,
            shipping_address: saved_asn.shipping_address,
            created_at: saved_asn.created_at,
            items: self.items.clone(),
            packages: self.packages.clone(),
        })
    }
}

impl CreateASNCommand {
    async fn validate_purchase_order(&self, _db: &DatabaseConnection) -> Result<(), ServiceError> {
        // Implementation to validate purchase order exists and is in valid state
        // This would query the purchase_orders table
        Ok(()) // Simplified for example
    }

    async fn create_asn(&self, db: &DatabaseConnection) -> Result<asn_entity::Model, ServiceError> {
        let supplier_id = self.supplier_id;
        let supplier_name = self.supplier_name.clone();
        let expected_delivery_date = self.expected_delivery_date;
        let shipping_address = format!("{}, {}, {} {}", self.shipping_address.street, self.shipping_address.city, self.shipping_address.state, self.shipping_address.postal_code);
        let items = self.items.clone();
        
        let result = db.transaction::<_, asn_entity::Model, DbErr>(|txn| {
            Box::pin(async move {
                let new_asn = asn_entity::ActiveModel {
                    asn_number: Set(format!("ASN-{}", Uuid::new_v4())),
                    supplier_id: Set(supplier_id),
                    supplier_name: Set(supplier_name),
                    status: Set(ASNStatus::Draft),
                    expected_delivery_date: Set(expected_delivery_date),
                    shipping_address: Set(shipping_address),
                    created_at: Set(Utc::now()),
                    updated_at: Set(Utc::now()),
                    version: Set(1),
                    ..Default::default()
                };

                let saved_asn = new_asn.insert(txn).await?;

                // Skip packages for now - not implemented in model

                // Save items
                for item in &items {
                    let new_item = asn_item_entity::ActiveModel {
                        asn_id: Set(saved_asn.id),
                        product_id: Set(item.product_id),
                        product_name: Set(item.product_name.clone()),
                        product_sku: Set(item.product_sku.clone()),
                        quantity_expected: Set(item.quantity),
                        quantity_received: Set(0), // Initially 0
                        unit_price: Set(item.unit_price),
                        ..Default::default()
                    };
                    new_item.insert(txn).await?;
                }

                Ok(saved_asn)
            })
        })
        .await;
        
        result.map_err(|e| match e {
            sea_orm::TransactionError::Connection(db_err) => ServiceError::DatabaseError(db_err),
            sea_orm::TransactionError::Transaction(db_err) => ServiceError::DatabaseError(db_err),
        })
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
