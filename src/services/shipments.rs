use crate::circuit_breaker::{CircuitBreaker, CircuitBreakerRegistry};
use crate::message_queue::MessageQueue;
use crate::{
    commands::shipments::{
        assign_shipment_carrier_command::AssignShipmentCarrierCommand,
        cancel_shipment_command::CancelShipmentCommand,
        confirm_shipment_delivery_command::ConfirmShipmentDeliveryCommand,
        create_shipment_command::CreateShipmentCommand,
        track_shipment_command::TrackShipmentCommand,
        update_shipment_general_command::UpdateShipmentCommand,
    },
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::EventSender,
    models::shipment,
};
use anyhow::Result;
use chrono::Utc;
use redis::Client as RedisClient;
use sea_orm::PaginatorTrait;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use slog::Logger;
use std::sync::Arc;
use tracing::instrument;
use uuid::Uuid;

/// Service for managing shipments
#[derive(Clone)]
pub struct ShipmentService {
    db_pool: Arc<DbPool>,
    event_sender: Arc<EventSender>,
    redis_client: Arc<RedisClient>,
    message_queue: Arc<dyn MessageQueue>,
    circuit_breaker: Arc<CircuitBreaker>,
    circuit_breaker_registry: Arc<CircuitBreakerRegistry>,
    logger: Logger,
}

impl ShipmentService {
    /// Creates a new shipment service instance
    pub fn new(
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
        redis_client: Arc<RedisClient>,
        message_queue: Arc<dyn MessageQueue>,
        circuit_breaker: Arc<CircuitBreaker>,
        circuit_breaker_registry: Arc<CircuitBreakerRegistry>,
        logger: Logger,
    ) -> Self {
        Self {
            db_pool,
            event_sender,
            redis_client,
            message_queue,
            circuit_breaker,
            circuit_breaker_registry,
            logger,
        }
    }

    /// Creates a new shipment
    #[instrument(skip(self))]
    pub async fn create_shipment(
        &self,
        command: CreateShipmentCommand,
    ) -> Result<Uuid, ServiceError> {
        let result = command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        // Outbox: ShipmentCreated
        let payload = serde_json::json!({
            "shipment_id": result.id.to_string(),
            "order_id": result.order_id.to_string(),
            "tracking_number": result.tracking_number,
        });
        let _ = crate::events::outbox::enqueue(
            &*self.db_pool,
            "shipment",
            Some(result.id),
            "ShipmentCreated",
            &payload,
        )
        .await;
        Ok(result.id)
    }

    /// Updates a shipment
    #[instrument(skip(self))]
    pub async fn update_shipment(
        &self,
        command: UpdateShipmentCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    /// Cancels a shipment
    #[instrument(skip(self))]
    pub async fn cancel_shipment(
        &self,
        command: CancelShipmentCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    /// Tracks a shipment
    #[instrument(skip(self))]
    pub async fn track_shipment(
        &self,
        mut command: TrackShipmentCommand,
    ) -> Result<String, ServiceError> {
        // Provide the circuit breaker registry to the command
        command.circuit_breaker = Some(self.circuit_breaker.clone());

        let result = command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(result.status.to_string()) // Assuming status is ShipmentStatus
    }

    /// Finds shipment by tracking number
    #[instrument(skip(self))]
    pub async fn find_by_tracking_number(
        &self,
        tracking_number: &str,
    ) -> Result<Option<shipment::Model>, ServiceError> {
        let db = &*self.db_pool;
        let found = shipment::Entity::find()
            .filter(shipment::Column::TrackingNumber.eq(tracking_number.to_string()))
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;
        Ok(found)
    }

    /// Marks a shipment as shipped
    #[instrument(skip(self))]
    pub async fn mark_shipped(&self, shipment_id: Uuid) -> Result<shipment::Model, ServiceError> {
        self.update_status(shipment_id, shipment::ShipmentStatus::Shipped, true, false)
            .await
    }

    /// Marks a shipment as delivered
    #[instrument(skip(self))]
    pub async fn mark_delivered(&self, shipment_id: Uuid) -> Result<shipment::Model, ServiceError> {
        self.update_status(
            shipment_id,
            shipment::ShipmentStatus::Delivered,
            false,
            true,
        )
        .await
    }

    async fn update_status(
        &self,
        shipment_id: Uuid,
        status: shipment::ShipmentStatus,
        set_shipped_at: bool,
        set_delivered_at: bool,
    ) -> Result<shipment::Model, ServiceError> {
        let db = &*self.db_pool;
        let model = shipment::Entity::find_by_id(shipment_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| ServiceError::NotFound(format!("Shipment {} not found", shipment_id)))?;

        let mut active: shipment::ActiveModel = model.into();
        active.status = Set(status);
        if set_shipped_at {
            active.shipped_at = Set(Some(Utc::now()));
        }
        if set_delivered_at {
            active.delivered_at = Set(Some(Utc::now()));
        }
        active.updated_at = Set(Utc::now());

        let updated = active.update(db).await.map_err(ServiceError::db_error)?;
        Ok(updated)
    }

    /// Confirms delivery of a shipment
    #[instrument(skip(self))]
    pub async fn confirm_delivery(
        &self,
        command: ConfirmShipmentDeliveryCommand,
    ) -> Result<(), ServiceError> {
        let _updated = command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    /// Assigns a carrier to a shipment
    #[instrument(skip(self))]
    pub async fn assign_carrier(
        &self,
        command: AssignShipmentCarrierCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    // /// Processes the shipping of a shipment
    // #[instrument(skip(self))]
    // // pub async fn ship(
    //     &self,
    //     command: ShipOrderCommand,
    // ) -> Result<(), ServiceError> {
    //     command
    //         .execute(self.db_pool.clone(), self.event_sender.clone())
    //         .await?;
    //     Ok(())
    // }

    /// Gets a shipment by ID
    #[instrument(skip(self))]
    pub async fn get_shipment(
        &self,
        shipment_id: Uuid,
    ) -> Result<Option<shipment::Model>, ServiceError> {
        let db = &*self.db_pool;
        let shipment = shipment::Entity::find_by_id(shipment_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        Ok(shipment)
    }

    /// Gets shipments for an order
    #[instrument(skip(self))]
    pub async fn get_shipments_for_order(
        &self,
        order_id: &Uuid,
    ) -> Result<Vec<shipment::Model>, ServiceError> {
        let db = &*self.db_pool;
        let shipments = shipment::Entity::find()
            .filter(shipment::Column::OrderId.eq(*order_id))
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        Ok(shipments)
    }

    /// Lists all shipments with pagination
    #[instrument(skip(self))]
    pub async fn list_shipments(
        &self,
        page: u64,
        limit: u64,
        status: Option<String>,
    ) -> Result<(Vec<shipment::Model>, u64), ServiceError> {
        let db = &*self.db_pool;

        // Build query with optional status filter
        let mut query = shipment::Entity::find();

        // Apply status filter if provided (database-side filtering)
        if let Some(status_filter) = status {
            // Parse status to ensure it's valid
            if let Ok(parsed_status) = status_filter.parse::<shipment::ShipmentStatus>() {
                query = query.filter(shipment::Column::Status.eq(parsed_status));
            } else {
                // If invalid status, return empty results
                return Ok((vec![], 0));
            }
        }

        // Create a paginator for the shipments
        let paginator = query
            .order_by_desc(shipment::Column::CreatedAt)
            .paginate(db, limit);

        // Get the total count of shipments (with filter applied)
        let total = paginator
            .num_items()
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        // Get the requested page of shipments
        let shipments = paginator
            .fetch_page(page - 1)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        Ok((shipments, total))
    }
}
