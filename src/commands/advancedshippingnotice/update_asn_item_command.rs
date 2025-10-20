use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        asn_entity::{ASNStatus, Entity as ASN},
        asn_items::{self, Entity as ASNItem},
    },
};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, DatabaseTransaction, EntityTrait, Set, TransactionError, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UpdateASNItemsCommand {
    pub asn_id: Uuid,
    pub version: i32,
    #[validate]
    pub items: Vec<ASNItemUpdate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ASNItemUpdate {
    pub id: Option<Uuid>, // None for new items
    pub purchase_order_item_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity_shipped: i32,
    pub package_number: Option<String>,
    pub lot_number: Option<String>,
    pub serial_numbers: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ASNItemResult {
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
impl Command for UpdateASNItemsCommand {
    type Result = Vec<ASNItemResult>;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()?;

        let db = db_pool.as_ref();

        let current_asn = ASN::find_by_id(self.asn_id)
            .one(db)
            .await
            .map_err(ServiceError::db_error)?
            .ok_or_else(|| ServiceError::NotFound(format!("ASN {} not found", self.asn_id)))?;

        if current_asn.status != ASNStatus::Draft && current_asn.status != ASNStatus::Submitted {
            return Err(ServiceError::InvalidStatus(format!(
                "ASN {} cannot be updated from status {}",
                self.asn_id, current_asn.status
            )));
        }

        let command_clone = self.clone();
        let updated_items = db
            .transaction::<_, Vec<asn_items::Model>, ServiceError>(move |txn| {
                Box::pin(async move { command_clone.apply_updates(txn).await })
            })
            .await
            .map_err(|err| match err {
                TransactionError::Connection(db_err) => ServiceError::db_error(db_err),
                TransactionError::Transaction(service_err) => service_err,
            })?;

        event_sender
            .send(Event::ASNItemsUpdated(
                self.asn_id,
                updated_items.iter().map(|item| item.id).collect(),
            ))
            .await
            .map_err(|e| ServiceError::EventError(e.to_string()))?;

        Ok(updated_items.into_iter().map(ASNItemResult::from).collect())
    }
}

impl UpdateASNItemsCommand {
    async fn apply_updates(
        &self,
        txn: &DatabaseTransaction,
    ) -> Result<Vec<asn_items::Model>, ServiceError> {
        let mut updated_items = Vec::with_capacity(self.items.len());

        for item in &self.items {
            let model = match item.id {
                Some(id) => self.update_existing_item(txn, id, item).await?,
                None => self.insert_new_item(txn, item).await?,
            };

            updated_items.push(model);
        }

        Ok(updated_items)
    }

    async fn update_existing_item(
        &self,
        txn: &DatabaseTransaction,
        id: Uuid,
        item: &ASNItemUpdate,
    ) -> Result<asn_items::Model, ServiceError> {
        let mut existing: asn_items::ActiveModel = ASNItem::find_by_id(id)
            .one(txn)
            .await
            .map_err(ServiceError::db_error)?
            .ok_or_else(|| ServiceError::NotFound(format!("ASN item {} not found", id)))?
            .into();

        existing.purchase_order_item_id = Set(item.purchase_order_item_id);
        existing.quantity_shipped = Set(item.quantity_shipped);
        existing.package_number = Set(item.package_number.clone());
        existing.lot_number = Set(item.lot_number.clone());
        existing.serial_numbers = Set(item.serial_numbers.clone());
        existing.updated_at = Set(Utc::now());

        existing.update(txn).await.map_err(ServiceError::db_error)
    }

    async fn insert_new_item(
        &self,
        txn: &DatabaseTransaction,
        item: &ASNItemUpdate,
    ) -> Result<asn_items::Model, ServiceError> {
        let now = Utc::now();

        let new_item = asn_items::ActiveModel {
            id: Set(Uuid::new_v4()),
            asn_id: Set(self.asn_id),
            purchase_order_item_id: Set(item.purchase_order_item_id),
            quantity_shipped: Set(item.quantity_shipped),
            package_number: Set(item.package_number.clone()),
            lot_number: Set(item.lot_number.clone()),
            serial_numbers: Set(item.serial_numbers.clone()),
            status: Set("PENDING".to_string()),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        new_item.insert(txn).await.map_err(ServiceError::db_error)
    }
}

impl From<asn_items::Model> for ASNItemResult {
    fn from(model: asn_items::Model) -> Self {
        Self {
            id: model.id,
            asn_id: model.asn_id,
            purchase_order_item_id: model.purchase_order_item_id,
            quantity_shipped: model.quantity_shipped,
            package_number: model.package_number,
            lot_number: model.lot_number,
            serial_numbers: model.serial_numbers,
            status: model.status,
        }
    }
}
