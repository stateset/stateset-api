use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::asn_items::{self, Entity as ASNItem},
};
use lazy_static::lazy_static;
use prometheus::IntCounter;
use sea_orm::{*, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref ASN_ITEMS_ADDED: IntCounter = IntCounter::new(
        "asn_items_added_total",
        "Total number of items added to ASNs"
    )
    .expect("metric can be created");
    static ref ASN_ITEM_ADD_FAILURES: IntCounter = IntCounter::new(
        "asn_item_add_failures_total",
        "Total number of failed item additions to ASNs"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AddItemToASNCommand {
    pub asn_id: Uuid,
    pub purchase_order_item_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity_shipped: i32,
    pub package_number: Option<String>,
    pub lot_number: Option<String>,
    #[validate]
    pub serial_numbers: Option<Vec<String>>,
    pub expiration_date: Option<String>,
    pub customs_value: Option<f64>,
    pub country_of_origin: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddItemToASNResult {
    pub id: Uuid,
    pub asn_id: Uuid,
    pub purchase_order_item_id: Uuid,
    pub quantity_shipped: i32,
    pub package_number: Option<String>,
    pub lot_number: Option<String>,
    pub serial_numbers: Option<Vec<String>>,
    pub status: String,
}

#[async_trait::async_trait]
impl Command for AddItemToASNCommand {
    type Result = AddItemToASNResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            ASN_ITEM_ADD_FAILURES.inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        // Validate purchase order item exists and has sufficient quantity
        self.validate_purchase_order_item(db).await?;

        let saved_item = self.add_item_to_asn(db).await?;

        self.log_and_trigger_event(&event_sender, &saved_item)
            .await?;

        ASN_ITEMS_ADDED.inc();

        Ok(AddItemToASNResult {
            id: saved_item.id,
            asn_id: saved_item.asn_id,
            purchase_order_item_id: saved_item.purchase_order_item_id,
            quantity_shipped: saved_item.quantity_shipped,
            package_number: saved_item.package_number,
            lot_number: saved_item.lot_number,
            serial_numbers: saved_item.serial_numbers,
            status: saved_item.status,
        })
    }
}

impl AddItemToASNCommand {
    async fn validate_purchase_order_item(
        &self,
        db: &DatabaseConnection,
    ) -> Result<(), ServiceError> {
        // Implementation to validate purchase order item exists and has sufficient quantity
        // This would query the purchase_order_items table and verify quantities
        Ok(()) // Simplified for example
    }

    async fn add_item_to_asn(
        &self,
        db: &DatabaseConnection,
    ) -> Result<asn_items::Model, ServiceError> {
        let new_item = asn_items::ActiveModel {
            id: Set(Uuid::new_v4()),
            asn_id: Set(self.asn_id),
            purchase_order_item_id: Set(self.purchase_order_item_id),
            quantity_shipped: Set(self.quantity_shipped),
            package_number: Set(self.package_number.clone()),
            lot_number: Set(self.lot_number.clone()),
            serial_numbers: Set(self.serial_numbers.clone()),
            expiration_date: Set(self.expiration_date.clone()),
            customs_value: Set(self.customs_value),
            country_of_origin: Set(self.country_of_origin.clone()),
            status: Set("PENDING".to_string()),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..Default::default()
        };

        new_item.insert(db).await.map_err(|e| {
            ASN_ITEM_ADD_FAILURES.inc();
            let msg = format!("Failed to add item to ASN {}: {}", self.asn_id, e);
            error!("{}", msg);
            ServiceError::DatabaseError(e)
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        saved_item: &asn_items::Model,
    ) -> Result<(), ServiceError> {
        info!(
            asn_id = %self.asn_id,
            purchase_order_item_id = %self.purchase_order_item_id,
            quantity_shipped = %self.quantity_shipped,
            package_number = ?self.package_number,
            "Item added to ASN successfully"
        );

        event_sender
            .send(Event::ASNItemAdded(self.asn_id))
            .await
            .map_err(|e| {
                ASN_ITEM_ADD_FAILURES.inc();
                let msg = format!("Failed to send event for added ASN item: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}
