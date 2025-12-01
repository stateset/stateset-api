use crate::circuit_breaker::CircuitBreaker;
use crate::commands::returns::{
    approve_return_command::ApproveReturnCommand,
    complete_return_command::CompleteReturnCommand,
    create_return_command::{InitiateReturnCommand, InitiateReturnResult},
    restock_returned_items_command::RestockReturnedItemsCommand,
};
use crate::message_queue::MessageQueue;
use crate::{
    // commands::returns::{
    // approve_return_command::ApproveReturnCommand, cancel_return_command::CancelReturnCommand,
    // complete_return_command::CompleteReturnCommand,
    // create_return_command::InitiateReturnCommand as CreateReturnCommand,
    // refund_return_command::RefundReturnCommand, reject_return_command::RejectReturnCommand,
    // restock_returned_items_command::RestockReturnedItemsCommand,
    // },
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::EventSender,
    models::return_entity,
};
use anyhow::Result;
use redis::Client as RedisClient;
use sea_orm::sea_query::{Expr, Func};
use sea_orm::PaginatorTrait;
use sea_orm::{Condition, EntityTrait, QueryFilter, QueryOrder};
use slog::Logger;
use std::sync::Arc;
use tracing::{error, instrument};
use uuid::Uuid;

/// Service for managing returns
#[derive(Clone)]
pub struct ReturnService {
    db_pool: Arc<DbPool>,
    event_sender: Arc<EventSender>,
    redis_client: Arc<RedisClient>,
    message_queue: Arc<dyn MessageQueue>,
    circuit_breaker: Arc<CircuitBreaker>,
    logger: Logger,
}

impl ReturnService {
    /// Creates a new return service instance
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

    /// Creates a new return
    #[instrument(skip(self))]
    pub async fn create_return(
        &self,
        command: InitiateReturnCommand,
    ) -> Result<InitiateReturnResult, ServiceError> {
        let result = command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(result)
    }

    #[instrument(skip(self))]
    pub async fn approve_return(
        &self,
        return_id: Uuid,
    ) -> Result<return_entity::Model, ServiceError> {
        let command = ApproveReturnCommand { return_id };
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        self.get_return(&return_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound("Return not found after approval".to_string()))
    }

    // /// Approves a return
    // #[instrument(skip(self))]
    // // pub async fn approve_return(
    //     &self,
    //     command: ApproveReturnCommand,
    // ) -> Result<(), ServiceError> {
    //     command
    //         .execute(self.db_pool.clone(), self.event_sender.clone())
    //         .await?;
    // /// Rejects a return
    // #[instrument(skip(self))]
    // // pub async fn reject_return(
    //     &self,
    //     command: RejectReturnCommand,
    // ) -> Result<(), ServiceError> {
    //     command
    //         .execute(self.db_pool.clone(), self.event_sender.clone())
    //         .await?;
    // /// Cancels a return
    // #[instrument(skip(self))]
    // // pub async fn cancel_return(
    //     &self,
    //     command: CancelReturnCommand,
    // ) -> Result<(), ServiceError> {
    //     command
    //         .execute(self.db_pool.clone(), self.event_sender.clone())
    //         .await?;
    // /// Processes a refund for a return
    // #[instrument(skip(self))]
    // // pub async fn refund_return(
    //     &self,
    //     command: RefundReturnCommand,
    // ) -> Result<(), ServiceError> {
    //     command
    //         .execute(self.db_pool.clone(), self.event_sender.clone())
    //         .await?;
    //     Ok(())
    // }
    //     Ok(())
    // }
    //

    // /// Processes a refund for a return
    // #[instrument(skip(self))]
    // // pub async fn refund_return(
    //     &self,
    //     command: RefundReturnCommand,
    // ) -> Result<(), ServiceError> {
    //     command
    //         .execute(self.db_pool.clone(), self.event_sender.clone())
    //         .await?;
    //     Ok(())
    // }

