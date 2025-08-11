use crate::{
    commands::Command,
    db::DbPool,
    errors::ASNError,
    events::{Event, EventSender},
    models::{
        asn_entity::{self, Entity as ASN},
        asn_item_entity::{self, Entity as ASNItem},
        asn_note_entity, ASNStatus,
    },
};
use chrono::Utc;
use lazy_static::lazy_static;
use prometheus::{IntCounter, IntCounterVec};
use sea_orm::{*, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateASNItemsCommand {
    pub asn_id: Uuid,
    pub version: i32,
    #[validate]
    pub items: Vec<ASNItemUpdate>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
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

    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ASNError> {
        let db = db_pool.as_ref();

        // Validate ASN can be modified
        let current_asn = ASN::find_by_id(self.asn_id)
            .one(db)
            .await?
            .ok_or(ASNError::NotFound(self.asn_id.to_string()))?;

        if !vec![
            ASNStatus::Draft.to_string(),
            ASNStatus::Submitted.to_string(),
        ]
        .contains(&current_asn.status)
        {
            return Err(ASNError::InvalidStatus(self.asn_id));
        }

        let updated_items = db
            .transaction::<_, Vec<asn_item_entity::Model>, ASNError>(|txn| {
                Box::pin(async move {
                    let mut updated_items = Vec::new();

                    for item in &self.items {
                        let item_model = match item.id {
                            Some(id) => {
                                // Update existing item
                                let mut existing: asn_item_entity::ActiveModel =
                                    ASNItem::find_by_id(id)
                                        .one(txn)
                                        .await?
                                        .ok_or(ASNError::ItemNotFound(id))?
                                        .into();

                                existing.quantity_shipped = Set(item.quantity_shipped);
                                existing.package_number = Set(item.package_number.clone());
                                existing.lot_number = Set(item.lot_number.clone());
                                existing.serial_numbers = Set(item.serial_numbers.clone());

                                existing.update(txn).await?
                            }
                            None => {
                                // Create new item
                                let new_item = asn_item_entity::ActiveModel {
                                    id: Set(Uuid::new_v4()),
                                    asn_id: Set(self.asn_id),
                                    purchase_order_item_id: Set(item.purchase_order_item_id),
                                    quantity_shipped: Set(item.quantity_shipped),
                                    package_number: Set(item.package_number.clone()),
                                    lot_number: Set(item.lot_number.clone()),
                                    serial_numbers: Set(item.serial_numbers.clone()),
                                    ..Default::default()
                                };

                                new_item.insert(txn).await?
                            }
                        };

                        updated_items.push(item_model);
                    }

                    Ok(updated_items)
                })
            })
            .await?;

        event_sender
            .send(Event::ASNItemsUpdated(
                self.asn_id,
                updated_items.iter().map(|i| i.id).collect(),
            ))
            .await?;

        Ok(updated_items.into_iter().map(ASNItemResult::from).collect())
    }
}
