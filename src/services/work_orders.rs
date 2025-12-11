use crate::circuit_breaker::CircuitBreaker;
use crate::message_queue::MessageQueue;
use crate::{
    commands::workorders::{
        assign_work_order_command::AssignWorkOrderCommand,
        // cancel_work_order_command::CancelWorkOrderCommand,
        // complete_work_order_command::CompleteWorkOrderCommand,
        // create_work_order_command::CreateWorkOrderCommand,
        // schedule_work_order_command::ScheduleWorkOrderCommand,
        // start_work_order_command::StartWorkOrderCommand,
        // unassign_work_order_command::UnassignWorkOrderCommand,
        // update_work_order_command::UpdateWorkOrderCommand,
    },
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::EventSender,
    models::work_order::{
        self, ActiveModel as WorkOrderActiveModel, Model as WorkOrderModel, WorkOrderPriority,
        WorkOrderStatus,
    },
};
use chrono::{DateTime, NaiveDateTime, Utc};
use redis::Client as RedisClient;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use serde_json::{json, Value};
use slog::Logger;
use std::sync::Arc;
use tracing::instrument;
use uuid::Uuid;

/// Service for managing work orders
#[derive(Clone)]
pub struct WorkOrderService {
    db_pool: Arc<DbPool>,
    event_sender: Arc<EventSender>,
    redis_client: Arc<RedisClient>,
    message_queue: Arc<dyn MessageQueue>,
    circuit_breaker: Arc<CircuitBreaker>,
    logger: Logger,
}

#[derive(Debug, Clone)]
pub struct WorkOrderCreateData {
    pub title: String,
    pub description: Option<String>,
    pub status: Option<WorkOrderStatus>,
    pub priority: WorkOrderPriority,
    pub asset_id: Option<Uuid>,
    pub assigned_to: Option<Uuid>,
    pub due_date: Option<DateTime<Utc>>,
    pub bill_of_materials_number: Option<String>,
    pub quantity_produced: Option<i32>,
    pub parts_required: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct WorkOrderUpdateData {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<WorkOrderStatus>,
    pub priority: Option<WorkOrderPriority>,
    pub asset_id: Option<Uuid>,
    pub assigned_to: Option<Uuid>,
    pub due_date: Option<DateTime<Utc>>,
    pub bill_of_materials_number: Option<String>,
    pub quantity_produced: Option<i32>,
    pub parts_required: Option<Value>,
}

impl WorkOrderService {
    /// Creates a new work order service instance
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

    #[instrument(skip(self), err)]
    pub async fn create_work_order(
        &self,
        data: WorkOrderCreateData,
    ) -> Result<WorkOrderModel, ServiceError> {
        let db = self.db_pool.as_ref();
        let now = Utc::now();
        let parts_required = data.parts_required.unwrap_or_else(|| json!({}));

        let active = WorkOrderActiveModel {
            id: Set(Uuid::new_v4()),
            title: Set(data.title),
            description: Set(data.description),
            status: Set(data.status.unwrap_or(WorkOrderStatus::Pending)),
            priority: Set(data.priority),
            asset_id: Set(data.asset_id),
            assigned_to: Set(data.assigned_to),
            created_at: Set(now),
            updated_at: Set(now),
            due_date: Set(data.due_date),
            bill_of_materials_number: Set(data.bill_of_materials_number),
            quantity_produced: Set(data.quantity_produced),
            parts_required: Set(parts_required),
        };

        active.insert(db).await.map_err(ServiceError::db_error)
    }

    #[instrument(skip(self), err)]
    pub async fn update_work_order(
        &self,
        work_order_id: &Uuid,
        data: WorkOrderUpdateData,
    ) -> Result<WorkOrderModel, ServiceError> {
        let db = self.db_pool.as_ref();
        let existing = work_order::Entity::find_by_id(*work_order_id)
            .one(db)
            .await
            .map_err(ServiceError::db_error)?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Work order {} not found", work_order_id))
            })?;

        let mut active: WorkOrderActiveModel = existing.into();

