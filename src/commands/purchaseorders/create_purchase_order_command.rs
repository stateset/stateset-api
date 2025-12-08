use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        purchase_order_entity::{self, PurchaseOrderStatus},
        purchase_order_item_entity::{self},
    },
};
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use prometheus::IntCounter;
use rust_decimal::Decimal;
use sea_orm::{Set, *};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref PO_CREATIONS: IntCounter = IntCounter::new(
        "purchase_order_creations_total",
        "Total number of purchase orders created"
    )
    .expect("metric can be created");
    static ref PO_CREATION_FAILURES: IntCounter = IntCounter::new(
        "purchase_order_creation_failures_total",
        "Total number of failed purchase order creations"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreatePurchaseOrderCommand {
    pub supplier_id: Uuid,
    #[validate(length(min = 1, message = "At least one item is required"))]
    pub items: Vec<PurchaseOrderItemRequest>,
    pub expected_delivery_date: DateTime<Utc>,
    #[validate]
    pub shipping_address: ShippingAddress,
    pub payment_terms: Option<String>,
    pub currency: String,
    #[validate(length(max = 1000))]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate, Clone)]
pub struct PurchaseOrderItemRequest {
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
    pub items: Vec<PurchaseOrderItemRequest>,
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
            status: saved_po.status.to_string(),
            po_number: saved_po.po_number,
            created_at: saved_po.created_at,
            expected_delivery_date: saved_po
                .expected_delivery_date
                .map(|d| {
                    DateTime::<Utc>::from_naive_utc_and_offset(d.and_hms_opt(0, 0, 0).unwrap(), Utc)
                })
                .unwrap_or_else(|| Utc::now()),
            total_amount: saved_po
                .total_amount
                .to_string()
                .parse::<f64>()
                .unwrap_or(0.0),
            currency: self.currency.clone(),
            items: self.items.clone(),
        })
    }
}

impl CreatePurchaseOrderCommand {
    async fn validate_supplier(&self, db: &DatabaseConnection) -> Result<(), ServiceError> {
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
        let po_number = self.generate_po_number().await;
        let total_amount = self.calculate_total_amount();
        let supplier_id = self.supplier_id;
        let expected_delivery_date = self.expected_delivery_date.date_naive();
        let currency = self.currency.clone();
        let items = self.items.clone();

        db.transaction::<_, purchase_order_entity::Model, ServiceError>(move |txn| {
            let items = items.clone();
            Box::pin(async move {
                let new_po = purchase_order_entity::ActiveModel {
                    po_number: Set(po_number.clone()),
                    supplier_id: Set(supplier_id),
                    status: Set(PurchaseOrderStatus::Draft),
                    order_date: Set(Utc::now()),
                    expected_delivery_date: Set(Some(expected_delivery_date)),
                    total_amount: Set(Decimal::from_f64_retain(total_amount).unwrap_or_default()),
                    currency: Set(currency.clone()),
                    created_at: Set(Utc::now()),
                    created_by: Set(Uuid::new_v4()), // System-generated; user context passed via command if needed
                    ..Default::default()
                };

                let saved_po = new_po.insert(txn).await.map_err(|e| {
                    let msg = format!(
                        "Failed to create purchase order {} for supplier {}: {}",
                        po_number, supplier_id, e
                    );
                    error!("{}", msg);
                    ServiceError::db_error(e)
                })?;

                for item in &items {
                    let item_total =
                        Decimal::from_f64_retain(item.quantity as f64 * item.unit_price)
                            .unwrap_or_default();
                    let tax_amount = item_total
                        * Decimal::from_f64_retain(item.tax_rate.unwrap_or(0.0))
                            .unwrap_or_default();

                    let new_item = purchase_order_item_entity::ActiveModel {
                        purchase_order_id: Set(saved_po.id),
                        sku: Set(item.product_id.to_string()), // Using product_id as SKU
                        product_name: Set(item.description.clone().unwrap_or_else(|| format!("Product {}", item.product_id))),
                        quantity_ordered: Set(item.quantity),
                        quantity_received: Set(0),
                        unit_cost: Set(
                            Decimal::from_f64_retain(item.unit_price).unwrap_or_default()
                        ),
                        total_cost: Set(item_total + tax_amount),
                        created_at: Set(Utc::now()),
                        ..Default::default()
                    };
                    new_item.insert(txn).await.map_err(|e| {
                        let msg = format!(
                            "Failed to create purchase order item for PO {} (product {}): {}",
                            po_number, item.product_id, e
                        );
                        error!("{}", msg);
                        ServiceError::db_error(e)
                    })?;
                }

                Ok(saved_po)
            })
        })
        .await
        .map_err(|e| match e {
            TransactionError::Connection(db_err) => ServiceError::db_error(db_err),
            TransactionError::Transaction(service_err) => service_err,
        })
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
            .send(Event::PurchaseOrderCreated(saved_po.id))
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
            PurchaseOrderStatus::Rejected => "REJECTED",
            PurchaseOrderStatus::Ordered => "ORDERED",
            PurchaseOrderStatus::PartiallyReceived => "PARTIALLY_RECEIVED",
        }
        .to_string()
    }
}
