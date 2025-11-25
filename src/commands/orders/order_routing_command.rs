use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    ml::routing_model::RoutingModel,
    models::{
        facility_entity::{self, Entity as Facility},
        incoming_inventory_entity::{self, Entity as IncomingInventory},
        inventory_item_entity::{self, Entity as InventoryItem},
        order_entity::{self, Entity as Order},
        order_item_entity::{self, Entity as OrderItem},
    },
    services::geocoding::GeocodingService,
};
use chrono::{DateTime, Duration, Utc};
use lazy_static::lazy_static;
use prometheus::IntCounter;
use sea_orm::{*, Set, TransactionError, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref ORDERS_ROUTED: IntCounter =
        IntCounter::new("orders_routed_total", "Total number of orders routed")
            .expect("metric can be created");
    static ref ORDER_ROUTING_FAILURES: IntCounter = IntCounter::new(
        "order_routing_failures_total",
        "Total number of failed order routings"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct OrderRoutingCommand {
    pub order_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderRoutingResult {
    pub original_order_id: Uuid,
    pub routed_orders: Vec<RoutedOrder>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RoutedOrder {
    pub id: Uuid,
    pub facility_id: Uuid,
    pub item_count: usize,
}

#[derive(Debug)]
struct FacilityScore {
    facility_id: Uuid,
    score: f64,
    inventory_allocation: Vec<(Uuid, i32)>, // (product_id, quantity)
}

#[derive(Debug, Clone)]
pub struct AllocatedOrder {
    pub order_id: Uuid,
    pub facility_id: Uuid,
}

#[async_trait::async_trait]
impl Command for OrderRoutingCommand {
    type Result = OrderRoutingResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        // TODO: Implement geocoding service and routing model
        let (order, order_items) = self.fetch_order_and_items(db).await?;
        let facilities = self.fetch_facilities(db).await?;
        
        // For now, just return a simple result
        Ok(OrderRoutingResult {
            original_order_id: self.order_id,
            routed_orders: vec![RoutedOrder {
                id: order.id,
                facility_id: Uuid::new_v4(),
                item_count: order_items.len(),
            }],
        })
    }
}

impl OrderRoutingCommand {
    async fn fetch_order_and_items(
        &self,
        db: &DatabaseConnection,
    ) -> Result<(order_entity::Model, Vec<order_item_entity::Model>), ServiceError> {
        let order = Order::find_by_id(self.order_id)
            .one(db)
            .await
            .map_err(|e| {
                ORDER_ROUTING_FAILURES.inc();
                let msg = format!("Failed to fetch order {}: {}", self.order_id, e);
                error!("{}", msg);
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                ORDER_ROUTING_FAILURES.inc();
                let msg = format!("Order {} not found", self.order_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        let order_items = OrderItem::find()
            .filter(order_item_entity::Column::OrderId.eq(self.order_id))
            .all(db)
            .await
            .map_err(|e| {
                ORDER_ROUTING_FAILURES.inc();
                let msg = format!(
                    "Failed to fetch order items for order {}: {}",
                    self.order_id, e
                );
                error!("{}", msg);
                ServiceError::db_error(e)
            })?;

        Ok((order, order_items))
    }

    async fn fetch_facilities(
        &self,
        db: &DatabaseConnection,
    ) -> Result<Vec<facility_entity::Model>, ServiceError> {
        Facility::find().all(db).await.map_err(|e| {
            ORDER_ROUTING_FAILURES.inc();
            let msg = format!("Failed to fetch facilities: {}", e);
            error!("{}", msg);
            ServiceError::db_error(e)
        })
    }

    async fn score_facilities(
        &self,
        db: &DatabaseConnection,
        facilities: &[facility_entity::Model],
        order_items: &[order_item_entity::Model],
        order: &order_entity::Model,
        geocoding_service: &GeocodingService,
        routing_model: &RoutingModel,
    ) -> Result<Vec<FacilityScore>, ServiceError> {
        let mut facility_scores = Vec::new();
        for facility in facilities {
            let score = self
                .score_facility(
                    db,
                    facility,
                    order_items,
                    order,
                    geocoding_service,
                    routing_model,
                )
                .await?;
            facility_scores.push(score);
        }
        facility_scores.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        Ok(facility_scores)
    }

    async fn score_facility(
        &self,
        db: &DatabaseConnection,
        facility: &facility_entity::Model,
        order_items: &[order_item_entity::Model],
        order: &order_entity::Model,
        _geocoding_service: &GeocodingService,
        _routing_model: &RoutingModel,
    ) -> Result<FacilityScore, ServiceError> {
        // Basic scoring implementation: score based on available inventory
        // Future: incorporate distance, capacity, and ML-based routing
        let mut inventory_allocation = Vec::new();
        let mut total_available = 0i64;

        for item in order_items {
            // Check inventory at this facility for each order item
            let inventory = InventoryItem::find()
                .filter(inventory_item_entity::Column::LocationId.eq(facility.id))
                .filter(inventory_item_entity::Column::ProductId.eq(item.product_id))
                .one(db)
                .await
                .map_err(ServiceError::db_error)?;

            if let Some(inv) = inventory {
                let available = inv.quantity_on_hand.unwrap_or(0);
                total_available += available;
                inventory_allocation.push((item.product_id, available as i32));
            }
        }

        // Simple score: ratio of available inventory to requested
        let total_requested: i64 = order_items.iter().map(|i| i.quantity as i64).sum();
        let score = if total_requested > 0 {
            (total_available as f64) / (total_requested as f64)
        } else {
            0.0
        };

        Ok(FacilityScore {
            facility_id: facility.id,
            score,
            inventory_allocation,
        })
    }

    async fn allocate_inventory(
        &self,
        db: &DatabaseConnection,
        original_order: order_entity::Model,
        order_items: Vec<order_item_entity::Model>,
        facility_scores: Vec<FacilityScore>,
    ) -> Result<Vec<order_entity::Model>, ServiceError> {
        // Allocate order to the facility with the highest score
        // Future: support splitting orders across multiple facilities
        if facility_scores.is_empty() {
            return Err(ServiceError::InvalidOperation(
                "No facilities available for order routing".to_string(),
            ));
        }

        // Get the best facility (already sorted by score descending)
        let best_facility = &facility_scores[0];

        if best_facility.score <= 0.0 {
            return Err(ServiceError::InvalidOperation(
                "No facility has sufficient inventory to fulfill this order".to_string(),
            ));
        }

        // For now, return the original order - facility assignment would be stored
        // in a separate order_allocation or order_routing table in production
        info!(
            order_id = %original_order.id,
            facility_id = %best_facility.facility_id,
            score = best_facility.score,
            "Order allocated to facility"
        );

        Ok(vec![original_order])
    }

    async fn update_orders(
        &self,
        db: &DatabaseConnection,
        allocated_orders: Vec<order_entity::Model>,
    ) -> Result<Vec<order_entity::Model>, ServiceError> {
        db.transaction::<_, Vec<order_entity::Model>, ServiceError>(|txn| {
            Box::pin(async move {
                let mut results = Vec::new();
                for order in allocated_orders {
                    let mut order: order_entity::ActiveModel = order.into();
                    // TODO: Add facility_id field to order entity or use a different approach
                    // order.facility_id = Set(order.facility_id);
                    let updated = order.update(txn).await.map_err(|e| {
                        let msg = format!("Failed to update order with facility: {}", e);
                        error!("{}", msg);
                        ServiceError::db_error(e)
                    })?;
                    results.push(updated);
                }
                Ok(results)
            })
        })
        .await
        .map_err(|e| {
            error!("Transaction failed for order routing: {}", e);
            match e {
                TransactionError::Connection(db_err) => ServiceError::db_error(db_err),
                TransactionError::Transaction(service_err) => service_err,
            }
        })
    }

    async fn log_and_trigger_events(
        &self,
        event_sender: &EventSender,
        updated_orders: &[order_entity::Model],
    ) -> Result<(), ServiceError> {
        for order in updated_orders {
            info!(
                original_order_id = %self.order_id,
                routed_order_id = %order.id,
                // TODO: Add facility_id to logging once available
                // facility_id = %order.facility_id.unwrap(),
                "Order routed successfully"
            );

            event_sender
                // TODO: Update event to use appropriate fields
                .send(Event::OrderUpdated(order.id))
                .await
                .map_err(|e| {
                    error!("Failed to send order routing event: {}", e);
                    ServiceError::EventError(e.to_string())
                })?;
        }
        Ok(())
    }

    async fn create_order_allocations(
        &self,
        db: &DatabaseConnection,
        allocated_orders: Vec<AllocatedOrder>,
    ) -> Result<Vec<order_entity::Model>, ServiceError> {
        db.transaction::<_, Vec<order_entity::Model>, ServiceError>(|txn| {
            Box::pin(async move {
                let mut results = Vec::new();
                for order in allocated_orders {
                    // TODO: Update order with facility assignment
                    // This would typically involve updating order status,
                    // assigned facility, etc.
                    
                    // For now, just fetch the order
                    let order_model = Order::find_by_id(order.order_id)
                        .one(txn)
                        .await
                        .map_err(|e| ServiceError::db_error(e))?
                        .ok_or_else(|| {
                            ServiceError::NotFound(format!(
                                "Order {} not found",
                                order.order_id
                            ))
                        })?;
                    
                    results.push(order_model);
                }
                Ok(results)
            })
        })
        .await
        .map_err(|e| {
            error!("Transaction failed for order allocation: {}", e);
            match e {
                TransactionError::Connection(db_err) => ServiceError::db_error(db_err),
                TransactionError::Transaction(service_err) => service_err,
            }
        })
    }
}
