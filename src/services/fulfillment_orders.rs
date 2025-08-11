use sea_orm::{ActiveModelTrait, DatabaseConnection, Set, EntityTrait};
use std::sync::Arc;
use uuid::Uuid;

use crate::{errors::AppError, models::fulfillment_order};

pub struct FulfillmentOrderService {
    db: Arc<DatabaseConnection>,
}

impl FulfillmentOrderService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// Create a new fulfillment order for an existing order
    pub async fn create_fulfillment_order(&self, order_id: Uuid) -> Result<Uuid, AppError> {
        let model = fulfillment_order::ActiveModel {
            id: Set(Uuid::new_v4()),
            order_id: Set(order_id),
            status: Set(fulfillment_order::FulfillmentOrderStatus::Pending),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
        };
        let result = model.insert(&*self.db).await?;
        Ok(result.id)
    }

    /// Update the status of a fulfillment order
    pub async fn update_status(
        &self,
        fulfillment_order_id: Uuid,
        status: fulfillment_order::FulfillmentOrderStatus,
    ) -> Result<(), AppError> {
        let mut fo: fulfillment_order::ActiveModel =
            fulfillment_order::Entity::find_by_id(fulfillment_order_id)
                .one(&*self.db)
                .await?
                .ok_or_else(|| AppError::NotFound("fulfillment order not found".into()))?
                .into();
        fo.status = Set(status);
        fo.updated_at = Set(chrono::Utc::now());
        fo.update(&*self.db).await?;
        Ok(())
    }
}
