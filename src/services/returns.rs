use crate::models::{Return, ReturnItem, ReturnStatus, Order, OrderStatus};
use crate::db::DbPool;
use crate::errors::{ApiError, ReturnError};
use crate::events::{EventSender, Event};
use crate::services::inventory::InventoryService;
use crate::services::order::OrderService;
use crate::utils::pagination::PaginationParams;
use uuid::Uuid;
use chrono::Utc;
use serde::{Serialize, Deserialize};
use async_trait::async_trait;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct NewReturn {
    pub order_id: Uuid,
    pub reason: String,
    pub items: Vec<NewReturnItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewReturnItem {
    pub product_id: Uuid,
    pub quantity: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReturnSearchParams {
    pub order_id: Option<Uuid>,
    pub status: Option<ReturnStatus>,
    pub date_from: Option<chrono::NaiveDateTime>,
    pub date_to: Option<chrono::NaiveDateTime>,
}

pub struct ReturnService {
    db_pool: Arc<DbPool>,
    inventory_service: Arc<InventoryService>,
    order_service: Arc<OrderService>,
    event_sender: EventSender,
}

impl ReturnService {
    pub fn new(
        db_pool: Arc<DbPool>, 
        inventory_service: Arc<InventoryService>,
        order_service: Arc<OrderService>,
        event_sender: EventSender
    ) -> Self {
        Self { db_pool, inventory_service, order_service, event_sender }
    }

    pub async fn create_return(&self, new_return: NewReturn, user_id: Uuid) -> Result<Return, ReturnError> {
        let conn = self.db_pool.get().map_err(|_| ReturnError::DatabaseError)?;

        let return_item = conn.transaction::<_, ReturnError, _>(|| {
            // Verify the order exists and belongs to the user
            let order = Order::belonging_to_user(new_return.order_id, user_id)
                .first::<Order>(&conn)
                .map_err(|_| ReturnError::OrderNotFound)?;

            // Create the return
            let return_item = diesel::insert_into(returns::table)
                .values(&Return {
                    id: Uuid::new_v4(),
                    order_id: new_return.order_id,
                    user_id,
                    status: ReturnStatus::Pending,
                    reason: new_return.reason,
                    created_at: Utc::now().naive_utc(),
                    updated_at: Utc::now().naive_utc(),
                })
                .get_result::<Return>(&conn)?;

            // Create return items
            for item in new_return.items {
                diesel::insert_into(return_items::table)
                    .values(&ReturnItem {
                        id: Uuid::new_v4(),
                        return_id: return_item.id,
                        product_id: item.product_id,
                        quantity: item.quantity,
                    })
                    .execute(&conn)?;
            }

            Ok(return_item)
        })?;

        self.event_sender.send(Event::ReturnCreated(return_item.id))?;

        Ok(return_item)
    }

    pub async fn process_return(&self, id: Uuid, user_id: Uuid) -> Result<Return, ReturnError> {
        let conn = self.db_pool.get().map_err(|_| ReturnError::DatabaseError)?;

        let return_item = conn.transaction::<_, ReturnError, _>(|| {
            let return_item = diesel::update(returns::table.find(id))
                .filter(returns::user_id.eq(user_id))
                .set(returns::status.eq(ReturnStatus::Processed))
                .get_result::<Return>(&conn)?;

            // Update inventory
            let return_items = ReturnItem::belonging_to(&return_item).load::<ReturnItem>(&conn)?;
            for item in return_items {
                self.inventory_service.release_inventory(item.product_id, item.quantity)?;
            }

            // Update order status
            self.order_service.update_order_status(return_item.order_id, OrderStatus::Returned)?;

            Ok(return_item)
        })?;

        self.event_sender.send(Event::ReturnProcessed(id))?;

        Ok(return_item)
    }

    pub async fn get_return(&self, id: Uuid, user_id: Uuid) -> Result<Return, ReturnError> {
        let conn = self.db_pool.get().map_err(|_| ReturnError::DatabaseError)?;
        let return_item = returns::table
            .filter(returns::id.eq(id))
            .filter(returns::user_id.eq(user_id))
            .first::<Return>(&conn)
            .map_err(|_| ReturnError::ReturnNotFound)?;
        Ok(return_item)
    }

    pub async fn update_return(&self, id: Uuid, updated_return: Return, user_id: Uuid) -> Result<Return, ReturnError> {
        let conn = self.db_pool.get().map_err(|_| ReturnError::DatabaseError)?;
        let return_item = diesel::update(returns::table)
            .filter(returns::id.eq(id))
            .filter(returns::user_id.eq(user_id))
            .set(&updated_return)
            .get_result::<Return>(&conn)
            .map_err(|_| ReturnError::ReturnNotFound)?;
        
        if updated_return.status == ReturnStatus::Approved {
            self.process_refund(&return_item).await?;
        }
        
        Ok(return_item)
    }

    pub async fn list_returns(&self, user_id: Uuid, pagination: PaginationParams) -> Result<(Vec<Return>, i64), ReturnError> {
        let conn = self.db_pool.get().map_err(|_| ReturnError::DatabaseError)?;
        let returns = returns::table
            .filter(returns::user_id.eq(user_id))
            .order(returns::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load::<Return>(&conn)?;

        let total = returns::table
            .filter(returns::user_id.eq(user_id))
            .count()
            .get_result(&conn)?;

        Ok((returns, total))
    }

    pub async fn search_returns(&self, user_id: Uuid, search: ReturnSearchParams, pagination: PaginationParams) -> Result<(Vec<Return>, i64), ReturnError> {
        let conn = self.db_pool.get().map_err(|_| ReturnError::DatabaseError)?;
        let mut query = returns::table.into_boxed();
        
        query = query.filter(returns::user_id.eq(user_id));
        
        if let Some(order_id) = search.order_id {
            query = query.filter(returns::order_id.eq(order_id));
        }
        
        if let Some(status) = search.status {
            query = query.filter(returns::status.eq(status));
        }

        if let Some(date_from) = search.date_from {
            query = query.filter(returns::created_at.ge(date_from));
        }

        if let Some(date_to) = search.date_to {
            query = query.filter(returns::created_at.le(date_to));
        }
        
        let returns = query
            .order(returns::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load::<Return>(&conn)?;

        let total = query.count().get_result(&conn)?;
        
        Ok((returns, total))
    }

    async fn process_refund(&self, return_item: &Return) -> Result<(), ReturnError> {
        // Implement refund process logic
        // This could involve calling a payment service, updating financial records, etc.
        Ok(())
    }

    async fn notify_return_created(&self, return_item: &Return) -> Result<(), ReturnError> {
        // Implement notification logic (e.g., send email, push notification)
        Ok(())
    }
}

#[async_trait]
impl EventHandler for ReturnService {
    async fn handle_event(&self, event: Event) {
        match event {
            Event::OrderCancelled(order_id) => {
                // Handle cancelled order, maybe update related returns
            },
            Event::InventoryUpdated(product_id) => {
                // Maybe update status of pending returns if inventory becomes available
            },
            _ => {}
        }
    }
}