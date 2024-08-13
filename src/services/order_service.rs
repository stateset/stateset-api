use crate::inventory::InventoryService;
use crate::shipments::ShipmentService;
use crate::models::{Order, OrderItem, OrderStatus, OrderNote, OrderHistory, NewOrder};
use crate::db::DbPool;
use crate::cache::Cache;
use crate::errors::{ApiError, OrderError};
use crate::events::{EventSender, Event};
use async_trait::async_trait;
use uuid::Uuid;
use futures::future::try_join_all;
use std::sync::Arc;
use std::time::Duration;
use diesel::prelude::*;
use diesel::PgConnection;

pub struct OrderService {
    db_pool: Arc<DbPool>,
    cache: Arc<dyn Cache>,
    inventory_service: Arc<InventoryService>,
    shipment_service: Arc<ShipmentService>,
    event_sender: EventSender,
}

impl OrderService {
    pub fn new(
        db_pool: Arc<DbPool>,
        cache: Arc<dyn Cache>,
        inventory_service: Arc<InventoryService>,
        shipment_service: Arc<ShipmentService>,
        event_sender: EventSender,
    ) -> Self {
        Self {
            db_pool,
            cache,
            inventory_service,
            shipment_service,
            analytics_service,
            event_sender,
        }
    }

    pub async fn create_order(&self, new_order: NewOrder) -> Result<Order, OrderError> {
        self.validate_order(&new_order).await?;

        let order = self.db_pool.get().and_then(|conn| {
            conn.transaction::<Order, OrderError, _>(|| {
                self.pre_create_hook(&new_order)?;

                for item in &new_order.items {
                    self.inventory_service.reserve_inventory(item.product_id, item.quantity)?;
                }

                let order = diesel::insert_into(orders::table)
                    .values(&new_order)
                    .get_result::<Order>(&conn)?;

                let order_items: Vec<OrderItem> = new_order.items.into_iter().map(|item| {
                    OrderItem {
                        order_id: order.id,
                        product_id: item.product_id,
                        quantity: item.quantity,
                        price: item.price,
                    }
                }).collect();

                diesel::insert_into(order_items::table)
                    .values(&order_items)
                    .execute(&conn)?;

                self.post_create_hook(&order)?;
                Ok(order)
            })
        }).map_err(|e| OrderError::DatabaseError(format!("Failed to create order: {}", e)))?;

        self.cache_order(order.clone()).await?;
        self.event_sender.send(Event::OrderCreated(order.id)).await.map_err(|e| OrderError::EventError(e.to_string()))?;

        Ok(order)
    }

    pub async fn get_order(&self, id: Uuid) -> Result<Order, OrderError> {
        if let Some(cached_order) = self.get_cached_order(id).await? {
            return Ok(cached_order);
        }

        let order = self.db_pool.get().and_then(|conn| {
            orders::table.find(id).first::<Order>(&conn)
        }).map_err(|e| OrderError::DatabaseError(format!("Failed to retrieve order with ID {}: {}", id, e)))?;

        self.cache_order(order.clone()).await?;
        Ok(order)
    }

    pub async fn update_order_status(&self, id: Uuid, new_status: OrderStatus) -> Result<Order, OrderError> {
        let order = self.db_pool.get().and_then(|conn| {
            conn.transaction::<Order, OrderError, _>(|| {
                self.pre_update_hook(id, &new_status)?;

                let order = diesel::update(orders::table.find(id))
                    .set(orders::status.eq(new_status))
                    .get_result::<Order>(&conn)?;

                match new_status {
                    OrderStatus::Shipped => self.shipment_service.create_shipment(order.id)?,
                    OrderStatus::Cancelled => self.release_inventory(conn, &order)?,
                    _ => {}
                }

                self.post_update_hook(&order)?;
                Ok(order)
            })
        }).map_err(|e| OrderError::DatabaseError(format!("Failed to update order status: {}", e)))?;

        self.cache_order(order.clone()).await?;
        self.event_sender.send(Event::OrderUpdated(id)).await.map_err(|e| OrderError::EventError(e.to_string()))?;

        Ok(order)
    }

