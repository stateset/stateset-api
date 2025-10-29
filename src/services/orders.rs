use std::{collections::HashMap, sync::Arc};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::{
    db::DbPool,
    entities::order::{
        self, ActiveModel as OrderActiveModel, Entity as OrderEntity, Model as OrderModel,
    },
    entities::order_item::{
        ActiveModel as OrderItemActiveModel, Column as OrderItemColumn, Entity as OrderItemEntity,
        Model as OrderItemModel,
    },
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

/// Lightweight representation of an order item used when creating an order transactionally.
#[derive(Debug, Clone)]
pub struct NewOrderItemInput {
    pub sku: String,
    pub product_id: Option<Uuid>,
    pub name: Option<String>,
    pub quantity: i32,
    pub unit_price: Decimal,
    pub tax_rate: Option<Decimal>,
}

/// Parameters required to atomically create an order and its items.
#[derive(Debug, Clone)]
pub struct CreateOrderWithItemsInput {
    pub customer_id: Uuid,
    pub total_amount: Decimal,
    pub currency: String,
    pub payment_status: String,
    pub fulfillment_status: String,
    pub payment_method: Option<String>,
    pub shipping_method: Option<String>,
    pub shipping_address: Option<String>,
    pub billing_address: Option<String>,
    pub notes: Option<String>,
    pub items: Vec<NewOrderItemInput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrderSortField {
    CreatedAt,
    OrderDate,
    TotalAmount,
    OrderNumber,
}

impl Default for OrderSortField {
    fn default() -> Self {
        OrderSortField::CreatedAt
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    Asc,
    Desc,
}

impl Default for SortDirection {
    fn default() -> Self {
        SortDirection::Desc
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderSearchQuery {
    pub customer_id: Option<Uuid>,
    pub status: Option<String>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub search: Option<String>,
    #[serde(default)]
    pub sort_field: OrderSortField,
    #[serde(default)]
    pub sort_direction: SortDirection,
    pub page: u64,
    pub per_page: u64,
}

/// Service for managing orders with PostgreSQL database operations
#[derive(Debug, Default)]
pub struct UpdateOrderDetails {
    pub shipping_address: Option<String>,
    pub billing_address: Option<String>,
    pub payment_method: Option<String>,
    pub notes: Option<String>,
}

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

    /// Creates a minimal order with sensible defaults. Does not insert items.
    #[instrument(skip(self), fields(customer_id = %customer_id))]
    pub async fn create_order_minimal(
        &self,
        customer_id: Uuid,
        total_amount: Decimal,
        currency: Option<String>,
        notes: Option<String>,
        shipping_address: Option<String>,
        billing_address: Option<String>,
        payment_method: Option<String>,
    ) -> Result<OrderResponse, ServiceError> {
        let db = &*self.db_pool;
        let now = Utc::now();
        let order_id = Uuid::new_v4();

        let order_active = OrderActiveModel {
            id: Set(order_id),
            order_number: Set(format!("ORD-{}", now.timestamp_millis())),
            customer_id: Set(customer_id),
            status: Set(STATUS_PENDING.to_string()),
            order_date: Set(now),
            total_amount: Set(total_amount),
            currency: Set(currency.unwrap_or_else(|| "USD".to_string())),
            payment_status: Set("pending".to_string()),
            fulfillment_status: Set("unfulfilled".to_string()),
            payment_method: Set(payment_method),
            shipping_method: Set(None),
            tracking_number: Set(None),
            notes: Set(notes),
            shipping_address: Set(shipping_address),
            billing_address: Set(billing_address),
            is_archived: Set(false),
            created_at: Set(now),
            updated_at: Set(Some(now)),
            version: Set(1),
        };

        let model = order_active.insert(db).await.map_err(|e| {
            error!(error = %e, order_id = %order_id, "Failed to create minimal order");
            ServiceError::db_error(e)
        })?;

        if let Some(event_sender) = &self.event_sender {
            event_sender
                .send_or_log(Event::OrderCreated(order_id))
                .await;
        }

        Ok(self.model_to_response(model))
    }

    /// Creates an order together with the provided items inside a single transaction.
    #[instrument(skip(self, input), fields(customer_id = %input.customer_id))]
    pub async fn create_order_with_items(
        &self,
        mut input: CreateOrderWithItemsInput,
    ) -> Result<(OrderResponse, Vec<OrderItemModel>), ServiceError> {
        if input.items.is_empty() {
            return Err(ServiceError::ValidationError(
                "orders must include at least one item".to_string(),
            ));
        }

        if input.total_amount < Decimal::ZERO {
            return Err(ServiceError::ValidationError(
                "total amount cannot be negative".to_string(),
            ));
        }

        let db = &*self.db_pool;
        let now = Utc::now();
        let order_id = Uuid::new_v4();

        let txn = db.begin().await.map_err(|e| {
            error!(error = %e, "Failed to start transaction for order creation");
            ServiceError::db_error(e)
        })?;

        let order_active = OrderActiveModel {
            id: Set(order_id),
            order_number: Set(format!("ORD-{}", now.timestamp_millis())),
            customer_id: Set(input.customer_id),
            status: Set(STATUS_PENDING.to_string()),
            order_date: Set(now),
            total_amount: Set(input.total_amount),
            currency: Set(input.currency.clone()),
            payment_status: Set(input.payment_status.clone()),
            fulfillment_status: Set(input.fulfillment_status.clone()),
            payment_method: Set(input.payment_method.clone()),
            shipping_method: Set(input.shipping_method.clone()),
            tracking_number: Set(None),
            notes: Set(input.notes.clone()),
            shipping_address: Set(input.shipping_address.clone()),
            billing_address: Set(input.billing_address.clone()),
            is_archived: Set(false),
            created_at: Set(now),
            updated_at: Set(Some(now)),
            version: Set(1),
        };

        let order_model = order_active.insert(&txn).await.map_err(|e| {
            error!(error = %e, order_id = %order_id, "Failed to persist order header");
            ServiceError::db_error(e)
        })?;

        let mut saved_items = Vec::with_capacity(input.items.len());
        for item in input.items.drain(..) {
            if item.quantity <= 0 {
                return Err(ServiceError::ValidationError(format!(
                    "invalid quantity {} for SKU {}",
                    item.quantity, item.sku
                )));
            }

            let total_price = item.unit_price * Decimal::from(item.quantity);
            let tax_rate = item.tax_rate.unwrap_or(Decimal::ZERO);
            let tax_amount = (total_price * tax_rate).round_dp(2);

            let am = OrderItemActiveModel {
                id: Set(Uuid::new_v4()),
                order_id: Set(order_id),
                product_id: Set(item.product_id.unwrap_or_else(Uuid::new_v4)),
                sku: Set(item.sku),
                name: Set(item.name.unwrap_or_else(|| "".to_string())),
                quantity: Set(item.quantity),
                unit_price: Set(item.unit_price),
                total_price: Set(total_price),
                discount: Set(Decimal::ZERO),
                tax_rate: Set(tax_rate),
                tax_amount: Set(tax_amount),
                status: Set("pending".to_string()),
                notes: Set(None),
                created_at: Set(now),
                updated_at: Set(Some(now)),
            };

            let saved = am.insert(&txn).await.map_err(|e| {
                error!(error = %e, order_id = %order_id, "Failed to persist order item");
                ServiceError::db_error(e)
            })?;
            saved_items.push(saved);
        }

        let payload = serde_json::json!({ "order_id": order_id.to_string() });
        if let Err(e) =
            crate::events::outbox::enqueue(&txn, "order", Some(order_id), "OrderCreated", &payload)
                .await
        {
            warn!(
                error = %e,
                order_id = %order_id,
                "Failed to enqueue outbox event for OrderCreated"
            );
        }

        txn.commit().await.map_err(|e| {
            error!(
                error = %e,
                order_id = %order_id,
                "Failed to commit order creation transaction"
            );
            ServiceError::db_error(e)
        })?;

        if let Some(sender) = &self.event_sender {
            sender.send_or_log(Event::OrderCreated(order_id)).await;
        }

        Ok((self.model_to_response(order_model), saved_items))
    }

    /// Creates a new order in the database
    #[instrument(skip(self, request), fields(customer_id = %request.customer_id, order_number = %request.order_number))]
    pub async fn create_order(
        &self,
        request: CreateOrderRequest,
    ) -> Result<OrderResponse, ServiceError> {
        // Validate the request
        request
            .validate()
            .map_err(|e| ServiceError::ValidationError(format!("Invalid order data: {}", e)))?;

        // Additional business validations
        if request.total_amount < Decimal::ZERO {
            return Err(ServiceError::ValidationError(
                "Total amount cannot be negative".to_string(),
            ));
        }

        let db = &*self.db_pool;
        let now = Utc::now();
        let order_id = Uuid::new_v4();

        // Start a database transaction
        let txn = db.begin().await.map_err(|e| {
            error!(error = %e, "Failed to start transaction for order creation");
            ServiceError::db_error(e)
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
            ServiceError::db_error(e)
        })?;

        // Enqueue outbox event within the transaction for reliability
        let payload = serde_json::json!({"order_id": order_id.to_string()});
        if let Err(e) =
            crate::events::outbox::enqueue(&txn, "order", Some(order_id), "OrderCreated", &payload)
                .await
        {
            warn!(error = %e, order_id = %order_id, "Failed to enqueue outbox for OrderCreated");
        }

        // Commit the transaction
        txn.commit().await.map_err(|e| {
            error!(error = %e, order_id = %order_id, "Failed to commit order creation transaction");
            ServiceError::db_error(e)
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
                ServiceError::db_error(e)
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
    pub async fn list_orders(
        &self,
        page: u64,
        per_page: u64,
    ) -> Result<OrderListResponse, ServiceError> {
        self.search_orders(OrderSearchQuery {
            customer_id: None,
            status: None,
            from_date: None,
            to_date: None,
            search: None,
            sort_field: OrderSortField::CreatedAt,
            sort_direction: SortDirection::Desc,
            page,
            per_page,
        })
        .await
    }

    /// Retrieves items for a given order
    #[instrument(skip(self), fields(order_id = %order_id))]
    pub async fn get_order_items(
        &self,
        order_id: Uuid,
    ) -> Result<Vec<OrderItemModel>, ServiceError> {
        let db = &*self.db_pool;
        let items = OrderItemEntity::find()
            .filter(OrderItemColumn::OrderId.eq(order_id))
            .order_by_asc(OrderItemColumn::CreatedAt)
            .all(db)
            .await
            .map_err(|e| {
                error!(error = %e, order_id = %order_id, "Failed to fetch order items");
                ServiceError::db_error(e)
            })?;
        Ok(items)
    }

    /// Retrieves items for multiple orders in a single query
    #[instrument(skip(self, order_ids))]
    pub async fn get_items_for_orders(
        &self,
        order_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Vec<OrderItemModel>>, ServiceError> {
        if order_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let db = &*self.db_pool;
        let ids: Vec<Uuid> = order_ids.iter().copied().collect();
        let rows = OrderItemEntity::find()
            .filter(OrderItemColumn::OrderId.is_in(ids))
            .order_by_asc(OrderItemColumn::CreatedAt)
            .all(db)
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to fetch batched order items");
                ServiceError::db_error(e)
            })?;

        let mut grouped: HashMap<Uuid, Vec<OrderItemModel>> = HashMap::new();
        for item in rows {
            grouped.entry(item.order_id).or_default().push(item);
        }

        Ok(grouped)
    }

    /// Adds an item to an order
    #[instrument(skip(self), fields(order_id = %order_id, sku = %sku))]
    pub async fn add_order_item(
        &self,
        order_id: Uuid,
        sku: String,
        product_id: Option<Uuid>,
        name: Option<String>,
        quantity: i32,
        unit_price: Decimal,
        tax_rate: Option<Decimal>,
    ) -> Result<OrderItemModel, ServiceError> {
        let db = &*self.db_pool;

        let total_price = unit_price * Decimal::from(quantity);
        let rate = tax_rate.unwrap_or(Decimal::ZERO);
        let tax_amount = (total_price * rate).round_dp(2);

        let am = OrderItemActiveModel {
            id: Set(Uuid::new_v4()),
            order_id: Set(order_id),
            product_id: Set(product_id.unwrap_or_else(Uuid::new_v4)),
            sku: Set(sku),
            name: Set(name.unwrap_or_else(|| "".to_string())),
            quantity: Set(quantity),
            unit_price: Set(unit_price),
            total_price: Set(total_price),
            discount: Set(Decimal::ZERO),
            tax_rate: Set(rate),
            tax_amount: Set(tax_amount),
            status: Set("pending".to_string()),
            notes: Set(None),
            created_at: Set(Utc::now()),
            updated_at: Set(Some(Utc::now())),
        };

        let saved = am.insert(db).await.map_err(|e| {
            error!(error = %e, order_id = %order_id, "Failed to add order item");
            ServiceError::db_error(e)
        })?;

        if let Some(sender) = &self.event_sender {
            sender.send_or_log(Event::OrderUpdated(order_id)).await;
        }

        Ok(saved)
    }

    /// Updates an order's status
    #[instrument(skip(self, request), fields(order_id = %order_id, new_status = %request.status))]
    pub async fn update_order_status(
        &self,
        order_id: Uuid,
        request: UpdateOrderStatusRequest,
    ) -> Result<OrderResponse, ServiceError> {
        request
            .validate()
            .map_err(|e| ServiceError::ValidationError(format!("Invalid status update: {}", e)))?;

        // Validate status transition
        let valid_statuses = vec![
            STATUS_PENDING,
            STATUS_PROCESSING,
            STATUS_SHIPPED,
            STATUS_DELIVERED,
            STATUS_CANCELLED,
            STATUS_REFUNDED,
        ];

        if !valid_statuses.contains(&request.status.as_str()) {
            return Err(ServiceError::ValidationError(format!(
                "Invalid status: {}. Must be one of: {:?}",
                request.status, valid_statuses
            )));
        }

        let db = &*self.db_pool;
        let now = Utc::now();

        // Start transaction
        let txn = db.begin().await.map_err(|e| {
            error!(error = %e, order_id = %order_id, "Failed to start transaction for status update");
            ServiceError::db_error(e)
        })?;

        // Find the order
        let order = OrderEntity::find_by_id(order_id)
            .one(&txn)
            .await
            .map_err(|e| {
                error!(error = %e, order_id = %order_id, "Failed to find order for status update");
                ServiceError::db_error(e)
            })?;

        let order = order.ok_or_else(|| {
            warn!(order_id = %order_id, "Order not found for status update");
            ServiceError::NotFound("Order not found".to_string())
        })?;

        let old_status = order.status.clone();

        // Validate status transition rules
        if !self.is_valid_status_transition(&old_status, &request.status) {
            return Err(ServiceError::ValidationError(format!(
                "Invalid status transition from {} to {}",
                old_status, request.status
            )));
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
            ServiceError::db_error(e)
        })?;

        // Enqueue status change outbox event
        let payload = serde_json::json!({"order_id": order_id.to_string(), "old_status": old_status, "new_status": request.status});
        if let Err(e) = crate::events::outbox::enqueue(
            &txn,
            "order",
            Some(order_id),
            "OrderStatusChanged",
            &payload,
        )
        .await
        {
            warn!(error = %e, order_id = %order_id, "Failed to enqueue outbox for OrderStatusChanged");
        }

        // Commit transaction
        txn.commit().await.map_err(|e| {
            error!(error = %e, order_id = %order_id, "Failed to commit status update transaction");
            ServiceError::db_error(e)
        })?;

        info!(order_id = %order_id, old_status = %old_status, new_status = %request.status, "Order status updated successfully");

        // Send event if event sender is available
        if let Some(event_sender) = &self.event_sender {
            if let Err(e) = event_sender
                .send(Event::OrderStatusChanged {
                    order_id,
                    old_status,
                    new_status: request.status.clone(),
                })
                .await
            {
                warn!(error = %e, order_id = %order_id, "Failed to send order status changed event");
            }
        }

        Ok(self.model_to_response(updated_order))
    }

    /// Cancels an order
    #[instrument(skip(self), fields(order_id = %order_id))]
    pub async fn cancel_order(
        &self,
        order_id: Uuid,
        reason: Option<String>,
    ) -> Result<OrderResponse, ServiceError> {
        // First check if the order can be cancelled
        let order = self
            .get_order(order_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound("Order not found".to_string()))?;

        if order.status == STATUS_DELIVERED || order.status == STATUS_CANCELLED {
            return Err(ServiceError::ValidationError(format!(
                "Cannot cancel order with status: {}",
                order.status
            )));
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
                ServiceError::db_error(e)
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
            ServiceError::db_error(e)
        })?;

        info!(order_id = %order_id, "Order archived successfully");

        Ok(self.model_to_response(archived_order))
    }

    /// Find an order by its order_number (string identifier)
    #[instrument(skip(self))]
    pub async fn get_order_by_order_number(
        &self,
        order_number: &str,
    ) -> Result<Option<OrderResponse>, ServiceError> {
        let db = &*self.db_pool;
        let found = OrderEntity::find()
            .filter(order::Column::OrderNumber.eq(order_number.to_string()))
            .one(db)
            .await
            .map_err(|e| {
                error!(error = %e, order_number = order_number, "Failed to fetch order by order_number");
                ServiceError::db_error(e)
            })?;
        Ok(found.map(|m| self.model_to_response(m)))
    }

    /// Resolve an order's UUID by order_number
    #[instrument(skip(self))]
    pub async fn find_order_id_by_order_number(
        &self,
        order_number: &str,
    ) -> Result<Option<Uuid>, ServiceError> {
        let db = &*self.db_pool;
        let found = OrderEntity::find()
            .filter(order::Column::OrderNumber.eq(order_number.to_string()))
            .one(db)
            .await
            .map_err(|e| {
                error!(error = %e, order_number = order_number, "Failed to resolve order id by order_number");
                ServiceError::db_error(e)
            })?;
        Ok(found.map(|m| m.id))
    }

    /// Ensure a demo order with order_number "order_123" exists in development
    #[instrument(skip(self))]
    pub async fn ensure_demo_order(&self) -> Result<Uuid, ServiceError> {
        use sea_orm::ActiveValue::Set;
        let db = &*self.db_pool;

        if let Some(existing) = OrderEntity::find()
            .filter(order::Column::OrderNumber.eq("order_123"))
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
        {
            return Ok(existing.id);
        }

        let now = Utc::now();
        let order_id = Uuid::new_v4();
        let header = OrderActiveModel {
            id: Set(order_id),
            order_number: Set("order_123".to_string()),
            customer_id: Set(Uuid::new_v4()),
            status: Set(STATUS_PROCESSING.to_string()),
            order_date: Set(now),
            total_amount: Set(Decimal::new(6997, 2)),
            currency: Set("USD".to_string()),
            payment_status: Set("paid".to_string()),
            fulfillment_status: Set("unfulfilled".to_string()),
            payment_method: Set(Some("pm_123".to_string())),
            shipping_method: Set(Some("standard".to_string())),
            tracking_number: Set(Some("ship_456".to_string())),
            notes: Set(Some("Demo order".to_string())),
            shipping_address: Set(Some("123 Main St, Anytown, CA, US 12345".to_string())),
            billing_address: Set(None),
            is_archived: Set(false),
            created_at: Set(now - chrono::Duration::hours(2)),
            updated_at: Set(Some(now)),
            version: Set(1),
        };
        let _model = header
            .insert(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        // Add a couple of items if none exist
        let existing_items = OrderItemEntity::find()
            .filter(OrderItemColumn::OrderId.eq(order_id))
            .count(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;
        if existing_items == 0 {
            let _ = OrderItemActiveModel {
                id: Set(Uuid::new_v4()),
                order_id: Set(order_id),
                product_id: Set(Uuid::new_v4()),
                sku: Set("prod_123".to_string()),
                name: Set("Sample Product 1".to_string()),
                quantity: Set(2),
                unit_price: Set(Decimal::new(1999, 2)),
                total_price: Set(Decimal::new(3998, 2)),
                discount: Set(Decimal::ZERO),
                tax_rate: Set(Decimal::new(160, 2) / Decimal::new(1999, 2)),
                tax_amount: Set(Decimal::new(320, 2)),
                status: Set("pending".to_string()),
                notes: Set(None),
                created_at: Set(now - chrono::Duration::hours(2)),
                updated_at: Set(Some(now)),
            }
            .insert(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

            let _ = OrderItemActiveModel {
                id: Set(Uuid::new_v4()),
                order_id: Set(order_id),
                product_id: Set(Uuid::new_v4()),
                sku: Set("prod_456".to_string()),
                name: Set("Sample Product 2".to_string()),
                quantity: Set(1),
                unit_price: Set(Decimal::new(2999, 2)),
                total_price: Set(Decimal::new(2999, 2)),
                discount: Set(Decimal::ZERO),
                tax_rate: Set(Decimal::new(120, 2) / Decimal::new(2999, 2)),
                tax_amount: Set(Decimal::new(240, 2)),
                status: Set("pending".to_string()),
                notes: Set(None),
                created_at: Set(now - chrono::Duration::hours(2)),
                updated_at: Set(Some(now)),
            }
            .insert(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;
        }

        Ok(order_id)
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
        let order = self
            .get_order(order_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound("Order not found".to_string()))?;

        // Only allow deletion of cancelled or draft orders
        if order.status != STATUS_CANCELLED && order.status != STATUS_PENDING {
            return Err(ServiceError::ValidationError(format!(
                "Cannot delete order with status: {}",
                order.status
            )));
        }

        self.archive_order(order_id).await?;

        info!(order_id = %order_id, "Order deleted (archived) successfully");
        Ok(())
    }

    /// Search orders by various criteria
    #[instrument(skip(self))]
    pub async fn search_orders(
        &self,
        query: OrderSearchQuery,
    ) -> Result<OrderListResponse, ServiceError> {
        let OrderSearchQuery {
            customer_id,
            status,
            from_date,
            to_date,
            search,
            sort_field,
            sort_direction,
            page,
            per_page,
        } = query;

        if page == 0 {
            return Err(ServiceError::ValidationError(
                "Page number must be greater than 0".to_string(),
            ));
        }
        if per_page == 0 || per_page > 100 {
            return Err(ServiceError::ValidationError(
                "Per page must be between 1 and 100".to_string(),
            ));
        }

        let db = &*self.db_pool;
        let mut select = OrderEntity::find().filter(order::Column::IsArchived.eq(false));

        if let Some(cid) = customer_id {
            select = select.filter(order::Column::CustomerId.eq(cid));
        }

        if let Some(status_filter) = status {
            select = select.filter(order::Column::Status.eq(status_filter));
        }

        if let Some(from) = from_date {
            select = select.filter(order::Column::OrderDate.gte(from));
        }

        if let Some(to) = to_date {
            select = select.filter(order::Column::OrderDate.lte(to));
        }

        if let Some(search_term) = search {
            let mut search_condition = Condition::any();
            search_condition =
                search_condition.add(order::Column::OrderNumber.contains(search_term.clone()));
            search_condition =
                search_condition.add(order::Column::Notes.contains(search_term.clone()));
            search_condition =
                search_condition.add(order::Column::ShippingAddress.contains(search_term.clone()));
            search_condition =
                search_condition.add(order::Column::BillingAddress.contains(search_term));
            select = select.filter(search_condition);
        }

        let sort_column = match sort_field {
            OrderSortField::CreatedAt => order::Column::CreatedAt,
            OrderSortField::OrderDate => order::Column::OrderDate,
            OrderSortField::TotalAmount => order::Column::TotalAmount,
            OrderSortField::OrderNumber => order::Column::OrderNumber,
        };

        select = match sort_direction {
            SortDirection::Asc => select.order_by_asc(sort_column),
            SortDirection::Desc => select.order_by_desc(sort_column),
        };

        if sort_field != OrderSortField::CreatedAt {
            select = select.order_by_desc(order::Column::CreatedAt);
        }

        let paginator = select.paginate(db, per_page);
        let total = paginator
            .num_items()
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let orders = paginator
            .fetch_page(page - 1)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

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

    /// Updates order header details such as addresses, payment method, and notes.
    #[instrument(skip(self, details), fields(order_id = %order_id))]
    pub async fn update_order_details(
        &self,
        order_id: Uuid,
        details: UpdateOrderDetails,
    ) -> Result<OrderResponse, ServiceError> {
        let has_updates = details.shipping_address.is_some()
            || details.billing_address.is_some()
            || details.payment_method.is_some()
            || details.notes.is_some();

        let db = &*self.db_pool;
        let order_record = OrderEntity::find_by_id(order_id)
            .one(db)
            .await
            .map_err(|e| {
                error!(
                    error = %e,
                    order_id = %order_id,
                    "Failed to fetch order for update"
                );
                ServiceError::db_error(e)
            })?;

        let order_model = order_record.ok_or_else(|| {
            warn!(order_id = %order_id, "Order not found for update");
            ServiceError::NotFound("Order not found".to_string())
        })?;

        if !has_updates {
            return Ok(self.model_to_response(order_model));
        }

        let now = Utc::now();
        let UpdateOrderDetails {
            shipping_address,
            billing_address,
            payment_method,
            notes,
        } = details;
        let current_version = order_model.version;
        let mut order_active: OrderActiveModel = order_model.into();
        order_active.updated_at = Set(Some(now));
        order_active.version = Set(current_version + 1);

        if let Some(value) = shipping_address {
            order_active.shipping_address = Set(Some(value));
        }
        if let Some(value) = billing_address {
            order_active.billing_address = Set(Some(value));
        }
        if let Some(value) = payment_method {
            order_active.payment_method = Set(Some(value));
        }
        if let Some(value) = notes {
            order_active.notes = Set(Some(value));
        }

        let updated = order_active.update(db).await.map_err(|e| {
            error!(
                error = %e,
                order_id = %order_id,
                "Failed to update order details"
            );
            ServiceError::db_error(e)
        })?;

        if let Some(sender) = &self.event_sender {
            sender.send_or_log(Event::OrderUpdated(order_id)).await;
        }

        Ok(self.model_to_response(updated))
    }
}

#[cfg(all(test, feature = "mock-tests"))]
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
