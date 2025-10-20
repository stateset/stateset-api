use crate::circuit_breaker::CircuitBreaker;
use crate::message_queue::MessageQueue;
use crate::{
    // commands::advancedshippingnotice::{
        // create_asn_command::CreateASNCommand,
        // cancel_asn_command::CancelASNCommand,
        // mark_asn_in_transit_command::MarkASNInTransitCommand,
        // mark_asn_delivered_command::MarkASNDeliveredCommand,
        // hold_asn_command::HoldASNCommand,
        // release_asn_from_hold_command::ReleaseASNFromHoldCommand,
        // add_item_to_asn_command::AddItemToASNCommand,
        // update_asn_details_command::UpdateAsnDetailsCommand,
    // },
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::asn_entity,
};
use anyhow::Result;
use redis::Client as RedisClient;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect, PaginatorTrait};
use slog::Logger;
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;

/// Service for managing Advanced Shipping Notices (ASNs)
#[derive(Clone)]
pub struct ASNService {
    db_pool: Arc<DbPool>,
    event_sender: Arc<EventSender>,
    redis_client: Arc<RedisClient>,
    message_queue: Arc<dyn MessageQueue>,
    circuit_breaker: Arc<CircuitBreaker>,
    logger: Logger,
}

impl ASNService {
    /// Creates a new ASN service instance
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

    /// Creates a new ASN
    #[instrument(skip(self))]
    pub async fn create_asn(
        &self,
        command: CreateASNCommand,
    ) -> Result<Uuid, ServiceError> {
        let result = command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(result.id)
    }

    /// Gets an ASN by ID
    #[instrument(skip(self))]
    pub async fn get_asn(
        &self,
        asn_id: &Uuid,
    ) -> Result<Option<asn_entity::Model>, ServiceError> {
        let db = &*self.db_pool;
        let asn = asn_entity::Entity::find_by_id(*asn_id)
            .one(db)
            .await
            .map_err(|e| {
                error!("Failed to get ASN by ID {}: {}", asn_id, e);
                ServiceError::db_error(e)
            })?;
        Ok(asn)
    }

    /// Lists ASNs with pagination
    #[instrument(skip(self))]
    pub async fn list_asns(
        &self,
        page: u64,
        limit: u64,
        supplier_id: Option<Uuid>,
        status: Option<asn_entity::ASNStatus>,
    ) -> Result<(Vec<asn_entity::Model>, u64), ServiceError> {
        if page == 0 {
            return Err(ServiceError::ValidationError(
                "Page number must be greater than 0".to_string()
            ));
        }

        if limit == 0 || limit > 1000 {
            return Err(ServiceError::ValidationError(
                "Limit must be between 1 and 1000".to_string()
            ));
        }

        let db = &*self.db_pool;

        // Build query with filters
        let mut query = asn_entity::Entity::find();

        if let Some(supplier_id) = supplier_id {
            query = query.filter(asn_entity::Column::SupplierId.eq(supplier_id));
        }

        if let Some(status) = status {
            query = query.filter(asn_entity::Column::Status.eq(status));
        }

        // Create paginator
        let paginator = query.paginate(db, limit);

        // Get total count
        let total = paginator.num_items().await.map_err(|e| {
            error!("Failed to count ASNs: {}", e);
            ServiceError::InternalError(format!("Failed to count ASNs: {}", e))
        })?;

        // Get requested page (0-indexed)
        let asns = paginator.fetch_page(page - 1).await.map_err(|e| {
            error!("Failed to fetch ASNs page {}: {}", page, e);
            ServiceError::InternalError(format!("Failed to fetch ASNs: {}", e))
        })?;

        Ok((asns, total))
    }

    /// Updates an ASN
    #[instrument(skip(self))]
    pub async fn update_asn(
        &self,
        command: UpdateAsnDetailsCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    /// Cancels an ASN
    #[instrument(skip(self))]
    pub async fn cancel_asn(
        &self,
        command: CancelASNCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    /// Marks an ASN as in transit
    #[instrument(skip(self))]
    pub async fn mark_asn_in_transit(
        &self,
        command: MarkASNInTransitCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    /// Marks an ASN as delivered
    #[instrument(skip(self))]
    pub async fn mark_asn_delivered(
        &self,
        command: MarkASNDeliveredCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    /// Puts an ASN on hold
    #[instrument(skip(self))]
    pub async fn hold_asn(
        &self,
        command: HoldASNCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    /// Releases an ASN from hold
    #[instrument(skip(self))]
    pub async fn release_asn_from_hold(
        &self,
        command: ReleaseASNFromHoldCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    /// Adds an item to an ASN
    #[instrument(skip(self))]
    pub async fn add_item_to_asn(
        &self,
        command: AddItemToASNCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    /// Deletes an ASN
    #[instrument(skip(self))]
    pub async fn delete_asn(
        &self,
        asn_id: &Uuid,
    ) -> Result<(), ServiceError> {
        let db = &*self.db_pool;

        // Check if ASN exists
        let asn = asn_entity::Entity::find_by_id(*asn_id)
            .one(db)
            .await
            .map_err(|e| {
                error!("Failed to find ASN {} for deletion: {}", asn_id, e);
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| ServiceError::NotFound(format!("ASN {} not found", asn_id)))?;

        // Check if ASN can be deleted (only draft status)
        if asn.status != asn_entity::ASNStatus::Draft {
            return Err(ServiceError::ValidationError(
                "Only draft ASNs can be deleted".to_string()
            ));
        }

        // Delete the ASN
        asn_entity::Entity::delete_by_id(*asn_id)
            .exec(db)
            .await
            .map_err(|e| {
                error!("Failed to delete ASN {}: {}", asn_id, e);
                ServiceError::db_error(e)
            })?;

        // Send event
        let event = Event::ASNDeleted {
            asn_id: *asn_id,
            asn_number: asn.asn_number,
        };

        self.event_sender.send(event).await
            .map_err(|e| ServiceError::EventError(e.to_string()))?;

        info!("ASN {} deleted successfully", asn_id);
        Ok(())
    }
}
