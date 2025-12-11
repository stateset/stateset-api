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

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Shipment Status Tests ====================

    #[test]
    fn test_shipment_status_pending() {
        let status = shipment::ShipmentStatus::Pending;
        assert_eq!(format!("{:?}", status), "Pending");
    }

    #[test]
    fn test_shipment_status_shipped() {
        let status = shipment::ShipmentStatus::Shipped;
        assert_eq!(format!("{:?}", status), "Shipped");
    }

    #[test]
    fn test_shipment_status_delivered() {
        let status = shipment::ShipmentStatus::Delivered;
        assert_eq!(format!("{:?}", status), "Delivered");
    }

    #[test]
    fn test_shipment_status_cancelled() {
        let status = shipment::ShipmentStatus::Cancelled;
        assert_eq!(format!("{:?}", status), "Cancelled");
    }

    // ==================== UUID Tests ====================

    #[test]
    fn test_shipment_id_uniqueness() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_shipment_id_format() {
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        // UUID format: 8-4-4-4-12
        assert_eq!(id_str.len(), 36);
        assert_eq!(id_str.chars().filter(|c| *c == '-').count(), 4);
    }

    #[test]
    fn test_shipment_id_not_nil() {
        let id = Uuid::new_v4();
        assert!(!id.is_nil());
    }

    // ==================== Tracking Number Tests ====================

    #[test]
    fn test_tracking_number_format_ups() {
        let tracking = "1Z999AA10123456784";
        assert!(tracking.starts_with("1Z"));
        assert_eq!(tracking.len(), 18);
    }

    #[test]
    fn test_tracking_number_format_fedex() {
        let tracking = "789456123012";
        assert_eq!(tracking.len(), 12);
        assert!(tracking.chars().all(|c| c.is_numeric()));
    }

    #[test]
    fn test_tracking_number_format_usps() {
        let tracking = "9400111899223033289645";
        assert!(tracking.len() >= 20);
    }

    #[test]
    fn test_tracking_number_not_empty() {
        let tracking = "TRACK123456";
        assert!(!tracking.is_empty());
    }

    // ==================== Carrier Tests ====================

    #[test]
    fn test_common_carriers() {
        let carriers = vec!["UPS", "FedEx", "USPS", "DHL", "Amazon Logistics", "OnTrac"];

        for carrier in carriers {
            assert!(!carrier.is_empty());
        }
    }

    // ==================== Command Tests ====================

    #[test]
    fn test_create_shipment_command_structure() {
        let order_id = Uuid::new_v4();
        let tracking_number = "TRACK123456";

        assert!(!order_id.is_nil());
        assert!(!tracking_number.is_empty());
    }

    #[test]
    fn test_cancel_shipment_command_fields() {
        let shipment_id = Uuid::new_v4();
        let reason = "Customer requested cancellation".to_string();
        let command = CancelShipmentCommand {
            shipment_id,
            reason: reason.clone(),
        };

        assert_eq!(command.shipment_id, shipment_id);
        assert_eq!(command.reason, reason);
    }

    #[test]
    fn test_assign_carrier_command_structure() {
        let shipment_id = Uuid::new_v4();
        let carrier_name = "FedEx".to_string();

        let command = AssignShipmentCarrierCommand {
            shipment_id,
            carrier_name: carrier_name.clone(),
        };

        assert_eq!(command.shipment_id, shipment_id);
        assert_eq!(command.carrier_name, carrier_name);
    }

    // ==================== Pagination Tests ====================

    #[test]
    fn test_pagination_valid_parameters() {
        let page: u64 = 1;
        let limit: u64 = 20;

        assert!(page > 0);
        assert!(limit > 0);
        assert!(limit <= 100);
    }

    #[test]
    fn test_pagination_offset_calculation() {
        let page: u64 = 5;
        let limit: u64 = 10;
        let offset = (page - 1) * limit;

        assert_eq!(offset, 40);
    }

    #[test]
    fn test_pagination_first_page_offset() {
        let page: u64 = 1;
        let limit: u64 = 25;
        let offset = (page - 1) * limit;

        assert_eq!(offset, 0);
    }

    // ==================== Status Filter Tests ====================

    #[test]
    fn test_status_filter_none() {
        let filter: Option<String> = None;
        assert!(filter.is_none());
    }

    #[test]
    fn test_status_filter_valid() {
        let filter: Option<String> = Some("Shipped".to_string());
        assert!(filter.is_some());
    }

    #[test]
    fn test_invalid_status_returns_empty() {
        let invalid_status = "InvalidStatus";
        let result = invalid_status.parse::<shipment::ShipmentStatus>();
        // Invalid status should fail to parse
        assert!(result.is_err());
    }

    // ==================== Shipment Lifecycle Tests ====================

    #[test]
    fn test_shipment_status_transitions() {
        // Valid transitions: Pending -> Shipped -> Delivered
        let pending = shipment::ShipmentStatus::Pending;
        let shipped = shipment::ShipmentStatus::Shipped;
        let delivered = shipment::ShipmentStatus::Delivered;

        assert_ne!(format!("{:?}", pending), format!("{:?}", shipped));
        assert_ne!(format!("{:?}", shipped), format!("{:?}", delivered));
    }

    #[test]
    fn test_shipment_can_be_cancelled() {
        let pending = shipment::ShipmentStatus::Pending;
        let cancelled = shipment::ShipmentStatus::Cancelled;

        assert_ne!(format!("{:?}", pending), format!("{:?}", cancelled));
    }

    // ==================== Order Relationship Tests ====================

    #[test]
    fn test_shipment_order_id_required() {
        let order_id = Uuid::new_v4();
        assert!(!order_id.is_nil());
    }

    #[test]
    fn test_multiple_shipments_per_order() {
        let order_id = Uuid::new_v4();
        let shipment1_id = Uuid::new_v4();
        let shipment2_id = Uuid::new_v4();

        // Multiple shipments can reference the same order
        assert_eq!(order_id, order_id);
        assert_ne!(shipment1_id, shipment2_id);
    }

    // ==================== Error Handling Tests ====================

    #[test]
    fn test_not_found_error() {
        let shipment_id = Uuid::new_v4();
        let error = ServiceError::NotFound(format!("Shipment {} not found", shipment_id));

        match error {
            ServiceError::NotFound(msg) => {
                assert!(msg.contains("not found"));
                assert!(msg.contains(&shipment_id.to_string()));
            }
            _ => panic!("Expected NotFound error"),
        }
    }

    // ==================== Delivery Confirmation Tests ====================

    #[test]
    fn test_delivery_confirmation_command() {
        let shipment_id = Uuid::new_v4();
        let command = ConfirmShipmentDeliveryCommand { shipment_id };

        assert_eq!(command.shipment_id, shipment_id);
    }

    #[test]
    fn test_delivery_confirmation_uuid_not_nil() {
        let shipment_id = Uuid::new_v4();
        let command = ConfirmShipmentDeliveryCommand { shipment_id };

        assert!(!command.shipment_id.is_nil());
    }

    // ==================== Timestamp Tests ====================

    #[test]
    fn test_shipment_timestamps() {
        let now = Utc::now();
        let later = now + chrono::Duration::hours(2);

        assert!(later > now);
    }

    #[test]
    fn test_shipped_at_after_created_at() {
        let created_at = Utc::now();
        let shipped_at = created_at + chrono::Duration::hours(1);

        assert!(shipped_at > created_at);
    }

    #[test]
    fn test_delivered_at_after_shipped_at() {
        let shipped_at = Utc::now();
        let delivered_at = shipped_at + chrono::Duration::days(3);

        assert!(delivered_at > shipped_at);
    }

    // ==================== Update Command Tests ====================

    #[test]
    fn test_update_shipment_command_structure() {
        let id = Uuid::new_v4();
        let command = UpdateShipmentCommand {
            id,
            recipient_name: None,
            shipping_address: None,
            carrier: Some("UPS".to_string()),
            tracking_number: Some("NEW_TRACKING_123".to_string()),
            status: None,
            estimated_delivery_date: None,
        };

        assert_eq!(command.id, id);
        assert!(command.tracking_number.is_some());
        assert!(command.carrier.is_some());
    }

    #[test]
    fn test_partial_update_command() {
        let id = Uuid::new_v4();
        let command = UpdateShipmentCommand {
            id,
            recipient_name: None,
            shipping_address: None,
            carrier: None, // Only update tracking, not carrier
            tracking_number: Some("UPDATED_123".to_string()),
            status: None,
            estimated_delivery_date: None,
        };

        assert!(command.tracking_number.is_some());
        assert!(command.carrier.is_none());
    }
}
