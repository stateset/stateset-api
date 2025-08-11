use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    entities::order::{self, Entity as OrderEntity, Model as OrderModel, ActiveModel as OrderActiveModel},
    entities::order_item::{self, Entity as OrderItemEntity, Model as OrderItemModel},
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    DbErr, ActiveValue, QueryOrder, PaginatorTrait, TransactionTrait
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;
use validator::Validate;

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
        request.validate().map_err(|e| ServiceError::ValidationError(e.to_string()))?;

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
            status: Set("pending".to_string()),
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
        request.validate().map_err(|e| ServiceError::ValidationError(e.to_string()))?;

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
        let cancel_request = UpdateOrderStatusRequest {
            status: "cancelled".to_string(),
            notes: reason,
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