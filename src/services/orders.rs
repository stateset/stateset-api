use std::sync::Arc;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, 
    QueryFilter, QueryOrder, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};
use uuid::Uuid;
use validator::Validate;

use crate::{
    db::DbPool,
    entities::order::{self, ActiveModel as OrderActiveModel, Entity as OrderEntity, Model as OrderModel},
    entities::order_item::{Entity as OrderItemEntity, Model as OrderItemModel},
    errors::ServiceError,
    events::{Event, EventSender},
};

/// Request/Response types for the order service
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateOrderRequest {
    pub customer_id: Uuid,
    #[validate(length(min = 1, message = "Order number is required"))]
    pub order_number: String,
    pub total_amount: Decimal,
    #[validate(length(min = 3, max = 3, message = "Currency must be 3 characters"))]
    pub currency: String,
    pub payment_status: String,
    pub fulfillment_status: String,
    pub payment_method: Option<String>,
    pub shipping_method: Option<String>,
    pub notes: Option<String>,
    pub shipping_address: Option<String>,
    pub billing_address: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateOrderStatusRequest {
    pub status: String,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderResponse {
    pub id: Uuid,
    pub order_number: String,
    pub customer_id: Uuid,
    pub status: String,
    pub order_date: DateTime<Utc>,
    pub total_amount: Decimal,
    pub currency: String,
    pub payment_status: String,
    pub fulfillment_status: String,
    pub payment_method: Option<String>,
    pub shipping_method: Option<String>,
    pub tracking_number: Option<String>,
    pub notes: Option<String>,
    pub shipping_address: Option<String>,
    pub billing_address: Option<String>,
    pub is_archived: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub version: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderListResponse {
    pub orders: Vec<OrderResponse>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

/// Service for managing orders with PostgreSQL database operations
#[derive(Clone)]
pub struct OrderService {
    db_pool: Arc<DbPool>,
    event_sender: Option<Arc<EventSender>>,
}

// Order status constants
const STATUS_PENDING: &str = "pending";
const STATUS_PROCESSING: &str = "processing";
const STATUS_SHIPPED: &str = "shipped";
const STATUS_DELIVERED: &str = "delivered";
const STATUS_CANCELLED: &str = "cancelled";
const STATUS_REFUNDED: &str = "refunded";

impl OrderService {
    /// Creates a new order service instance
    pub fn new(db_pool: Arc<DbPool>, event_sender: Option<Arc<EventSender>>) -> Self {
        Self {
            db_pool,
            event_sender,
        }
    }

    /// Creates a new order in the database
    #[instrument(skip(self, request), fields(customer_id = %request.customer_id, order_number = %request.order_number))]
    pub async fn create_order(&self, request: CreateOrderRequest) -> Result<OrderResponse, ServiceError> {
        // Validate the request
        request.validate()
            .map_err(|e| ServiceError::ValidationError(format!("Invalid order data: {}", e)))?;
        
        // Additional business validations
        if request.total_amount < Decimal::ZERO {
            return Err(ServiceError::ValidationError(
                "Total amount cannot be negative".to_string()
            ));
        }

        let db = &*self.db_pool;
        let now = Utc::now();
        let order_id = Uuid::new_v4();

        // Start a database transaction
        let txn = db.begin().await.map_err(|e| {
            error!(error = %e, "Failed to start transaction for order creation");
            ServiceError::DatabaseError(e.into())
        })?;

        // Create the order active model
        let order_active_model = OrderActiveModel {
            id: Set(order_id),
            order_number: Set(request.order_number.clone()),
            customer_id: Set(request.customer_id),
            status: Set(STATUS_PENDING.to_string()),
            order_date: Set(now),
            total_amount: Set(request.total_amount),
            currency: Set(request.currency),
            payment_status: Set(request.payment_status),
            fulfillment_status: Set(request.fulfillment_status),
            payment_method: Set(request.payment_method),
            shipping_method: Set(request.shipping_method),
            tracking_number: Set(None),
            notes: Set(request.notes),
            shipping_address: Set(request.shipping_address),
            billing_address: Set(request.billing_address),
            is_archived: Set(false),
            created_at: Set(now),
            updated_at: Set(Some(now)),
            version: Set(1),
        };

        // Insert the order
        let order_model = order_active_model.insert(&txn).await.map_err(|e| {
            error!(error = %e, order_id = %order_id, "Failed to create order in database");
            ServiceError::DatabaseError(e.into())
        })?;

        // Commit the transaction
        txn.commit().await.map_err(|e| {
            error!(error = %e, order_id = %order_id, "Failed to commit order creation transaction");
            ServiceError::DatabaseError(e.into())
        })?;

        info!(order_id = %order_id, customer_id = %request.customer_id, "Order created successfully");

        // Send event if event sender is available
        if let Some(event_sender) = &self.event_sender {
            if let Err(e) = event_sender.send(Event::OrderCreated(order_id)).await {
                warn!(error = %e, order_id = %order_id, "Failed to send order created event");
            }
        }

        // Convert to response
        Ok(self.model_to_response(order_model))
    }

    /// Retrieves an order by ID
    #[instrument(skip(self), fields(order_id = %order_id))]
    pub async fn get_order(&self, order_id: Uuid) -> Result<Option<OrderResponse>, ServiceError> {
        let db = &*self.db_pool;

        let order = OrderEntity::find_by_id(order_id)
            .one(db)
            .await
            .map_err(|e| {
                error!(error = %e, order_id = %order_id, "Failed to fetch order from database");
                ServiceError::DatabaseError(e.into())
            })?;

        match order {
            Some(order_model) => {
                info!(order_id = %order_id, "Order retrieved successfully");
                Ok(Some(self.model_to_response(order_model)))
            }
            None => {
                info!(order_id = %order_id, "Order not found");
                Ok(None)
            }
        }
    }

    /// Lists orders with pagination
    #[instrument(skip(self))]
    pub async fn list_orders(&self, page: u64, per_page: u64) -> Result<OrderListResponse, ServiceError> {
        // Validate pagination parameters
        if page == 0 {
            return Err(ServiceError::ValidationError(
                "Page number must be greater than 0".to_string()
            ));
        }
        
        if per_page == 0 || per_page > 100 {
            return Err(ServiceError::ValidationError(
                "Per page must be between 1 and 100".to_string()
            ));
        }
        
        let db = &*self.db_pool;

        // Get paginated orders
        let paginator = OrderEntity::find()
            .filter(order::Column::IsArchived.eq(false))
            .order_by_desc(order::Column::CreatedAt)
            .paginate(db, per_page);

        let total = paginator.num_items().await.map_err(|e| {
            error!(error = %e, "Failed to count orders");
            ServiceError::DatabaseError(e.into())
        })?;

        let orders = paginator.fetch_page(page - 1).await.map_err(|e| {
            error!(error = %e, page = page, per_page = per_page, "Failed to fetch orders page");
            ServiceError::DatabaseError(e.into())
        })?;

        let order_responses: Vec<OrderResponse> = orders
            .into_iter()
            .map(|order| self.model_to_response(order))
            .collect();

        info!(total = total, page = page, per_page = per_page, returned_count = order_responses.len(), "Orders listed successfully");

        Ok(OrderListResponse {
            orders: order_responses,
            total,
            page,
            per_page,
        })
    }

    /// Updates an order's status
    #[instrument(skip(self, request), fields(order_id = %order_id, new_status = %request.status))]
    pub async fn update_order_status(&self, order_id: Uuid, request: UpdateOrderStatusRequest) -> Result<OrderResponse, ServiceError> {
        request.validate()
            .map_err(|e| ServiceError::ValidationError(format!("Invalid status update: {}", e)))?;
        
        // Validate status transition
        let valid_statuses = vec![
            STATUS_PENDING, STATUS_PROCESSING, STATUS_SHIPPED, 
            STATUS_DELIVERED, STATUS_CANCELLED, STATUS_REFUNDED
        ];
        
        if !valid_statuses.contains(&request.status.as_str()) {
            return Err(ServiceError::ValidationError(
                format!("Invalid status: {}. Must be one of: {:?}", request.status, valid_statuses)
            ));
        }

        let db = &*self.db_pool;
        let now = Utc::now();

        // Start transaction
        let txn = db.begin().await.map_err(|e| {
            error!(error = %e, order_id = %order_id, "Failed to start transaction for status update");
            ServiceError::DatabaseError(e.into())
        })?;

        // Find the order
        let order = OrderEntity::find_by_id(order_id)
            .one(&txn)
            .await
            .map_err(|e| {
                error!(error = %e, order_id = %order_id, "Failed to find order for status update");
                ServiceError::DatabaseError(e.into())
            })?;

        let order = order.ok_or_else(|| {
            warn!(order_id = %order_id, "Order not found for status update");
            ServiceError::NotFound("Order not found".to_string())
        })?;

        let old_status = order.status.clone();
        
        // Validate status transition rules
        if !self.is_valid_status_transition(&old_status, &request.status) {
            return Err(ServiceError::ValidationError(
                format!("Invalid status transition from {} to {}", old_status, request.status)
            ));
        }

        // Update the order
        let mut order_active_model: OrderActiveModel = order.into();
        order_active_model.status = Set(request.status.clone());
        order_active_model.updated_at = Set(Some(now));
        order_active_model.version = Set(order_active_model.version.unwrap() + 1);

        if let Some(notes) = request.notes {
            order_active_model.notes = Set(Some(notes));
        }

        let updated_order = order_active_model.update(&txn).await.map_err(|e| {
            error!(error = %e, order_id = %order_id, "Failed to update order status");
            ServiceError::DatabaseError(e.into())
        })?;

        // Commit transaction
        txn.commit().await.map_err(|e| {
            error!(error = %e, order_id = %order_id, "Failed to commit status update transaction");
            ServiceError::DatabaseError(e.into())
        })?;

        info!(order_id = %order_id, old_status = %old_status, new_status = %request.status, "Order status updated successfully");

        // Send event if event sender is available
        if let Some(event_sender) = &self.event_sender {
            if let Err(e) = event_sender.send(Event::OrderStatusChanged {
                order_id,
                old_status,
                new_status: request.status.clone(),
            }).await {
                warn!(error = %e, order_id = %order_id, "Failed to send order status changed event");
            }
        }

        Ok(self.model_to_response(updated_order))
    }

    /// Cancels an order
    #[instrument(skip(self), fields(order_id = %order_id))]
    pub async fn cancel_order(&self, order_id: Uuid, reason: Option<String>) -> Result<OrderResponse, ServiceError> {
        // First check if the order can be cancelled
        let order = self.get_order(order_id).await?
            .ok_or_else(|| ServiceError::NotFound("Order not found".to_string()))?;
        
        if order.status == STATUS_DELIVERED || order.status == STATUS_CANCELLED {
            return Err(ServiceError::ValidationError(
                format!("Cannot cancel order with status: {}", order.status)
            ));
        }
        
        let cancel_request = UpdateOrderStatusRequest {
            status: STATUS_CANCELLED.to_string(),
            notes: reason.or(Some("Order cancelled by user".to_string())),
        };

        let response = self.update_order_status(order_id, cancel_request).await?;

        // Send specific cancel event
        if let Some(event_sender) = &self.event_sender {
            if let Err(e) = event_sender.send(Event::OrderCancelled(order_id)).await {
                warn!(error = %e, order_id = %order_id, "Failed to send order cancelled event");
            }
        }

        Ok(response)
    }

    /// Archives an order
    #[instrument(skip(self), fields(order_id = %order_id))]
    pub async fn archive_order(&self, order_id: Uuid) -> Result<OrderResponse, ServiceError> {
        let db = &*self.db_pool;
        let now = Utc::now();

        // Find and update the order
        let order = OrderEntity::find_by_id(order_id)
            .one(db)
            .await
            .map_err(|e| {
                error!(error = %e, order_id = %order_id, "Failed to find order for archiving");
                ServiceError::DatabaseError(e.into())
            })?;

        let order = order.ok_or_else(|| {
            warn!(order_id = %order_id, "Order not found for archiving");
            ServiceError::NotFound("Order not found".to_string())
        })?;

        let mut order_active_model: OrderActiveModel = order.into();
        order_active_model.is_archived = Set(true);
        order_active_model.updated_at = Set(Some(now));
        order_active_model.version = Set(order_active_model.version.unwrap() + 1);

        let archived_order = order_active_model.update(db).await.map_err(|e| {
            error!(error = %e, order_id = %order_id, "Failed to archive order");
            ServiceError::DatabaseError(e.into())
        })?;

        info!(order_id = %order_id, "Order archived successfully");

        Ok(self.model_to_response(archived_order))
    }

    /// Validates if a status transition is allowed
    fn is_valid_status_transition(&self, from: &str, to: &str) -> bool {
        match (from, to) {
            // From pending, can go to processing, cancelled
            (STATUS_PENDING, STATUS_PROCESSING) | (STATUS_PENDING, STATUS_CANCELLED) => true,
            // From processing, can go to shipped, cancelled
            (STATUS_PROCESSING, STATUS_SHIPPED) | (STATUS_PROCESSING, STATUS_CANCELLED) => true,
            // From shipped, can go to delivered
            (STATUS_SHIPPED, STATUS_DELIVERED) => true,
            // From delivered, can go to refunded
            (STATUS_DELIVERED, STATUS_REFUNDED) => true,
            // From cancelled, can go to refunded
            (STATUS_CANCELLED, STATUS_REFUNDED) => true,
            // All other transitions are invalid
            _ => false,
        }
    }
    
    /// Converts an order model to response format
    fn model_to_response(&self, model: OrderModel) -> OrderResponse {
        OrderResponse {
            id: model.id,
            order_number: model.order_number,
            customer_id: model.customer_id,
            status: model.status,
            order_date: model.order_date,
            total_amount: model.total_amount,
            currency: model.currency,
            payment_status: model.payment_status,
            fulfillment_status: model.fulfillment_status,
            payment_method: model.payment_method,
            shipping_method: model.shipping_method,
            tracking_number: model.tracking_number,
            notes: model.notes,
            shipping_address: model.shipping_address,
            billing_address: model.billing_address,
            is_archived: model.is_archived,
            created_at: model.created_at,
            updated_at: model.updated_at,
            version: model.version,
        }
    }
    
    /// Deletes an order (soft delete by archiving)
    #[instrument(skip(self), fields(order_id = %order_id))]
    pub async fn delete_order(&self, order_id: Uuid) -> Result<(), ServiceError> {
        // Check if order exists and can be deleted
        let order = self.get_order(order_id).await?
            .ok_or_else(|| ServiceError::NotFound("Order not found".to_string()))?;
        
        // Only allow deletion of cancelled or draft orders
        if order.status != STATUS_CANCELLED && order.status != STATUS_PENDING {
            return Err(ServiceError::ValidationError(
                format!("Cannot delete order with status: {}", order.status)
            ));
        }
        
        self.archive_order(order_id).await?;
        
        info!(order_id = %order_id, "Order deleted (archived) successfully");
        Ok(())
    }
    
    /// Search orders by various criteria
    #[instrument(skip(self))]
    pub async fn search_orders(
        &self,
        customer_id: Option<Uuid>,
        status: Option<String>,
        from_date: Option<DateTime<Utc>>,
        to_date: Option<DateTime<Utc>>,
        page: u64,
        per_page: u64,
    ) -> Result<OrderListResponse, ServiceError> {
        // Validate pagination
        if page == 0 || per_page == 0 || per_page > 100 {
            return Err(ServiceError::ValidationError(
                "Invalid pagination parameters".to_string()
            ));
        }
        
        let db = &*self.db_pool;
        let mut query = OrderEntity::find();
        
        // Apply filters
        query = query.filter(order::Column::IsArchived.eq(false));
        
        if let Some(cid) = customer_id {
            query = query.filter(order::Column::CustomerId.eq(cid));
        }
        
        if let Some(s) = status {
            query = query.filter(order::Column::Status.eq(s));
        }
        
        if let Some(from) = from_date {
            query = query.filter(order::Column::OrderDate.gte(from));
        }
        
        if let Some(to) = to_date {
            query = query.filter(order::Column::OrderDate.lte(to));
        }
        
        // Get results with pagination
        let paginator = query
            .order_by_desc(order::Column::CreatedAt)
            .paginate(db, per_page);
        
        let total = paginator.num_items().await
            .map_err(|e| ServiceError::DatabaseError(e.into()))?;
        
        let orders = paginator.fetch_page(page - 1).await
            .map_err(|e| ServiceError::DatabaseError(e.into()))?;
        
        let order_responses: Vec<OrderResponse> = orders
            .into_iter()
            .map(|order| self.model_to_response(order))
            .collect();
        
        Ok(OrderListResponse {
            orders: order_responses,
            total,
            page,
            per_page,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_model_to_response_conversion() {
        let now = Utc::now();
        let order_id = Uuid::new_v4();
        let customer_id = Uuid::new_v4();

        let model = OrderModel {
            id: order_id,
            order_number: "ORD-001".to_string(),
            customer_id,
            status: "pending".to_string(),
            order_date: now,
            total_amount: Decimal::from_str("99.99").unwrap(),
            currency: "USD".to_string(),
            payment_status: "pending".to_string(),
            fulfillment_status: "unfulfilled".to_string(),
            payment_method: Some("credit_card".to_string()),
            shipping_method: Some("standard".to_string()),
            tracking_number: None,
            notes: Some("Test order".to_string()),
            shipping_address: Some("123 Main St".to_string()),
            billing_address: Some("123 Main St".to_string()),
            is_archived: false,
            created_at: now,
            updated_at: Some(now),
            version: 1,
        };

        let db_pool = Arc::new(DatabaseConnection::Disconnected);
        let service = OrderService::new(db_pool, None);
        let response = service.model_to_response(model);

        assert_eq!(response.id, order_id);
        assert_eq!(response.customer_id, customer_id);
        assert_eq!(response.order_number, "ORD-001");
        assert_eq!(response.status, "pending");
        assert_eq!(response.total_amount, Decimal::from_str("99.99").unwrap());
    }
}