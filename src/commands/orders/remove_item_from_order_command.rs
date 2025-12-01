use uuid::Uuid;
use crate::commands::Command;
use crate::events::{Event, EventSender};
use crate::{
    db::DbPool,
    errors::ServiceError,
    models::{order_item_entity, order_item_entity::Entity as OrderItem},
};
use async_trait::async_trait;
use lazy_static::lazy_static;
use prometheus::IntCounter;
use sea_orm::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

lazy_static! {
    static ref ORDER_ITEMS_REMOVED: IntCounter = IntCounter::new(
        "order_items_removed_total",
        "Total number of items removed from orders"
    )
    .expect("metric can be created");
    static ref ORDER_ITEM_REMOVE_FAILURES: IntCounter = IntCounter::new(
        "order_item_remove_failures_total",
        "Total number of failed item removals from orders"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RemoveItemFromOrderCommand {
    pub order_id: Uuid,
    pub item_id: Uuid,
}

#[async_trait]
impl Command for RemoveItemFromOrderCommand {
    type Result = ();

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        let db = db_pool.as_ref();

        let delete_result = OrderItem::delete_many()
            .filter(
                Condition::all()
                    .add(order_item_entity::Column::Id.eq(self.item_id))
                    .add(order_item_entity::Column::OrderId.eq(self.order_id)),
            )
            .exec(db)
            .await
            .map_err(|e| {
                ORDER_ITEM_REMOVE_FAILURES.inc();
                let msg = format!(
                    "Failed to remove item {} from order {}: {}",
                    self.item_id, self.order_id, e
                );
                error!("{}", msg);
                ServiceError::db_error(e)
            })?;

        if delete_result.rows_affected == 0 {
            ORDER_ITEM_REMOVE_FAILURES.inc();
            error!(
                "Item {} not found in order {}. No rows were deleted.",
                self.item_id, self.order_id
            );
            return Err(ServiceError::NotFound(format!(
                "Item {} not found in order {}",
                self.item_id, self.order_id
            )));
        }

        // Trigger an event indicating that an item was removed from the order
        event_sender
            .send(Event::OrderUpdated {
                order_id: self.order_id,
                checkout_session_id: None,
                status: None,
                refunds: vec![],
            })
            .await
            .map_err(ServiceError::EventError)?;

        ORDER_ITEMS_REMOVED.inc();

        info!(
            order_id = %self.order_id,
            item_id = %self.item_id,
            "Item removed from order successfully"
        );

        Ok(())
    }
}
