use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Order, Facility, InventoryItem, IncomingInventory}};
use crate::events::{Event, EventSender};
use crate::services::geocoding::GeocodingService;
use crate::ml::routing_model::RoutingModel;
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use prometheus::IntCounter;
use chrono::{Utc, NaiveDateTime};

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
    pub order_id: i32,
}

#[derive(Debug)]
struct FacilityScore {
    facility_id: i32,
    score: f64,
    inventory_allocation: Vec<(i32, i32)>, // (product_id, quantity)
}

#[async_trait]
impl Command for OrderRoutingCommand {
    type Result = Vec<Order>; // Now returns multiple orders in case of splitting

    #[instrument(skip(db_pool, event_sender, geocoding_service, routing_model))]
    async fn execute(
        &self, 
        db_pool: Arc<DbPool>, 
        event_sender: Arc<EventSender>,
        geocoding_service: Arc<GeocodingService>,
        routing_model: Arc<RoutingModel>,
    ) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            ORDER_ROUTING_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        // Fetch the order and its items
        let order = orders::table
            .filter(orders::id.eq(self.order_id))
            .first::<Order>(&conn)
            .map_err(|e| {
                ORDER_ROUTING_FAILURES.inc();
                error!("Failed to fetch order {}: {}", self.order_id, e);
                ServiceError::DatabaseError
            })?;

        let order_items = order_items::table
            .filter(order_items::order_id.eq(self.order_id))
            .load::<OrderItem>(&conn)
            .map_err(|e| {
                ORDER_ROUTING_FAILURES.inc();
                error!("Failed to fetch order items for order {}: {}", self.order_id, e);
                ServiceError::DatabaseError
            })?;

        // Fetch all facilities
        let facilities = facilities::table
            .load::<Facility>(&conn)
            .map_err(|e| {
                ORDER_ROUTING_FAILURES.inc();
                error!("Failed to fetch facilities: {}", e);
                ServiceError::DatabaseError
            })?;

        // Score each facility
        let mut facility_scores = Vec::new();
        for facility in facilities {
            let score = self.score_facility(&conn, &facility, &order_items, &order, &geocoding_service, &routing_model).await?;
            facility_scores.push(score);
        }

        // Sort facilities by score in descending order
        facility_scores.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        // Attempt to allocate inventory and split order if necessary
        let mut allocated_orders = Vec::new();
        let mut remaining_items = order_items.clone();

        for facility_score in facility_scores {
            let allocated_items: Vec<_> = facility_score.inventory_allocation.iter()
                .filter_map(|&(product_id, quantity)| {
                    remaining_items.iter().position(|item| item.product_id == product_id && item.quantity <= quantity)
                })
                .collect();

            if !allocated_items.is_empty() {
                let mut new_order = order.clone();
                new_order.facility_id = Some(facility_score.facility_id);

                // Remove allocated items from remaining_items and add to new_order
                for &index in allocated_items.iter().rev() {
                    let item = remaining_items.remove(index);
                    diesel::insert_into(order_items::table)
                        .values(&OrderItem {
                            order_id: new_order.id,
                            product_id: item.product_id,
                            quantity: item.quantity,
                            ..item
                        })
                        .execute(&conn)
                        .map_err(|e| {
                            error!("Failed to insert new order item: {}", e);
                            ServiceError::DatabaseError
                        })?;
                }

                allocated_orders.push(new_order);
            }

            if remaining_items.is_empty() {
                break;
            }
        }

        if !remaining_items.is_empty() {
            ORDER_ROUTING_FAILURES.inc();
            error!("Unable to allocate all items for order {}", self.order_id);
            return Err(ServiceError::BusinessLogicError("Insufficient inventory across all facilities".to_string()));
        }

        // Update the orders in the database
        let updated_orders = conn.transaction::<_, diesel::result::Error, _>(|| {
            let mut results = Vec::new();
            for order in allocated_orders {
                let updated_order = diesel::update(orders::table)
                    .filter(orders::id.eq(order.id))
                    .set(orders::facility_id.eq(order.facility_id))
                    .get_result::<Order>(&conn)?;
                results.push(updated_order);
            }
            Ok(results)
        }).map_err(|e| {
            ORDER_ROUTING_FAILURES.inc();
            error!("Failed to update orders: {}", e);
            ServiceError::DatabaseError
        })?;

