use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        order_entity::{self, Entity as Order},
        order_item_entity::{self, Entity as OrderItem},
        facility_entity::{self, Entity as Facility},
        inventory_item_entity::{self, Entity as InventoryItem},
        incoming_inventory_entity::{self, Entity as IncomingInventory},
    },
    services::geocoding::GeocodingService,
    ml::routing_model::RoutingModel,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;
use prometheus::IntCounter;
use lazy_static::lazy_static;
use chrono::{DateTime, Duration, Utc};

lazy_static! {
    static ref ORDERS_ROUTED: IntCounter = 
        IntCounter::new("orders_routed_total", "Total number of orders routed")
            .expect("metric can be created");

    static ref ORDER_ROUTING_FAILURES: IntCounter = 
        IntCounter::new("order_routing_failures_total", "Total number of failed order routings")
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

#[async_trait::async_trait]
impl Command for OrderRoutingCommand {
    type Result = OrderRoutingResult;

    #[instrument(skip(self, db_pool, event_sender, geocoding_service, routing_model))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
        geocoding_service: Arc<GeocodingService>,
        routing_model: Arc<RoutingModel>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        let (order, order_items) = self.fetch_order_and_items(db).await?;
        let facilities = self.fetch_facilities(db).await?;
        let facility_scores = self.score_facilities(db, &facilities, &order_items, &order, &geocoding_service, &routing_model).await?;
        let allocated_orders = self.allocate_inventory(db, order, order_items, facility_scores).await?;
        let updated_orders = self.update_orders(db, allocated_orders).await?;

        self.log_and_trigger_events(&event_sender, &updated_orders).await?;

        ORDERS_ROUTED.inc();

        Ok(OrderRoutingResult {
            original_order_id: self.order_id,
            routed_orders: updated_orders.into_iter().map(|o| RoutedOrder {
                id: o.id,
                facility_id: o.facility_id.unwrap(),
                item_count: 0, // You might want to calculate this based on the actual items
            }).collect(),
        })
    }
}

impl OrderRoutingCommand {
    async fn fetch_order_and_items(&self, db: &DatabaseConnection) -> Result<(order_entity::Model, Vec<order_item_entity::Model>), ServiceError> {
        let order = Order::find_by_id(self.order_id)
            .one(db)
            .await
            .map_err(|e| {
                ORDER_ROUTING_FAILURES.inc();
                let msg = format!("Failed to fetch order {}: {}", self.order_id, e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
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
                let msg = format!("Failed to fetch order items for order {}: {}", self.order_id, e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?;

        Ok((order, order_items))
    }

    async fn fetch_facilities(&self, db: &DatabaseConnection) -> Result<Vec<facility_entity::Model>, ServiceError> {
        Facility::find()
            .all(db)
            .await
            .map_err(|e| {
                ORDER_ROUTING_FAILURES.inc();
                let msg = format!("Failed to fetch facilities: {}", e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
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
            let score = self.score_facility(db, facility, order_items, order, geocoding_service, routing_model).await?;
            facility_scores.push(score);
        }
        facility_scores.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        Ok(facility_scores)
    }

    async fn score_facility(
        &self,
        db: &DatabaseConnection,
        facility: &facility_entity::Model,
        order_items: &[order_item_entity::Model],
        order: &order_entity::Model,
        geocoding_service: &GeocodingService,
        routing_model: &RoutingModel,
    ) -> Result<FacilityScore, ServiceError> {
        // ... (keep the existing logic for scoring facilities)
        // Remember to update any i32 IDs to Uuid
        todo!("Implement facility scoring logic")
    }

    async fn allocate_inventory(
        &self,
        db: &DatabaseConnection,
        original_order: order_entity::Model,
        order_items: Vec<order_item_entity::Model>,
        facility_scores: Vec<FacilityScore>,
    ) -> Result<Vec<order_entity::Model>, ServiceError> {
        // ... (keep the existing logic for allocating inventory)
        // Remember to update any i32 IDs to Uuid
        todo!("Implement inventory allocation logic")
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
                    order.facility_id = Set(order.facility_id);
                    let updated_order = order.update(txn).await.map_err(|e| {
                        let msg = format!("Failed to update order: {}", e);
                        error!("{}", msg);
                        ServiceError::DatabaseError(msg)
                    })?;
                    results.push(updated_order);
                }
                Ok(results)
            })
        }).await
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
                facility_id = %order.facility_id.unwrap(),
                "Order routed successfully"
            );

            event_sender
                .send(Event::OrderRouted(order.id, order.facility_id.unwrap()))
                .await
                .map_err(|e| {
                    ORDER_ROUTING_FAILURES.inc();
                    let msg = format!("Failed to send event for routed order: {}", e);
                    error!("{}", msg);
                    ServiceError::EventError(msg)
                })?;
        }
        Ok(())
    }
}