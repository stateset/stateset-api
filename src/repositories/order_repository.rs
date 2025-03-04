use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, 
    ModelTrait, PaginatorTrait, QueryFilter, QueryOrder, Set
};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use std::sync::Arc;

use crate::entities::order::{Entity as Order, Model as OrderModel, ActiveModel as OrderActiveModel, Column};
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
            .map_err(|e| AppError::DatabaseError(format!("Error finding order: {}", e)))
    }

    /// Find orders by customer ID
    pub async fn find_by_customer(&self, customer_id: Uuid, page: u64, page_size: u64) -> Result<(Vec<OrderModel>, u64), AppError> {
        let paginator = Order::find()
            .filter(Column::CustomerId.eq(customer_id))
            .order_by_desc(Column::CreatedAt)
            .paginate(self.base.get_db(), page_size);

        let total = paginator
            .num_items()
            .await
            .map_err(|e| AppError::DatabaseError(format!("Error counting orders: {}", e)))?;

        let orders = paginator
            .fetch_page(page - 1)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Error fetching orders: {}", e)))?;

        Ok((orders, total))
    }

    /// Get all orders with pagination
    pub async fn find_all(&self, page: u64, page_size: u64) -> Result<(Vec<OrderModel>, u64), AppError> {
        let paginator = Order::find()
            .order_by_desc(Column::CreatedAt)
            .paginate(self.base.get_db(), page_size);

        let total = paginator
            .num_items()
            .await
            .map_err(|e| AppError::DatabaseError(format!("Error counting orders: {}", e)))?;

        let orders = paginator
            .fetch_page(page - 1)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Error fetching orders: {}", e)))?;

        Ok((orders, total))
    }

    /// Get orders by status
    pub async fn find_by_status(&self, status: &str, page: u64, page_size: u64) -> Result<(Vec<OrderModel>, u64), AppError> {
        let paginator = Order::find()
            .filter(Column::Status.eq(status))
            .order_by_desc(Column::CreatedAt)
            .paginate(self.base.get_db(), page_size);

        let total = paginator
            .num_items()
            .await
            .map_err(|e| AppError::DatabaseError(format!("Error counting orders: {}", e)))?;

        let orders = paginator
            .fetch_page(page - 1)
            .await
            .map_err(|e| AppError::DatabaseError(format!("Error fetching orders: {}", e)))?;

        Ok((orders, total))
    }

    /// Create a new order
    pub async fn create(&self, order: OrderActiveModel) -> Result<OrderModel, AppError> {
        order
            .insert(self.base.get_db())
            .await
            .map_err(|e| AppError::DatabaseError(format!("Error creating order: {}", e)))
    }

    /// Update an order
    pub async fn update(&self, id: Uuid, order: OrderActiveModel) -> Result<OrderModel, AppError> {
        // Find the order first to ensure it exists
        let existing = self.find_by_id(id).await?
            .ok_or_else(|| AppError::DatabaseError(format!("Order with ID {} not found", id)))?;

        // Convert to active model and update
        let mut order_am: OrderActiveModel = existing.into();
        
        // Apply changes from the input model, preserving ID
        if let Some(status) = order.status.as_ref() {
            order_am.status = Set(status.clone());
        }
        
        if let Some(total_amount) = order.total_amount.as_ref() {
            order_am.total_amount = Set(*total_amount);
        }
        
        if let Some(currency) = order.currency.as_ref() {
            order_am.currency = Set(currency.clone());
        }
        
        if let Some(payment_status) = order.payment_status.as_ref() {
            order_am.payment_status = Set(payment_status.clone());
        }
        
        if let Some(fulfillment_status) = order.fulfillment_status.as_ref() {
            order_am.fulfillment_status = Set(fulfillment_status.clone());
        }
        
        if let Some(notes) = order.notes.as_ref() {
            order_am.notes = Set(notes.clone());
        }
        
        if let Some(shipping_address) = order.shipping_address.as_ref() {
            order_am.shipping_address = Set(shipping_address.clone());
        }
        
        if let Some(billing_address) = order.billing_address.as_ref() {
            order_am.billing_address = Set(billing_address.clone());
        }
        
        if let Some(is_archived) = order.is_archived.as_ref() {
            order_am.is_archived = Set(*is_archived);
        }
        
        // Always update the updated_at timestamp
        order_am.updated_at = Set(Some(Utc::now()));
        
        order_am
            .update(self.base.get_db())
            .await
            .map_err(|e| AppError::DatabaseError(format!("Error updating order: {}", e)))
    }

    /// Delete an order
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let order = self.find_by_id(id).await?
            .ok_or_else(|| AppError::DatabaseError(format!("Order with ID {} not found", id)))?;
            
        order
            .delete(self.base.get_db())
            .await
            .map_err(|e| AppError::DatabaseError(format!("Error deleting order: {}", e)))?;
            
        Ok(())
    }

    /// Get order items for an order
    pub async fn get_order_items(&self, order_id: Uuid) -> Result<Vec<OrderItemModel>, AppError> {
        let order = self.find_by_id(order_id).await?
            .ok_or_else(|| AppError::DatabaseError(format!("Order with ID {} not found", order_id)))?;
            
        order
            .find_related(OrderItem)
            .all(self.base.get_db())
            .await
            .map_err(|e| AppError::DatabaseError(format!("Error fetching order items: {}", e)))
    }
}

impl Repository for OrderRepository {
    fn get_db(&self) -> &DatabaseConnection {
        self.base.get_db()
    }
}