    pub async fn add_order_note(&self, order_id: Uuid, note: String) -> Result<OrderNote, OrderError> {
        let order_note = self.db_pool.get().and_then(|conn| {
            diesel::insert_into(order_notes::table)
                .values(&NewOrderNote { order_id, note })
                .get_result::<OrderNote>(&conn)
        }).map_err(|e| OrderError::DatabaseError(format!("Failed to add note to order: {}", e)))?;

        self.invalidate_cache(order_id).await?;
        Ok(order_note)
    }

    pub async fn get_order_history(&self, order_id: Uuid) -> Result<Vec<OrderHistory>, OrderError> {
        let history = self.db_pool.get().and_then(|conn| {
            OrderHistory::belonging_to(&Order::find(order_id))
                .order(order_history::created_at.desc())
                .load::<OrderHistory>(&conn)
        }).map_err(|e| OrderError::DatabaseError(format!("Failed to retrieve order history: {}", e)))?;

        Ok(history)
    }

    pub async fn search_orders(&self, query: &str, limit: i64, offset: i64) -> Result<Vec<Order>, OrderError> {
        let orders = self.db_pool.get().and_then(|conn| {
            orders::table
                .filter(orders::order_number.ilike(format!("%{}%", query)))
                .or_filter(orders::customer_name.ilike(format!("%{}%", query)))
                .limit(limit)
                .offset(offset)
                .load::<Order>(&conn)
        }).map_err(|e| OrderError::DatabaseError(format!("Failed to search orders: {}", e)))?;

        Ok(orders)
    }

    pub async fn batch_create_orders(&self, new_orders: Vec<NewOrder>) -> Result<Vec<Order>, OrderError> {
        let results = try_join_all(new_orders.into_iter().map(|new_order| {
            let order_service = self.clone();
            async move {
                order_service.create_order(new_order).await
            }
        })).await?;

        Ok(results)
    }

    async fn validate_order(&self, new_order: &NewOrder) -> Result<(), OrderError> {
        // Implement order validation logic
        Ok(())
    }

    async fn cache_order(&self, order: Order) -> Result<(), OrderError> {
        let cache_key = self.cache_key(order.id);
        self.cache.set(&cache_key, &serde_json::to_string(&order)?, Some(Duration::from_secs(3600))).await
            .map_err(|e| OrderError::CacheError(format!("Failed to cache order {}: {}", order.id, e)))
    }

    async fn get_cached_order(&self, id: Uuid) -> Result<Option<Order>, OrderError> {
        let cache_key = self.cache_key(id);
        if let Some(cached_order) = self.cache.get(&cache_key).await
            .map_err(|e| OrderError::CacheError(format!("Failed to get cached order {}: {}", id, e)))? {
            return Ok(Some(serde_json::from_str(&cached_order)?));
        }
        Ok(None)
    }

    async fn invalidate_cache(&self, id: Uuid) -> Result<(), OrderError> {
        self.cache.delete(&self.cache_key(id)).await
            .map_err(|e| OrderError::CacheError(format!("Failed to invalidate cache for order {}: {}", id, e)))
    }

    fn release_inventory(&self, conn: &PgConnection, order: &Order) -> Result<(), OrderError> {
        let order_items = OrderItem::belonging_to(order).load::<OrderItem>(conn)?;
        for item in order_items {
            self.inventory_service.release_inventory(item.product_id, item.quantity)?;
        }
        Ok(())
    }

    fn cache_key(&self, id: Uuid) -> String {
        format!("order:{}:v1", id)
    }

    fn pre_create_hook(&self, new_order: &NewOrder) -> Result<(), OrderError> {
        // Implement pre-creation logic
        Ok(())
    }

    fn post_create_hook(&self, order: &Order) -> Result<(), OrderError> {
        // Implement post-creation logic
        Ok(())
    }

    fn pre_update_hook(&self, id: Uuid, new_status: &OrderStatus) -> Result<(), OrderError> {
        // Implement pre-update logic
        Ok(())
    }

    fn post_update_hook(&self, order: &Order) -> Result<(), OrderError> {
        // Implement post-update logic
        Ok(())
    }
}

#[async_trait]
impl EventHandler for OrderService {
    async fn handle_event(&self, event: Event) {
        match event {
            Event::ShipmentCreated(shipment_id) => {
                // Implement logic to update order status when shipment is created
            },
            Event::ReturnCreated(return_id) => {
                // Implement logic to update order status when return is created
            },
            _ => {}
        }
    }
}