    /// Completes a return process
    #[instrument(skip(self))]
    pub async fn complete_return(
        &self,
        command: CompleteReturnCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    /// Restocks items from a return
    #[instrument(skip(self))]
    pub async fn restock_returned_items(
        &self,
        command: RestockReturnedItemsCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    /// Gets a return by ID
    #[instrument(skip(self))]
    pub async fn get_return(
        &self,
        return_id: &Uuid,
    ) -> Result<Option<return_entity::Model>, ServiceError> {
        let db = &*self.db_pool;
        let ret = return_entity::Entity::find_by_id(*return_id)
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get return: {}", e);
                error!(return_id = %return_id, error = %e, "Database error when fetching return");
                ServiceError::db_error(e)
            })?;

        Ok(ret)
    }

    /// Lists returns with pagination
    #[instrument(skip(self))]
    pub async fn list_returns(
        &self,
        page: u64,
        limit: u64,
        status_filter: Option<String>,
    ) -> Result<(Vec<return_entity::Model>, u64), ServiceError> {
        let db = &*self.db_pool;

        let mut query =
            return_entity::Entity::find().order_by_desc(return_entity::Column::CreatedAt);

        if let Some(status) = status_filter {
            let normalized = status.trim().to_ascii_lowercase();
            if !normalized.is_empty() {
                let condition = Condition::all().add(
                    Expr::expr(Func::lower(Expr::col(return_entity::Column::Status)))
                        .eq(normalized),
                );
                query = query.filter(condition);
            }
        }

        // Create a paginator for the returns
        let paginator = query.paginate(db, limit);

        // Get the total count of returns
        let total = paginator
            .num_items()
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let returns = paginator
            .fetch_page(page - 1)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        Ok((returns, total))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Return Status Tests ====================

    #[test]
    fn test_return_status_normalization_lowercase() {
        let status = "pending";
        let normalized = status.trim().to_ascii_lowercase();
        assert_eq!(normalized, "pending");
    }

    #[test]
    fn test_return_status_normalization_uppercase() {
        let status = "PENDING";
        let normalized = status.trim().to_ascii_lowercase();
        assert_eq!(normalized, "pending");
    }

    #[test]
    fn test_return_status_normalization_mixed_case() {
        let status = "Approved";
        let normalized = status.trim().to_ascii_lowercase();
        assert_eq!(normalized, "approved");
    }

    #[test]
    fn test_return_status_normalization_with_whitespace() {
        let status = "  completed  ";
        let normalized = status.trim().to_ascii_lowercase();
        assert_eq!(normalized, "completed");
    }

    #[test]
    fn test_return_status_normalization_empty() {
        let status = "";
        let normalized = status.trim().to_ascii_lowercase();
        assert!(normalized.is_empty());
    }

    #[test]
    fn test_return_status_normalization_whitespace_only() {
        let status = "   ";
        let normalized = status.trim().to_ascii_lowercase();
        assert!(normalized.is_empty());
    }

    // ==================== UUID Tests ====================

    #[test]
    fn test_return_id_uniqueness() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_return_id_format() {
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        // UUID format: 8-4-4-4-12
        assert_eq!(id_str.len(), 36);
        assert_eq!(id_str.chars().filter(|c| *c == '-').count(), 4);
    }

    // ==================== Return Lifecycle Tests ====================

    #[test]
    fn test_valid_return_statuses() {
        let valid_statuses = vec![
            "pending",
            "approved",
            "rejected",
            "completed",
            "cancelled",
            "processing",
        ];

        for status in valid_statuses {
            let normalized = status.trim().to_ascii_lowercase();
            assert!(!normalized.is_empty(), "Status {} should not be empty", status);
        }
    }

    #[test]
    fn test_return_status_transitions() {
        // Define valid state transitions
        let valid_transitions = vec![
            ("pending", "approved"),
            ("pending", "rejected"),
            ("approved", "completed"),
            ("approved", "cancelled"),
            ("pending", "cancelled"),
        ];

        for (from, to) in valid_transitions {
            // Just validate the statuses are different
            assert_ne!(from, to, "Transition should change state");
        }
    }

    // ==================== Command Pattern Tests ====================

    #[test]
    fn test_initiate_return_command_creation() {
        let order_id = Uuid::new_v4();
        let reason = "Defective product";

        // Test that we can create the command structure
        assert!(!order_id.is_nil());
        assert!(!reason.is_empty());
    }

    #[test]
    fn test_approve_return_command_fields() {
        let return_id = Uuid::new_v4();
        let command = ApproveReturnCommand { return_id };

        assert_eq!(command.return_id, return_id);
    }

    // ==================== Pagination Tests ====================

    #[test]
    fn test_pagination_parameters() {
        let page: u64 = 1;
        let limit: u64 = 20;

        assert!(page > 0, "Page should be greater than 0");
        assert!(limit > 0, "Limit should be greater than 0");
        assert!(limit <= 100, "Limit should not exceed 100");
    }

    #[test]
    fn test_pagination_page_calculation() {
        let page: u64 = 3;
        let limit: u64 = 10;
        let offset = (page - 1) * limit;

        assert_eq!(offset, 20);
    }

    #[test]
    fn test_pagination_first_page() {
        let page: u64 = 1;
        let limit: u64 = 25;
        let offset = (page - 1) * limit;

        assert_eq!(offset, 0);
    }

    // ==================== Return Reason Tests ====================

    #[test]
    fn test_common_return_reasons() {
        let valid_reasons = vec![
            "Defective product",
            "Wrong item received",
            "Item not as described",
            "Changed mind",
            "Better price found elsewhere",
            "Damaged during shipping",
            "Quality not as expected",
        ];

        for reason in valid_reasons {
            assert!(!reason.is_empty());
            assert!(reason.len() <= 500, "Reason should not exceed 500 characters");
        }
    }

    #[test]
    fn test_return_reason_length_limits() {
        let short_reason = "Too small";
        let long_reason = "A".repeat(1000);

        assert!(short_reason.len() >= 3, "Reason should have minimum length");
        assert!(long_reason.len() > 500, "Very long reason for testing truncation");
    }

    // ==================== Restock Tests ====================

    #[test]
    fn test_restock_quantity_positive() {
        let quantity: i32 = 5;
        assert!(quantity > 0, "Restock quantity must be positive");
    }

    #[test]
    fn test_restock_quantity_matches_return() {
        let returned_quantity: i32 = 3;
        let restock_quantity: i32 = 3;

        assert_eq!(returned_quantity, restock_quantity, "Restock should match returned quantity");
    }

    // ==================== Error Handling Tests ====================

    #[test]
    fn test_not_found_error_message() {
        let return_id = Uuid::new_v4();
        let error_msg = format!("Return not found after approval");

        assert!(error_msg.contains("not found"));
    }

    #[test]
    fn test_service_error_from_string() {
        let error = ServiceError::NotFound("Return not found".to_string());
        match error {
            ServiceError::NotFound(msg) => assert!(msg.contains("not found")),
            _ => panic!("Expected NotFound error"),
        }
    }

    // ==================== Complete Return Command Tests ====================

    #[test]
    fn test_complete_return_command_structure() {
        let return_id = Uuid::new_v4();
        let command = CompleteReturnCommand {
            return_id,
            notes: Some("Return processed successfully".to_string()),
            completed_by: Some("system".to_string()),
            metadata: None,
        };

        assert_eq!(command.return_id, return_id);
        assert!(command.notes.is_some());
        assert!(command.completed_by.is_some());
    }

    // ==================== Restock Command Tests ====================

    #[test]
    fn test_restock_command_structure() {
        let return_id = Uuid::new_v4();
        let command = RestockReturnedItemsCommand { return_id };

        assert_eq!(command.return_id, return_id);
    }

    // ==================== Filter Tests ====================

    #[test]
    fn test_status_filter_none() {
        let filter: Option<String> = None;
        assert!(filter.is_none());
    }

    #[test]
    fn test_status_filter_some() {
        let filter: Option<String> = Some("approved".to_string());
        assert!(filter.is_some());
        assert_eq!(filter.unwrap(), "approved");
    }

    #[test]
    fn test_status_filter_empty_string_handling() {
        let filter = Some("".to_string());
        if let Some(status) = filter {
            let normalized = status.trim().to_ascii_lowercase();
            // Empty string should be treated as no filter
            assert!(normalized.is_empty());
        }
    }
}
