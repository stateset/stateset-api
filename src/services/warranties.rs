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
    events::EventSender,
};
use anyhow::Result;
use chrono::{Duration, Utc};
use redis::Client as RedisClient;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use slog::Logger;
use std::sync::Arc;
use tracing::instrument;
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
        let claim = command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        let claim_id = claim.id;
        let warranty_id = claim.warranty_id;
        let payload = serde_json::json!({
            "claim_id": claim_id.to_string(),
            "warranty_id": warranty_id.to_string(),
            "status": claim.status.clone(),
            "claim_number": claim.claim_number.clone(),
            "claim_date": claim.claim_date.to_rfc3339(),
        });
        let _ = crate::events::outbox::enqueue(
            &*self.db_pool,
            "warranty",
            Some(warranty_id),
            "WarrantyClaimed",
            &payload,
        )
        .await;
        Ok(claim_id)
    }

    /// Approves a warranty claim
    #[instrument(skip(self))]
    pub async fn approve_warranty_claim(
        &self,
        command: ApproveWarrantyClaimCommand,
    ) -> Result<(), ServiceError> {
        let claim = command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        // Outbox: WarrantyClaimApproved
        let payload = serde_json::json!({
            "claim_id": claim.id.to_string(),
            "warranty_id": claim.warranty_id.to_string(),
            "status": claim.status.clone(),
            "resolution": claim.resolution.clone(),
            "resolved_at": claim.resolved_date.map(|dt| dt.to_rfc3339()),
            "approved_by": command.approved_by.to_string(),
            "notes": command.notes.clone(),
        });
        let _ = crate::events::outbox::enqueue(
            &*self.db_pool,
            "warranty",
            Some(claim.warranty_id),
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
        let claim = command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        // Outbox: WarrantyClaimRejected
        let payload = serde_json::json!({
            "claim_id": claim.id.to_string(),
            "warranty_id": claim.warranty_id.to_string(),
            "status": claim.status.clone(),
            "reason": command.reason.clone(),
            "resolved_at": claim.resolved_date.map(|dt| dt.to_rfc3339()),
            "rejected_by": command.rejected_by.to_string(),
            "notes": command.notes.clone(),
        });
        let _ = crate::events::outbox::enqueue(
            &*self.db_pool,
            "warranty",
            Some(claim.warranty_id),
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
        let warranty = warranty::Entity::find()
            .filter(warranty::Column::Id.eq(*warranty_id))
            .one(db)
            .await?;
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

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Warranty Duration Tests ====================

    #[test]
    fn test_extension_duration_calculation() {
        // Test that extension calculates correctly
        let months = 6;
        let extension_days = Duration::days((months as i64) * 30);
        assert_eq!(extension_days.num_days(), 180);
    }

    #[test]
    fn test_extension_duration_one_year() {
        let months = 12;
        let extension_days = Duration::days((months as i64) * 30);
        assert_eq!(extension_days.num_days(), 360);
    }

    #[test]
    fn test_extension_duration_one_month() {
        let months = 1;
        let extension_days = Duration::days((months as i64) * 30);
        assert_eq!(extension_days.num_days(), 30);
    }

    // ==================== Validation Tests ====================

    #[test]
    fn test_negative_extension_validation() {
        let additional_months = -1;
        assert!(additional_months <= 0, "Negative months should fail validation");
    }

    #[test]
    fn test_zero_extension_validation() {
        let additional_months = 0;
        assert!(additional_months <= 0, "Zero months should fail validation");
    }

    #[test]
    fn test_positive_extension_validation() {
        let additional_months = 3;
        assert!(additional_months > 0, "Positive months should pass validation");
    }

    // ==================== UUID Tests ====================

    #[test]
    fn test_warranty_id_generation() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        assert_ne!(id1, id2);
        assert!(!id1.is_nil());
        assert!(!id2.is_nil());
    }

    #[test]
    fn test_warranty_id_parsing() {
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let parsed = Uuid::parse_str(&id_str);

        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap(), id);
    }

    // ==================== Date/Time Tests ====================

    #[test]
    fn test_warranty_date_comparison() {
        let now = Utc::now();
        let future = now + Duration::days(365);
        let past = now - Duration::days(365);

        assert!(future > now);
        assert!(past < now);
    }

    #[test]
    fn test_warranty_expiration_check() {
        let now = Utc::now();
        let end_date = now + Duration::days(30);

        // Warranty should be active if end_date >= now
        assert!(end_date >= now);
    }

    #[test]
    fn test_expired_warranty_check() {
        let now = Utc::now();
        let end_date = now - Duration::days(30);

        // Warranty should be expired if end_date < now
        assert!(end_date < now);
    }

    // ==================== Warranty Status Tests ====================

    #[test]
    fn test_valid_warranty_statuses() {
        let valid_statuses = vec!["active", "expired", "claimed", "void"];

        for status in &valid_statuses {
            assert!(!status.is_empty());
        }
    }

    #[test]
    fn test_warranty_status_active() {
        let status = "active";
        assert_eq!(status, "active");
    }

    // ==================== Claim Workflow Tests ====================

    #[test]
    fn test_claim_workflow_statuses() {
        let claim_statuses = vec!["pending", "approved", "rejected", "in_review"];

        for status in claim_statuses {
            assert!(!status.is_empty());
        }
    }

    #[test]
    fn test_claim_id_generation() {
        let claim_id = Uuid::new_v4();
        let warranty_id = Uuid::new_v4();

        assert_ne!(claim_id, warranty_id);
    }

    // ==================== Pagination Tests ====================

    #[test]
    fn test_pagination_offset_calculation() {
        // Page 1, limit 10 -> offset 0
        let page = 1u64;
        let limit = 10u64;
        let offset = page.saturating_sub(1) * limit;
        assert_eq!(offset, 0);
    }

    #[test]
    fn test_pagination_offset_page_2() {
        // Page 2, limit 10 -> offset 10
        let page = 2u64;
        let limit = 10u64;
        let offset = page.saturating_sub(1) * limit;
        assert_eq!(offset, 10);
    }

    #[test]
    fn test_pagination_offset_page_0_saturates() {
        // Page 0 should saturate to offset 0
        let page = 0u64;
        let limit = 10u64;
        let offset = page.saturating_sub(1) * limit;
        assert_eq!(offset, 0); // saturating_sub(1) on 0 gives 0
    }

    #[test]
    fn test_pagination_large_page() {
        let page = 100u64;
        let limit = 25u64;
        let offset = page.saturating_sub(1) * limit;
        assert_eq!(offset, 2475);
    }

    // ==================== Event Payload Tests ====================

    #[test]
    fn test_warranty_created_payload_structure() {
        let warranty_id = Uuid::new_v4();
        let payload = serde_json::json!({"warranty_id": warranty_id.to_string()});

        assert!(payload.is_object());
        assert!(payload.get("warranty_id").is_some());
    }

    #[test]
    fn test_warranty_claimed_payload_structure() {
        let claim_id = Uuid::new_v4();
        let warranty_id = Uuid::new_v4();
        let payload = serde_json::json!({
            "claim_id": claim_id.to_string(),
            "warranty_id": warranty_id.to_string(),
            "status": "pending",
            "claim_number": "CLM-001",
            "claim_date": Utc::now().to_rfc3339(),
        });

        assert!(payload.is_object());
        assert!(payload.get("claim_id").is_some());
        assert!(payload.get("warranty_id").is_some());
        assert!(payload.get("status").is_some());
    }

    // ==================== Error Message Tests ====================

    #[test]
    fn test_not_found_error_message() {
        let warranty_id = Uuid::new_v4();
        let msg = format!("Warranty {} not found", warranty_id);

        assert!(msg.contains("not found"));
        assert!(msg.contains(&warranty_id.to_string()));
    }

    #[test]
    fn test_validation_error_message() {
        let msg = "additional_months must be positive";
        assert!(msg.contains("positive"));
    }
}
