use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        purchase_order_entity::{self, Entity as PurchaseOrder},
        purchase_order_item_entity::{self, Entity as PurchaseOrderItem},
        PurchaseOrderStatus,
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
    static ref PO_CREATIONS: IntCounter = 
        IntCounter::new("purchase_order_creations_total", "Total number of purchase orders created")
            .expect("metric can be created");

    static ref PO_CREATION_FAILURES: IntCounter = 
        IntCounter::new("purchase_order_creation_failures_total", "Total number of failed purchase order creations")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreatePurchaseOrderCommand {
    pub supplier_id: Uuid,
    #[validate(length(min = 1, message = "At least one item is required"))]
    pub items: Vec<PurchaseOrderItem>,
    pub expected_delivery_date: DateTime<Utc>,
    #[validate]
    pub shipping_address: ShippingAddress,
    pub payment_terms: Option<String>,
    pub currency: String,
    #[validate(length(max = 1000))]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct PurchaseOrderItem {
    pub product_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
    #[validate(range(min = 0.0))]
    pub unit_price: f64,
    pub tax_rate: Option<f64>,
    pub currency: Option<String>,
    #[validate(length(max = 500))]
    pub description: Option<String>,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePurchaseOrderResult {
    pub id: Uuid,
    pub supplier_id: Uuid,
    pub status: String,
    pub po_number: String,
    pub created_at: DateTime<Utc>,
    pub expected_delivery_date: DateTime<Utc>,
    pub total_amount: f64,
    pub currency: String,
    pub items: Vec<PurchaseOrderItem>,
}

#[async_trait::async_trait]
impl Command for CreatePurchaseOrderCommand {
    type Result = CreatePurchaseOrderResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            PO_CREATION_FAILURES.inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        // Validate supplier exists and is active
        self.validate_supplier(db_pool.as_ref()).await?;

        let db = db_pool.as_ref();

        let saved_po = self.create_purchase_order(db).await?;

        self.log_and_trigger_event(&event_sender, &saved_po).await?;

        PO_CREATIONS.inc();

        Ok(CreatePurchaseOrderResult {
            id: saved_po.id,
            supplier_id: saved_po.supplier_id,
            status: saved_po.status,
            po_number: saved_po.po_number,
            created_at: saved_po.created_at.and_utc(),
            expected_delivery_date: saved_po.expected_delivery_date.and_utc(),
            total_amount: saved_po.total_amount,
            currency: self.currency.clone(),
            items: self.items.clone(),
        })
    }
}

impl CreatePurchaseOrderCommand {
    async fn validate_supplier(
        &self,
        db: &DatabaseConnection,
    ) -> Result<(), ServiceError> {
        // Implementation to validate supplier exists and is active
        Ok(()) // Simplified for example
    }

    async fn generate_po_number(&self) -> String {
        // Implementation to generate unique PO number
        format!("PO-{}", Uuid::new_v4().simple())
    }

    fn calculate_total_amount(&self) -> f64 {
        self.items.iter().fold(0.0, |acc, item| {
            let item_total = item.unit_price * item.quantity as f64;
            let tax_amount = item.tax_rate.unwrap_or(0.0) * item_total;
            acc + item_total + tax_amount
        })
    }

    async fn create_purchase_order(
        &self,
        db: &DatabaseConnection,
    ) -> Result<purchase_order_entity::Model, ServiceError> {
        db.transaction::<_, purchase_order_entity::Model, ServiceError>(|txn| {
            Box::pin(async move {
                let po_number = self.generate_po_number().await;
                let total_amount = self.calculate_total_amount();

                let new_po = purchase_order_entity::ActiveModel {
                    supplier_id: Set(self.supplier_id),
                    status: Set(PurchaseOrderStatus::Draft.to_string()),
                    po_number: Set(po_number),
                    expected_delivery_date: Set(self.expected_delivery_date.naive_utc()),
                    shipping_address: Set(serde_json::to_value(&self.shipping_address).unwrap()),
                    payment_terms: Set(self.payment_terms.clone()),
                    currency: Set(self.currency.clone()),
                    total_amount: Set(total_amount),
                    notes: Set(self.notes.clone()),
                    created_at: Set(Utc::now().naive_utc()),
                    created_by: Set(None), // Could add user context if available
                    version: Set(1),
                    ..Default::default()
                };

                let saved_po = new_po.insert(txn).await.map_err(|e| {
                    let msg = format!("Failed to save purchase order: {}", e);
                    error!("{}", msg);
                    ServiceError::DatabaseError(msg)
                })?;

                for item in &self.items {
                    let item_total = item.unit_price * item.quantity as f64;
                    let tax_amount = item.tax_rate.unwrap_or(0.0) * item_total;

                    let new_item = purchase_order_item_entity::ActiveModel {
                        purchase_order_id: Set(saved_po.id),
                        product_id: Set(item.product_id),
                        quantity: Set(item.quantity),
                        unit_price: Set(item.unit_price),
                        currency: Set(item.currency.clone().unwrap_or(self.currency.clone())),
                        tax_rate: Set(item.tax_rate),
                        total_amount: Set(item_total + tax_amount),
                        description: Set(item.description.clone()),
                        status: Set(PurchaseOrderStatus::Draft.to_string()),
                        created_at: Set(Utc::now().naive_utc()),
                        ..Default::default()
                    };

                    new_item.insert(txn).await.map_err(|e| {
                        let msg = format!("Failed to save purchase order item: {}", e);
                        error!("{}", msg);
                        ServiceError::DatabaseError(msg)
                    })?;
                }

                Ok(saved_po)
            })
        }).await
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        saved_po: &purchase_order_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(
            purchase_order_id = %saved_po.id,
            supplier_id = %self.supplier_id,
            items_count = %self.items.len(),
            total_amount = %saved_po.total_amount,
            currency = %self.currency,
            "Purchase order created successfully"
        );

        event_sender
            .send(Event::PurchaseOrderCreated(
                saved_po.id,
                saved_po.po_number.clone(),
                saved_po.total_amount,
                self.currency.clone()
            ))
            .await
            .map_err(|e| {
                PO_CREATION_FAILURES.inc();
                let msg = format!("Failed to send event for created purchase order: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}

// Additional helper to convert enum to string
impl ToString for PurchaseOrderStatus {
    fn to_string(&self) -> String {
        match self {
            PurchaseOrderStatus::Draft => "DRAFT",
            PurchaseOrderStatus::Submitted => "SUBMITTED",
            PurchaseOrderStatus::Approved => "APPROVED",
            PurchaseOrderStatus::Received => "RECEIVED",
            PurchaseOrderStatus::Cancelled => "CANCELLED",
        }.to_string()
    }
}