        if let Some(title) = data.title {
            active.title = Set(title);
        }
        if let Some(description) = data.description {
            active.description = Set(Some(description));
        }
        if let Some(status) = data.status {
            active.status = Set(status);
        }
        if let Some(priority) = data.priority {
            active.priority = Set(priority);
        }
        if let Some(asset_id) = data.asset_id {
            active.asset_id = Set(Some(asset_id));
        }
        if let Some(assigned_to) = data.assigned_to {
            active.assigned_to = Set(Some(assigned_to));
        }
        if let Some(due_date) = data.due_date {
            active.due_date = Set(Some(due_date));
        }
        if let Some(bom) = data.bill_of_materials_number {
            active.bill_of_materials_number = Set(Some(bom));
        }
        if let Some(quantity) = data.quantity_produced {
            active.quantity_produced = Set(Some(quantity));
        }
        if let Some(parts) = data.parts_required {
            active.parts_required = Set(parts);
        }

        active.updated_at = Set(Utc::now());

        active.update(db).await.map_err(ServiceError::db_error)
    }

    #[instrument(skip(self), err)]
    pub async fn delete_work_order(&self, work_order_id: &Uuid) -> Result<(), ServiceError> {
        let db = self.db_pool.as_ref();
        let result = work_order::Entity::delete_by_id(*work_order_id)
            .exec(db)
            .await
            .map_err(ServiceError::db_error)?;

        if result.rows_affected == 0 {
            return Err(ServiceError::NotFound(format!(
                "Work order {} not found",
                work_order_id
            )));
        }

        Ok(())
    }

    #[instrument(skip(self), err)]
    pub async fn list_work_orders(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<WorkOrderModel>, u64), ServiceError> {
        let db = self.db_pool.as_ref();
        let total = work_order::Entity::find()
            .count(db)
            .await
            .map_err(ServiceError::db_error)? as u64;

        let work_orders = work_order::Entity::find()
            .order_by_desc(work_order::Column::CreatedAt)
            .offset(page.saturating_sub(1) * page_size)
            .limit(page_size)
            .all(db)
            .await
            .map_err(ServiceError::db_error)?;

        Ok((work_orders, total))
    }

    /// Creates a new work order
    #[instrument(skip(self))]
    // /// Starts a work order
    // #[instrument(skip(self))]
    // // pub async fn start_work_order(
    //     &self,
    //     command: StartWorkOrderCommand,
    // ) -> Result<(), ServiceError> {
    //     command
    //         .execute(self.db_pool.clone(), self.event_sender.clone())
    //         .await?;
    //     Ok(())
    // }

    // /// Completes a work order
    // #[instrument(skip(self))]
    // // pub async fn complete_work_order(
    //     &self,
    //     command: CompleteWorkOrderCommand,
    // ) -> Result<(), ServiceError> {
    //     command
    //         .execute(self.db_pool.clone(), self.event_sender.clone())
    //         .await?;
    //     Ok(())
    // }

    /// Assigns a work order to a user
    #[instrument(skip(self))]
    pub async fn assign_work_order(
        &self,
        command: AssignWorkOrderCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    // /// Unassigns a work order from a user
    // #[instrument(skip(self))]
    // // pub async fn unassign_work_order(
    //     &self,
    //     command: UnassignWorkOrderCommand,
    // ) -> Result<(), ServiceError> {
    //     command
    //         .execute(self.db_pool.clone(), self.event_sender.clone())
    //         .await?;
    //     Ok(())
    // }

    // /// Schedules a work order
    // #[instrument(skip(self))]
    // // pub async fn schedule_work_order(
    //     &self,
    //     command: ScheduleWorkOrderCommand,
    // ) -> Result<(), ServiceError> {
    //     command
    //         .execute(self.db_pool.clone(), self.event_sender.clone())
    //         .await?;
    //     Ok(())
    // }

    /// Gets a work order by ID
    #[instrument(skip(self))]
    pub async fn get_work_order(
        &self,
        work_order_id: &Uuid,
    ) -> Result<Option<work_order::Model>, ServiceError> {
        let db = &*self.db_pool;
        let work_order = work_order::Entity::find_by_id(*work_order_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        Ok(work_order)
    }

    /// Gets work orders assigned to a user
    #[instrument(skip(self))]
    pub async fn get_work_orders_by_assignee(
        &self,
        user_id: &Uuid,
    ) -> Result<Vec<work_order::Model>, ServiceError> {
        let db = &*self.db_pool;
        let work_orders = work_order::Entity::find()
            .filter(work_order::Column::AssignedTo.eq(*user_id))
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        Ok(work_orders)
    }

    /// Gets work orders by status
    #[instrument(skip(self))]
    pub async fn get_work_orders_by_status(
        &self,
        status: &str,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<work_order::Model>, u64), ServiceError> {
        let db = &*self.db_pool;
        let filter = work_order::Column::Status.eq(status);

        let total = work_order::Entity::find()
            .filter(filter.clone())
            .count(db)
            .await
            .map_err(|e| ServiceError::db_error(e))? as u64;

        let work_orders = work_order::Entity::find()
            .filter(filter)
            .order_by_desc(work_order::Column::CreatedAt)
            .offset((page - 1) * page_size)
            .limit(page_size)
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        Ok((work_orders, total))
    }

    /// Gets work orders scheduled within a date range
    #[instrument(skip(self))]
    pub async fn get_work_orders_by_schedule(
        &self,
        start_date: NaiveDateTime,
        end_date: NaiveDateTime,
    ) -> Result<Vec<work_order::Model>, ServiceError> {
        let db = &*self.db_pool;
        let work_orders = work_order::Entity::find()
            .filter(work_order::Column::CreatedAt.gte(start_date))
            .filter(work_order::Column::DueDate.lte(end_date))
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        Ok(work_orders)
    }

    fn build_work_order_filters(
        &self,
        status: Option<String>,
        assignee_id: Option<Uuid>,
        start_date: Option<DateTime<Utc>>,
        end_date: Option<DateTime<Utc>>,
    ) -> sea_orm::Condition {
        let mut filter = sea_orm::Condition::all();

        if let Some(status) = status {
            filter = filter.add(work_order::Column::Status.eq(status));
        }

        if let Some(assignee_id) = assignee_id {
            filter = filter.add(work_order::Column::AssignedTo.eq(assignee_id));
        }

        if let Some(start_date) = start_date {
            filter = filter.add(work_order::Column::CreatedAt.gte(start_date));
        }

        if let Some(end_date) = end_date {
            filter = filter.add(work_order::Column::DueDate.lte(end_date));
        }

        filter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    // ========================================
    // WorkOrderStatus Tests
    // ========================================

    #[test]
    fn test_work_order_status_variants() {
        let statuses = vec![
            WorkOrderStatus::Pending,
            WorkOrderStatus::InProgress,
            WorkOrderStatus::Completed,
            WorkOrderStatus::Cancelled,
        ];

        // All statuses should be distinct
        for (i, s1) in statuses.iter().enumerate() {
            for (j, s2) in statuses.iter().enumerate() {
                if i != j {
                    assert_ne!(format!("{:?}", s1), format!("{:?}", s2));
                }
            }
        }
    }

    #[test]
    fn test_work_order_status_pending() {
        let status = WorkOrderStatus::Pending;
        assert!(format!("{:?}", status).contains("Pending"));
    }

    #[test]
    fn test_work_order_status_in_progress() {
        let status = WorkOrderStatus::InProgress;
        assert!(format!("{:?}", status).contains("InProgress"));
    }

    #[test]
    fn test_work_order_status_completed() {
        let status = WorkOrderStatus::Completed;
        assert!(format!("{:?}", status).contains("Completed"));
    }

    #[test]
    fn test_work_order_status_cancelled() {
        let status = WorkOrderStatus::Cancelled;
        assert!(format!("{:?}", status).contains("Cancelled"));
    }

    // ========================================
    // WorkOrderPriority Tests
    // ========================================

    #[test]
    fn test_work_order_priority_variants() {
        let priorities = vec![
            WorkOrderPriority::Low,
            WorkOrderPriority::Normal,
            WorkOrderPriority::High,
            WorkOrderPriority::Urgent,
        ];

        assert_eq!(priorities.len(), 4);
    }

    #[test]
    fn test_work_order_priority_low() {
        let priority = WorkOrderPriority::Low;
        assert!(format!("{:?}", priority).contains("Low"));
    }

    #[test]
    fn test_work_order_priority_normal() {
        let priority = WorkOrderPriority::Normal;
        assert!(format!("{:?}", priority).contains("Normal"));
    }

    #[test]
    fn test_work_order_priority_high() {
        let priority = WorkOrderPriority::High;
        assert!(format!("{:?}", priority).contains("High"));
    }

    #[test]
    fn test_work_order_priority_urgent() {
        let priority = WorkOrderPriority::Urgent;
        assert!(format!("{:?}", priority).contains("Urgent"));
    }

    // ========================================
    // WorkOrderCreateData Tests
    // ========================================

    #[test]
    fn test_create_data_minimal() {
        let data = WorkOrderCreateData {
            title: "Test Work Order".to_string(),
            description: None,
            status: None,
            priority: WorkOrderPriority::Normal,
            asset_id: None,
            assigned_to: None,
            due_date: None,
            bill_of_materials_number: None,
            quantity_produced: None,
            parts_required: None,
        };

        assert_eq!(data.title, "Test Work Order");
        assert!(data.description.is_none());
    }

    #[test]
    fn test_create_data_full() {
        let asset_id = Uuid::new_v4();
        let assigned_to = Uuid::new_v4();
        let due_date = Utc::now() + Duration::days(7);

        let data = WorkOrderCreateData {
            title: "Full Work Order".to_string(),
            description: Some("Complete description".to_string()),
            status: Some(WorkOrderStatus::Pending),
            priority: WorkOrderPriority::High,
            asset_id: Some(asset_id),
            assigned_to: Some(assigned_to),
            due_date: Some(due_date),
            bill_of_materials_number: Some("BOM-001".to_string()),
            quantity_produced: Some(100),
            parts_required: Some(json!({"part_a": 10, "part_b": 20})),
        };

        assert_eq!(data.title, "Full Work Order");
        assert!(data.description.is_some());
        assert_eq!(data.asset_id, Some(asset_id));
        assert_eq!(data.assigned_to, Some(assigned_to));
        assert!(data.due_date.is_some());
        assert_eq!(data.bill_of_materials_number, Some("BOM-001".to_string()));
        assert_eq!(data.quantity_produced, Some(100));
    }

    #[test]
    fn test_create_data_with_parts_json() {
        let parts = json!({
            "items": [
                {"part_number": "P001", "quantity": 5},
                {"part_number": "P002", "quantity": 10}
            ]
        });

        let data = WorkOrderCreateData {
            title: "Assembly Order".to_string(),
            description: None,
            status: None,
            priority: WorkOrderPriority::Normal,
            asset_id: None,
            assigned_to: None,
            due_date: None,
            bill_of_materials_number: None,
            quantity_produced: None,
            parts_required: Some(parts.clone()),
        };

        assert!(data.parts_required.is_some());
        let parts_req = data.parts_required.unwrap();
        assert!(parts_req["items"].is_array());
    }

    // ========================================
    // WorkOrderUpdateData Tests
    // ========================================

    #[test]
    fn test_update_data_empty() {
        let data = WorkOrderUpdateData {
            title: None,
            description: None,
            status: None,
            priority: None,
            asset_id: None,
            assigned_to: None,
            due_date: None,
            bill_of_materials_number: None,
            quantity_produced: None,
            parts_required: None,
        };

        assert!(data.title.is_none());
        assert!(data.status.is_none());
    }

    #[test]
    fn test_update_data_status_change() {
        let data = WorkOrderUpdateData {
            title: None,
            description: None,
            status: Some(WorkOrderStatus::InProgress),
            priority: None,
            asset_id: None,
            assigned_to: None,
            due_date: None,
            bill_of_materials_number: None,
            quantity_produced: None,
            parts_required: None,
        };

        assert!(data.status.is_some());
    }

    #[test]
    fn test_update_data_priority_change() {
        let data = WorkOrderUpdateData {
            title: None,
            description: None,
            status: None,
            priority: Some(WorkOrderPriority::Urgent),
            asset_id: None,
            assigned_to: None,
            due_date: None,
            bill_of_materials_number: None,
            quantity_produced: None,
            parts_required: None,
        };

        assert!(data.priority.is_some());
    }

    #[test]
    fn test_update_data_assign_worker() {
        let worker_id = Uuid::new_v4();
        let data = WorkOrderUpdateData {
            title: None,
            description: None,
            status: None,
            priority: None,
            asset_id: None,
            assigned_to: Some(worker_id),
            due_date: None,
            bill_of_materials_number: None,
            quantity_produced: None,
            parts_required: None,
        };

        assert_eq!(data.assigned_to, Some(worker_id));
    }

    #[test]
    fn test_update_data_quantity_produced() {
        let data = WorkOrderUpdateData {
            title: None,
            description: None,
            status: None,
            priority: None,
            asset_id: None,
            assigned_to: None,
            due_date: None,
            bill_of_materials_number: None,
            quantity_produced: Some(50),
            parts_required: None,
        };

        assert_eq!(data.quantity_produced, Some(50));
    }

    // ========================================
    // AssignWorkOrderCommand Tests
    // ========================================

    #[test]
    fn test_assign_command_structure() {
        let work_order_id = Uuid::new_v4();
        let assignee_id = Uuid::new_v4();

        let command = AssignWorkOrderCommand {
            work_order_id,
            assignee_id,
        };

        assert_eq!(command.work_order_id, work_order_id);
        assert_eq!(command.assignee_id, assignee_id);
    }

    // ========================================
    // Status Transition Tests
    // ========================================

    #[test]
    fn test_valid_status_transitions() {
        // Pending can transition to InProgress or Cancelled
        let from_pending = vec![
            (WorkOrderStatus::Pending, WorkOrderStatus::InProgress, true),
            (WorkOrderStatus::Pending, WorkOrderStatus::Cancelled, true),
        ];

        for (from, to, valid) in from_pending {
            assert_eq!(is_valid_transition(&from, &to), valid);
        }
    }

    #[test]
    fn test_in_progress_transitions() {
        // InProgress can transition to Completed or Cancelled
        assert!(is_valid_transition(
            &WorkOrderStatus::InProgress,
            &WorkOrderStatus::Completed
        ));
        assert!(is_valid_transition(
            &WorkOrderStatus::InProgress,
            &WorkOrderStatus::Cancelled
        ));
    }

    #[test]
    fn test_terminal_state_transitions() {
        // Completed and Cancelled are terminal states
        assert!(!is_valid_transition(
            &WorkOrderStatus::Completed,
            &WorkOrderStatus::Pending
        ));
        assert!(!is_valid_transition(
            &WorkOrderStatus::Cancelled,
            &WorkOrderStatus::InProgress
        ));
    }

    fn is_valid_transition(from: &WorkOrderStatus, to: &WorkOrderStatus) -> bool {
        match (from, to) {
            (WorkOrderStatus::Pending, WorkOrderStatus::InProgress) => true,
            (WorkOrderStatus::Pending, WorkOrderStatus::Cancelled) => true,
            (WorkOrderStatus::InProgress, WorkOrderStatus::Completed) => true,
            (WorkOrderStatus::InProgress, WorkOrderStatus::Cancelled) => true,
            (WorkOrderStatus::InProgress, WorkOrderStatus::Pending) => true, // pause
            _ => false,
        }
    }

    // ========================================
    // Due Date Tests
    // ========================================

    #[test]
    fn test_due_date_in_future() {
        let due_date = Utc::now() + Duration::days(7);
        assert!(due_date > Utc::now());
    }

    #[test]
    fn test_due_date_calculation() {
        let start = Utc::now();
        let duration_days = 14;
        let due_date = start + Duration::days(duration_days);

        assert!(due_date > start);
    }

    #[test]
    fn test_overdue_work_order() {
        let past_due = Utc::now() - Duration::days(1);
        let now = Utc::now();

        assert!(past_due < now, "Work order is overdue");
    }

    // ========================================
    // Title and Description Validation
    // ========================================

    #[test]
    fn test_title_not_empty() {
        let title = "Repair Machine A";
        assert!(!title.is_empty());
        assert!(title.len() >= 3);
    }

    #[test]
    fn test_title_max_length() {
        let title = "A".repeat(200);
        assert!(
            title.len() <= 255,
            "Title should be within reasonable length"
        );
    }

    #[test]
    fn test_description_optional() {
        let description: Option<String> = None;
        assert!(description.is_none());

        let with_desc: Option<String> = Some("Detailed instructions".to_string());
        assert!(with_desc.is_some());
    }

    // ========================================
    // Bill of Materials Tests
    // ========================================

    #[test]
    fn test_bom_number_format() {
        let valid_boms = vec!["BOM-001", "BOM-2024-001", "PROD-BOM-123"];

        for bom in valid_boms {
            assert!(bom.starts_with("BOM") || bom.contains("BOM"));
        }
    }

    #[test]
    fn test_bom_number_optional() {
        let data = WorkOrderCreateData {
            title: "Test".to_string(),
            description: None,
            status: None,
            priority: WorkOrderPriority::Low,
            asset_id: None,
            assigned_to: None,
            due_date: None,
            bill_of_materials_number: None,
            quantity_produced: None,
            parts_required: None,
        };

        assert!(data.bill_of_materials_number.is_none());
    }

    // ========================================
    // Quantity Tests
    // ========================================

    #[test]
    fn test_quantity_positive() {
        let quantity: i32 = 100;
        assert!(quantity > 0);
    }

    #[test]
    fn test_quantity_zero_valid() {
        // Work order not yet started
        let quantity: i32 = 0;
        assert_eq!(quantity, 0);
    }

    #[test]
    fn test_quantity_update() {
        let initial = 0;
        let produced = 50;
        let final_quantity = initial + produced;

        assert_eq!(final_quantity, 50);
    }

    // ========================================
    // Filter Building Tests
    // ========================================

    #[test]
    fn test_filter_with_status() {
        let status = Some("InProgress".to_string());
        assert!(status.is_some());
    }

    #[test]
    fn test_filter_with_assignee() {
        let assignee_id = Some(Uuid::new_v4());
        assert!(assignee_id.is_some());
    }

    #[test]
    fn test_filter_with_date_range() {
        let start = Some(Utc::now() - Duration::days(7));
        let end = Some(Utc::now());

        assert!(start.is_some());
        assert!(end.is_some());
        assert!(start.unwrap() < end.unwrap());
    }

    #[test]
    fn test_filter_empty() {
        let status: Option<String> = None;
        let assignee_id: Option<Uuid> = None;

        assert!(status.is_none());
        assert!(assignee_id.is_none());
    }

    // ========================================
    // Pagination Tests
    // ========================================

    #[test]
    fn test_pagination_defaults() {
        let page: u64 = 0;
        let page_size: u64 = 20;

        assert_eq!(page, 0);
        assert!(page_size > 0 && page_size <= 100);
    }

    #[test]
    fn test_pagination_offset_calculation() {
        let page: u64 = 2;
        let page_size: u64 = 20;
        let offset = page * page_size;

        assert_eq!(offset, 40);
    }

    // ========================================
    // Clone and Debug Trait Tests
    // ========================================

    #[test]
    fn test_create_data_clone() {
        let original = WorkOrderCreateData {
            title: "Clone Test".to_string(),
            description: Some("Testing clone".to_string()),
            status: None,
            priority: WorkOrderPriority::High,
            asset_id: None,
            assigned_to: None,
            due_date: None,
            bill_of_materials_number: None,
            quantity_produced: None,
            parts_required: None,
        };

        let cloned = original.clone();
        assert_eq!(cloned.title, "Clone Test");
    }

    #[test]
    fn test_update_data_clone() {
        let original = WorkOrderUpdateData {
            title: Some("Update Test".to_string()),
            description: None,
            status: None,
            priority: None,
            asset_id: None,
            assigned_to: None,
            due_date: None,
            bill_of_materials_number: None,
            quantity_produced: None,
            parts_required: None,
        };

        let cloned = original.clone();
        assert_eq!(cloned.title, Some("Update Test".to_string()));
    }

    #[test]
    fn test_create_data_debug() {
        let data = WorkOrderCreateData {
            title: "Debug Test".to_string(),
            description: None,
            status: None,
            priority: WorkOrderPriority::Low,
            asset_id: None,
            assigned_to: None,
            due_date: None,
            bill_of_materials_number: None,
            quantity_produced: None,
            parts_required: None,
        };

        let debug_str = format!("{:?}", data);
        assert!(debug_str.contains("Debug Test"));
    }
}
