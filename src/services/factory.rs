use std::sync::Arc;

use crate::{
    db::DbPool,
    events::EventSender,
    services::{
        inventory::InventoryService, order_status::OrderStatusService, orders::OrderService,
    },
};

/// Factory for creating service instances with shared dependencies
pub struct ServiceFactory {
    db_pool: Arc<DbPool>,
    event_sender: EventSender,
}

impl ServiceFactory {
    /// Creates a new service factory with the given dependencies
    pub fn new(db_pool: Arc<DbPool>, event_sender: EventSender) -> Self {
        Self {
            db_pool,
            event_sender,
        }
    }

    /// Creates an inventory service instance
    pub fn inventory_service(&self) -> InventoryService {
        InventoryService::new(self.db_pool.clone(), self.event_sender.clone())
    }

    /// Creates an order service instance
    pub fn order_service(&self) -> OrderService {
        OrderService::new(
            self.db_pool.clone(),
            Some(Arc::new(self.event_sender.clone())),
        )
    }

    /// Creates an order status service instance
    pub fn order_status_service(&self) -> OrderStatusService {
        OrderStatusService::new(self.db_pool.clone())
    }

    /// Creates all services as a tuple for convenience
    pub fn create_all(&self) -> (InventoryService, OrderService, OrderStatusService) {
        (
            self.inventory_service(),
            self.order_service(),
            self.order_status_service(),
        )
    }

    /// Gets a reference to the database pool
    pub fn db_pool(&self) -> &Arc<DbPool> {
        &self.db_pool
    }

    /// Gets a reference to the event sender
    pub fn event_sender(&self) -> &EventSender {
        &self.event_sender
    }
}

/// Service container holding all service instances
#[derive(Clone)]
pub struct ServiceContainer {
    pub inventory: Arc<InventoryService>,
    pub orders: Arc<OrderService>,
    pub order_status: Arc<OrderStatusService>,
}

impl ServiceContainer {
    /// Creates a new service container with all services initialized
    pub fn new(factory: &ServiceFactory) -> Self {
        let (inventory, orders, order_status) = factory.create_all();

        Self {
            inventory: Arc::new(inventory),
            orders: Arc::new(orders),
            order_status: Arc::new(order_status),
        }
    }
}
