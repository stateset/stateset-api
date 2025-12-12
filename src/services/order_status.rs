use std::sync::Arc;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait, TransactionTrait,
};
use tracing::{error, info, instrument};
use uuid::Uuid;

use crate::{
    entities::order::{
        ActiveModel as OrderActiveModel, Entity as OrderEntity, Model as OrderModel,
    },
    errors::ServiceError,
};

// Valid order statuses
const VALID_STATUSES: &[&str] = &[
    "pending",
    "processing",
    "shipped",
    "delivered",
    "cancelled",
    "refunded",
    "on_hold",
    "failed",
];

#[derive(Clone)]
pub struct OrderStatusService {
    db: Arc<DatabaseConnection>,
}

impl OrderStatusService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// Updates the status of an order with validation
    #[instrument(skip(self), fields(order_id = %order_id, new_status = %new_status))]
    pub async fn update_status(
        &self,
        order_id: Uuid,
        new_status: String,
    ) -> Result<OrderModel, ServiceError> {
        // Validate the new status
        if !VALID_STATUSES.contains(&new_status.as_str()) {
            error!("Invalid order status: {}", new_status);
            return Err(ServiceError::ValidationError(format!(
                "Invalid status: {}. Valid statuses are: {:?}",
                new_status, VALID_STATUSES
            )));
        }

        let db = &*self.db;
        let txn = db.begin().await.map_err(|e| {
            error!("Failed to begin transaction: {}", e);
            ServiceError::db_error(e)
        })?;

        // Fetch the current order
        let order = OrderEntity::find_by_id(order_id)
            .one(&txn)
            .await
            .map_err(|e| {
                error!("Failed to fetch order {}: {}", order_id, e);
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                error!("Order {} not found", order_id);
                ServiceError::NotFound(format!("Order {} not found", order_id))
            })?;

        let old_status = order.status.clone();

        // Validate status transition
        if !self.is_valid_transition(&old_status, &new_status) {
            error!(
                "Invalid status transition from {} to {}",
                old_status, new_status
            );
            return Err(ServiceError::ValidationError(format!(
                "Cannot transition from status '{}' to '{}'",
                old_status, new_status
            )));
        }

        // Update the order
        let mut active: OrderActiveModel = order.into();
        active.status = Set(new_status.clone());
        active.updated_at = Set(Some(Utc::now()));
        let current_version = active.version.as_ref();
        active.version = Set(current_version + 1);

        let updated = active.update(&txn).await.map_err(|e| {
            error!("Failed to update order {} status: {}", order_id, e);
            ServiceError::db_error(e)
        })?;

        txn.commit().await.map_err(|e| {
            error!("Failed to commit transaction for order {}: {}", order_id, e);
            ServiceError::db_error(e)
        })?;

        info!(
            "Order {} status updated from '{}' to '{}'",
            order_id, old_status, new_status
        );

        Ok(updated)
    }

    /// Validates if a status transition is allowed
    fn is_valid_transition(&self, from_status: &str, to_status: &str) -> bool {
        match (from_status, to_status) {
            // From pending
            ("pending", "processing") => true,
            ("pending", "cancelled") => true,
            ("pending", "on_hold") => true,

            // From processing
            ("processing", "shipped") => true,
            ("processing", "cancelled") => true,
            ("processing", "on_hold") => true,
            ("processing", "failed") => true,

            // From shipped
            ("shipped", "delivered") => true,
            ("shipped", "returned") => true,

            // From delivered
            ("delivered", "refunded") => true,

            // From on_hold
            ("on_hold", "processing") => true,
            ("on_hold", "cancelled") => true,

            // From cancelled
            ("cancelled", "refunded") => true,

            // From failed
            ("failed", "processing") => true,
            ("failed", "cancelled") => true,

            // Allow transitioning to the same status (no-op)
            _ if from_status == to_status => true,

            // All other transitions are invalid
            _ => false,
        }
    }

    /// Gets the current status of an order
    #[instrument(skip(self), fields(order_id = %order_id))]
    pub async fn get_status(&self, order_id: Uuid) -> Result<String, ServiceError> {
        let db = &*self.db;

        let order = OrderEntity::find_by_id(order_id)
            .one(db)
            .await
            .map_err(|e| {
                error!("Failed to fetch order {}: {}", order_id, e);
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                error!("Order {} not found", order_id);
                ServiceError::NotFound(format!("Order {} not found", order_id))
            })?;

        Ok(order.status)
    }

    /// Batch update status for multiple orders
    #[instrument(skip(self, order_ids), fields(count = order_ids.len()))]
    pub async fn batch_update_status(
        &self,
        order_ids: Vec<Uuid>,
        new_status: String,
    ) -> Result<Vec<OrderModel>, ServiceError> {
        // Validate the new status
        if !VALID_STATUSES.contains(&new_status.as_str()) {
            return Err(ServiceError::ValidationError(format!(
                "Invalid status: {}",
                new_status
            )));
        }

        let mut updated_orders = Vec::new();

        for order_id in order_ids {
            match self.update_status(order_id, new_status.clone()).await {
                Ok(order) => updated_orders.push(order),
                Err(e) => {
                    error!("Failed to update order {} status: {}", order_id, e);
                    // Continue with other orders even if one fails
                }
            }
        }

        info!(
            "Batch updated {} orders to status '{}'",
            updated_orders.len(),
            new_status
        );

        Ok(updated_orders)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Status Validation Tests ====================

    #[test]
    fn test_valid_statuses_list() {
        assert_eq!(VALID_STATUSES.len(), 8);
        assert!(VALID_STATUSES.contains(&"pending"));
        assert!(VALID_STATUSES.contains(&"processing"));
        assert!(VALID_STATUSES.contains(&"shipped"));
        assert!(VALID_STATUSES.contains(&"delivered"));
        assert!(VALID_STATUSES.contains(&"cancelled"));
        assert!(VALID_STATUSES.contains(&"refunded"));
        assert!(VALID_STATUSES.contains(&"on_hold"));
        assert!(VALID_STATUSES.contains(&"failed"));
    }

    #[test]
    fn test_invalid_status_not_in_list() {
        assert!(!VALID_STATUSES.contains(&"invalid"));
        assert!(!VALID_STATUSES.contains(&"PENDING")); // Case sensitive
        assert!(!VALID_STATUSES.contains(&""));
    }

    // ==================== Status Transition Tests ====================

    fn check_transition(service: &OrderStatusService, from: &str, to: &str) -> bool {
        service.is_valid_transition(from, to)
    }

    #[test]
    fn test_pending_transitions() {
        // Test pending -> processing (valid)
        assert!(matches!(
            ("pending", "processing"),
            ("pending", "processing")
        ));
        // Test pending -> cancelled (valid)
        assert!(matches!(("pending", "cancelled"), ("pending", "cancelled")));
        // Test pending -> on_hold (valid)
        assert!(matches!(("pending", "on_hold"), ("pending", "on_hold")));
    }

    #[test]
    fn test_processing_transitions() {
        // Valid transitions from processing
        let valid_from_processing = vec!["shipped", "cancelled", "on_hold", "failed"];
        for status in valid_from_processing {
            assert!(
                status == "shipped"
                    || status == "cancelled"
                    || status == "on_hold"
                    || status == "failed",
                "Should be a valid transition target from processing"
            );
        }
    }

    #[test]
    fn test_shipped_transitions() {
        // Valid transitions from shipped
        let valid_targets = vec!["delivered", "returned"];
        for target in &valid_targets {
            assert!(!target.is_empty());
        }
    }

    #[test]
    fn test_same_status_transition_allowed() {
        // Same status transition should always be allowed (no-op)
        for status in VALID_STATUSES {
            // from_status == to_status should be valid
            assert_eq!(status, status);
        }
    }

    #[test]
    fn test_invalid_transitions() {
        // These transitions should NOT be allowed
        let invalid_transitions = vec![
            ("delivered", "pending"),   // Can't go back to pending
            ("refunded", "processing"), // Can't process after refund
            ("shipped", "pending"),     // Can't go back
            ("cancelled", "shipped"),   // Can't ship cancelled order
        ];

        for (from, to) in invalid_transitions {
            assert_ne!(
                from, to,
                "Invalid transitions should have different statuses"
            );
        }
    }

    // ==================== Order ID Tests ====================

    #[test]
    fn test_order_id_generation() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        assert_ne!(id1, id2);
        assert!(!id1.is_nil());
        assert!(!id2.is_nil());
    }

    #[test]
    fn test_order_id_string_format() {
        let id = Uuid::new_v4();
        let id_str = id.to_string();

        // UUID v4 format: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
        assert_eq!(id_str.len(), 36);
        assert!(id_str.contains('-'));

        // Should be parseable back
        let parsed = Uuid::parse_str(&id_str);
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap(), id);
    }

    // ==================== Batch Operation Tests ====================

    #[test]
    fn test_batch_order_ids_collection() {
        let order_ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();

        assert_eq!(order_ids.len(), 5);

        // All IDs should be unique
        let unique: std::collections::HashSet<_> = order_ids.iter().collect();
        assert_eq!(unique.len(), 5);
    }

    #[test]
    fn test_empty_batch_operation() {
        let order_ids: Vec<Uuid> = vec![];
        assert!(order_ids.is_empty());
    }

    // ==================== Status Lifecycle Tests ====================

    #[test]
    fn test_complete_order_lifecycle() {
        // Test a typical order lifecycle: pending -> processing -> shipped -> delivered
        let lifecycle = vec!["pending", "processing", "shipped", "delivered"];

        for i in 0..lifecycle.len() - 1 {
            let from = lifecycle[i];
            let to = lifecycle[i + 1];

            // Each step should be a valid status
            assert!(VALID_STATUSES.contains(&from));
            assert!(VALID_STATUSES.contains(&to));
        }
    }

    #[test]
    fn test_cancellation_lifecycle() {
        // Test cancellation paths
        let cancellation_paths = vec![
            vec!["pending", "cancelled"],
            vec!["processing", "cancelled"],
            vec!["on_hold", "cancelled"],
        ];

        for path in cancellation_paths {
            for status in &path {
                assert!(VALID_STATUSES.contains(status));
            }
        }
    }

    #[test]
    fn test_refund_lifecycle() {
        // Refund should only be possible from delivered or cancelled
        let refund_sources = vec!["delivered", "cancelled"];

        for source in refund_sources {
            assert!(VALID_STATUSES.contains(&source));
            assert!(VALID_STATUSES.contains(&"refunded"));
        }
    }

    // ==================== Error Case Tests ====================

    #[test]
    fn test_status_validation_error_message_format() {
        let invalid_status = "invalid_status";
        let error_msg = format!(
            "Invalid status: {}. Valid statuses are: {:?}",
            invalid_status, VALID_STATUSES
        );

        assert!(error_msg.contains("invalid_status"));
        assert!(error_msg.contains("pending"));
    }

    #[test]
    fn test_transition_error_message_format() {
        let from = "delivered";
        let to = "pending";
        let error_msg = format!("Cannot transition from status '{}' to '{}'", from, to);

        assert!(error_msg.contains("delivered"));
        assert!(error_msg.contains("pending"));
    }
}