        // Trigger events for each routed order
        for order in &updated_orders {
            if let Err(e) = event_sender.send(Event::OrderRouted(order.id, order.facility_id.unwrap())).await {
                ORDER_ROUTING_FAILURES.inc();
                error!("Failed to send OrderRouted event for order {}: {}", order.id, e);
                return Err(ServiceError::EventError(e.to_string()));
            }
        }

        ORDERS_ROUTED.inc();

        info!(
            order_id = %self.order_id,
            num_splits = %updated_orders.len(),
            "Order routed successfully"
        );

        Ok(updated_orders)
    }
}

impl OrderRoutingCommand {
    async fn score_facility(
        &self, 
        conn: &PgConnection, 
        facility: &Facility, 
        order_items: &[OrderItem], 
        order: &Order,
        geocoding_service: &GeocodingService,
        routing_model: &RoutingModel,
    ) -> Result<FacilityScore, ServiceError> {
        let mut score = 0.0;
        let mut inventory_allocation = Vec::new();

        // Check current and incoming inventory availability
        for item in order_items {
            let current_inventory = inventory_items::table
                .filter(inventory_items::facility_id.eq(facility.id))
                .filter(inventory_items::product_id.eq(item.product_id))
                .first::<InventoryItem>(conn)
                .optional()
                .map_err(|e| {
                    error!("Failed to fetch inventory for product {} in facility {}: {}", item.product_id, facility.id, e);
                    ServiceError::DatabaseError
                })?;

            let incoming_inventory = incoming_inventory::table
                .filter(incoming_inventory::facility_id.eq(facility.id))
                .filter(incoming_inventory::product_id.eq(item.product_id))
                .filter(incoming_inventory::expected_arrival.le(Utc::now().naive_utc() + chrono::Duration::hours(24)))
                .select(diesel::dsl::sum(incoming_inventory::quantity))
                .first::<Option<i32>>(conn)
                .map_err(|e| {
                    error!("Failed to fetch incoming inventory for product {} in facility {}: {}", item.product_id, facility.id, e);
                    ServiceError::DatabaseError
                })?
                .unwrap_or(0);

            let total_available = current_inventory.map_or(0, |inv| inv.quantity) + incoming_inventory;

            if total_available >= item.quantity {
                score += 1.0;
                inventory_allocation.push((item.product_id, item.quantity));
            } else {
                score += total_available as f64 / item.quantity as f64;
                inventory_allocation.push((item.product_id, total_available));
            }
        }

        // Consider facility capacity
        let current_orders = orders::table
            .filter(orders::facility_id.eq(facility.id))
            .filter(orders::status.eq("Processing"))
            .count()
            .get_result::<i64>(conn)
            .map_err(|e| {
                error!("Failed to fetch current orders for facility {}: {}", facility.id, e);
                ServiceError::DatabaseError
            })?;

        let capacity_score = 1.0 - (current_orders as f64 / facility.max_daily_orders as f64);
        score += capacity_score;

        // Calculate actual shipping distance and cost
        let distance = geocoding_service.calculate_distance(order.shipping_address, facility.address).await?;
        let shipping_cost = facility.calculate_shipping_cost(distance);
        score += 1.0 / (1.0 + shipping_cost);

        // Consider facility operating hours and cut-off times
        let current_time = Utc::now().naive_utc();
        if !facility.is_operating_time(current_time) {
            score -= 0.5;
        }
        if facility.is_past_cutoff_time(current_time) {
            score -= 0.3;
        }

        // Use ML model for final score adjustment
        let ml_score = routing_model.predict_score(facility, order, &inventory_allocation).await?;
        score *= ml_score;

        Ok(FacilityScore {
            facility_id: facility.id,
            score,
            inventory_allocation,
        })
    }
}