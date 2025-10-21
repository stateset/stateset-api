use crate::circuit_breaker::CircuitBreaker;
use crate::message_queue::MessageQueue;
use crate::{
    commands::warranties::{
        approve_warranty_claim_command::ApproveWarrantyClaimCommand,
        claim_warranty_command::ClaimWarrantyCommand,
        create_warranty_command::CreateWarrantyCommand,
        reject_warranty_claim_command::RejectWarrantyClaimCommand,
    },
    commands::Command,
    db::DbPool,
    entities::warranty,
    errors::ServiceError,
    events::{Event, EventSender},
};
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use redis::Client as RedisClient;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use slog::Logger;
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;

/// Service for managing warranties
#[derive(Clone)]
pub struct WarrantyService {
    db_pool: Arc<DbPool>,
    event_sender: Arc<EventSender>,
    redis_client: Arc<RedisClient>,
    message_queue: Arc<dyn MessageQueue>,
    circuit_breaker: Arc<CircuitBreaker>,
    logger: Logger,
}

impl WarrantyService {
    /// Creates a new warranty service instance
    pub fn new(
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
        redis_client: Arc<RedisClient>,
        message_queue: Arc<dyn MessageQueue>,
        circuit_breaker: Arc<CircuitBreaker>,
        logger: Logger,
    ) -> Self {
        Self {
            db_pool,
            event_sender,
            redis_client,
            message_queue,
            circuit_breaker,
            logger,
        }
    }

    /// Creates a new warranty
    #[instrument(skip(self))]
    pub async fn create_warranty(
        &self,
        command: CreateWarrantyCommand,
    ) -> Result<Uuid, ServiceError> {
        let result = command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        // Outbox: WarrantyCreated
        let payload = serde_json::json!({"warranty_id": result.to_string()});
        let _ = crate::events::outbox::enqueue(
            &*self.db_pool,
            "warranty",
            Some(result),
            "WarrantyCreated",
            &payload,
        )
        .await;
        Ok(result)
    }

    /// Processes a warranty claim
    #[instrument(skip(self))]
    pub async fn claim_warranty(
        &self,
        command: ClaimWarrantyCommand,
    ) -> Result<Uuid, ServiceError> {
        let result = command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        // Outbox: WarrantyClaimed
        let payload = serde_json::json!({"warranty_id": result.to_string()});
        let _ = crate::events::outbox::enqueue(
            &*self.db_pool,
            "warranty",
            Some(result),
            "WarrantyClaimed",
            &payload,
        )
        .await;
        Ok(result)
    }

    /// Approves a warranty claim
    #[instrument(skip(self))]
    pub async fn approve_warranty_claim(
        &self,
        command: ApproveWarrantyClaimCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        // Outbox: WarrantyClaimApproved
        let payload = serde_json::json!({"claim_id": command.claim_id.to_string()});
        let _ = crate::events::outbox::enqueue(
            &*self.db_pool,
            "warranty",
            Some(command.claim_id),
            "WarrantyClaimApproved",
            &payload,
        )
        .await;
        Ok(())
    }

    /// Rejects a warranty claim
    #[instrument(skip(self))]
    pub async fn reject_warranty_claim(
        &self,
        command: RejectWarrantyClaimCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        // Outbox: WarrantyClaimRejected
        let payload = serde_json::json!({"claim_id": command.claim_id.to_string()});
        let _ = crate::events::outbox::enqueue(
            &*self.db_pool,
            "warranty",
            Some(command.claim_id),
            "WarrantyClaimRejected",
            &payload,
        )
        .await;
        Ok(())
    }

    /// Gets a warranty by ID
    #[instrument(skip(self))]
    pub async fn get_warranty(
        &self,
        warranty_id: &Uuid,
    ) -> Result<Option<warranty::Model>, ServiceError> {
        let db = self.db_pool.as_ref();
        let warranty = warranty::Entity::find_by_id(*warranty_id).one(db).await?;
        Ok(warranty)
    }

    /// Extends the warranty duration by the provided number of months
    #[instrument(skip(self))]
    pub async fn extend_warranty(
        &self,
        warranty_id: Uuid,
        additional_months: i32,
    ) -> Result<warranty::Model, ServiceError> {
        if additional_months <= 0 {
            return Err(ServiceError::ValidationError(
                "additional_months must be positive".to_string(),
            ));
        }

        let db = self.db_pool.as_ref();
        let existing = warranty::Entity::find_by_id(warranty_id)
            .one(db)
            .await?
            .ok_or_else(|| ServiceError::NotFound(format!("Warranty {} not found", warranty_id)))?;

        let current_end = existing.end_date;
        let mut active: warranty::ActiveModel = existing.into();
        let extension_days = Duration::days((additional_months as i64) * 30);
        let new_end = current_end + extension_days;
        active.end_date = Set(new_end);
        active.updated_at = Set(Some(Utc::now()));

        let updated = active.update(db).await.map_err(ServiceError::db_error)?;
        Ok(updated)
    }

    /// Gets warranties for a product
    #[instrument(skip(self))]
    pub async fn get_warranties_for_product(
        &self,
        product_id: &Uuid,
    ) -> Result<Vec<warranty::Model>, ServiceError> {
        let db = self.db_pool.as_ref();
        let warranties = warranty::Entity::find()
            .filter(warranty::Column::ProductId.eq(*product_id))
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        Ok(warranties)
    }

    /// Gets active warranties for a customer
    #[instrument(skip(self))]
    pub async fn get_active_warranties_for_customer(
        &self,
        customer_id: &Uuid,
    ) -> Result<Vec<warranty::Model>, ServiceError> {
        let db = self.db_pool.as_ref();
        let now = Utc::now();

        let warranties = warranty::Entity::find()
            .filter(warranty::Column::CustomerId.eq(*customer_id))
            .filter(warranty::Column::EndDate.gte(now))
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        Ok(warranties)
    }

    /// Checks if a product is under warranty
    #[instrument(skip(self))]
    pub async fn is_under_warranty(
        &self,
        product_id: &Uuid,
        serial_number: &str,
    ) -> Result<bool, ServiceError> {
        let db = self.db_pool.as_ref();
        let _ = serial_number;

        let warranty_exists = warranty::Entity::find()
            .filter(warranty::Column::ProductId.eq(*product_id))
            .filter(warranty::Column::Status.eq("active"))
            .count(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        Ok(warranty_exists > 0)
    }

    pub async fn list_warranties(
        &self,
        page: u64,
        limit: u64,
    ) -> Result<(Vec<warranty::Model>, u64), ServiceError> {
        let db = self.db_pool.as_ref();
        let total = warranty::Entity::find()
            .count(db)
            .await
            .map_err(|e| ServiceError::db_error(e))? as u64;

        let offset = page.saturating_sub(1) * limit;

        let warranties = warranty::Entity::find()
            .order_by_desc(warranty::Column::CreatedAt)
            .offset(offset)
            .limit(limit)
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        Ok((warranties, total))
    }
}
