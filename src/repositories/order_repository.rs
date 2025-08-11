use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, ModelTrait, PaginatorTrait,
    QueryFilter, QueryOrder, Set, ActiveValue,
};
use rust_decimal::Decimal;
use std::sync::Arc;
use uuid::Uuid;

use crate::entities::order::{
    ActiveModel as OrderActiveModel, Column, Entity as Order, Model as OrderModel,
};
use crate::entities::order_item::{Entity as OrderItem, Model as OrderItemModel};
use crate::errors::AppError;
use crate::repositories::Repository;

use super::BaseRepository;

/// Repository for order operations
#[derive(Debug)]
pub struct OrderRepository {
    base: BaseRepository,
}

impl OrderRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            base: BaseRepository::new(db),
        }
    }

    /// Find an order by ID
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<OrderModel>, AppError> {
        Order::find_by_id(id)
            .one(self.base.get_db())
            .await
            .map_err(|e| AppError::DatabaseError(e))
    }

    /// Find orders by customer ID
    pub async fn find_by_customer(
        &self,
        customer_id: Uuid,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<OrderModel>, u64), AppError> {
        let paginator = Order::find()
            .filter(Column::CustomerId.eq(customer_id))
            .order_by_desc(Column::CreatedAt)
            .paginate(self.base.get_db(), page_size);

        let total = paginator
            .num_items()
            .await
            .map_err(|e| AppError::DatabaseError(e))?;

        let orders = paginator
            .fetch_page(page - 1)
            .await
            .map_err(|e| AppError::DatabaseError(e))?;

        Ok((orders, total))
    }

    /// Get all orders with pagination
    pub async fn find_all(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<OrderModel>, u64), AppError> {
        let paginator = Order::find()
            .order_by_desc(Column::CreatedAt)
            .paginate(self.base.get_db(), page_size);

        let total = paginator
            .num_items()
            .await
            .map_err(|e| AppError::DatabaseError(e))?;

        let orders = paginator
            .fetch_page(page - 1)
            .await
            .map_err(|e| AppError::DatabaseError(e))?;

        Ok((orders, total))
    }

    /// Get orders by status
    pub async fn find_by_status(
        &self,
        status: &str,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<OrderModel>, u64), AppError> {
        let paginator = Order::find()
            .filter(Column::Status.eq(status))
            .order_by_desc(Column::CreatedAt)
            .paginate(self.base.get_db(), page_size);

        let total = paginator
            .num_items()
            .await
            .map_err(|e| AppError::DatabaseError(e))?;

        let orders = paginator
            .fetch_page(page - 1)
            .await
            .map_err(|e| AppError::DatabaseError(e))?;

        Ok((orders, total))
    }

    /// Create a new order
    pub async fn create(&self, order: OrderActiveModel) -> Result<OrderModel, AppError> {
        order
            .insert(self.base.get_db())
            .await
            .map_err(|e| AppError::DatabaseError(e))
    }

    /// Update an order
    pub async fn update(&self, id: Uuid, order: OrderActiveModel) -> Result<OrderModel, AppError> {
        let existing_order = Order::find_by_id(id)
            .one(self.base.get_db())
            .await?
            .ok_or_else(|| AppError::DatabaseError(format!("Order with ID {} not found", id)))?;

        let mut active_model: OrderActiveModel = existing_order.into();

        if let Some(total_amount) = order.total_amount.as_ref() {
            active_model.total_amount = Set(total_amount.clone());
        }
        if let Some(currency) = order.currency.as_ref() {
            active_model.currency = Set(currency.clone());
        }
        if let Some(payment_status) = order.payment_status.as_ref() {
            active_model.payment_status = Set(payment_status.clone());
        }
        if let Some(fulfillment_status) = order.fulfillment_status.as_ref() {
            active_model.fulfillment_status = Set(fulfillment_status.clone());
        }

        active_model.updated_at = Set(Some(Utc::now()));

        active_model.update(self.base.get_db()).await
            .map_err(|e| AppError::DatabaseError(e))
    }

    /// Delete an order
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let order = self
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::DatabaseError(format!("Order with ID {} not found", id)))?;

        order
            .delete(self.base.get_db())
            .await
            .map_err(|e| AppError::DatabaseError(e))?;

        Ok(())
    }

    /// Get order items for an order
    pub async fn get_order_items(&self, order_id: Uuid) -> Result<Vec<OrderItemModel>, AppError> {
        let order = self.find_by_id(order_id).await?.ok_or_else(|| {
            AppError::DatabaseError(format!("Order with ID {} not found", order_id))
        })?;

        order
            .find_related(OrderItem)
            .all(self.base.get_db())
            .await
            .map_err(|e| AppError::DatabaseError(e))
    }
}

impl Repository for OrderRepository {
    fn get_db(&self) -> &DatabaseConnection {
        self.base.get_db()
    }
}
