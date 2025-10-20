use uuid::Uuid;
use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::suppliers::{self, Entity as Supplier, SupplierRating, SupplierStatus},
};
use sea_orm::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ListSuppliersCommand {
    pub status: Option<SupplierStatus>,
    pub rating: Option<SupplierRating>,
    #[validate(range(min = 1, max = 1000))]
    pub limit: Option<u64>,
}

#[async_trait::async_trait]
impl Command for ListSuppliersCommand {
    type Result = Vec<suppliers::Model>;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        let db = db_pool.as_ref();
        let mut query = Supplier::find();

        if let Some(status) = self.status {
            query = query.filter(suppliers::Column::Status.eq(status));
        }
        if let Some(rating) = self.rating {
            query = query.filter(suppliers::Column::Rating.eq(rating));
        }
        if let Some(limit) = self.limit {
            query = query.limit(limit);
        }

        let suppliers = query.all(db).await.map_err(|e| {
            let msg = format!("Failed to list suppliers: {}", e);
            error!("{}", msg);
            ServiceError::db_error(msg)
        })?;

        info!(count = suppliers.len(), "Listed suppliers");
        event_sender
            .send(Event::with_data("suppliers_listed".to_string()))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(suppliers)
    }
}
