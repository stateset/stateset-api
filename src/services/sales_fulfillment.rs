use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    TransactionTrait,
};
use std::sync::Arc;
use tracing::{error, info, instrument};

use crate::{
    entities::{
        order_fulfillments::{self, Entity as OrderFulfillmentEntity},
        sales_order_header::{self, Entity as SalesOrderHeaderEntity},
        sales_order_line::{self, Entity as SalesOrderLineEntity},
    },
    errors::ServiceError,
    events::{Event, EventSender},
    services::inventory_sync::{InventorySyncService, TransactionType},
};

/// Sales order fulfillment service for shipping orders and updating inventory
#[derive(Clone)]
pub struct SalesFulfillmentService {
    db: Arc<DatabaseConnection>,
    inventory_sync: Arc<InventorySyncService>,
    event_sender: Option<EventSender>,
}

impl SalesFulfillmentService {
    pub fn new(
        db: Arc<DatabaseConnection>,
        inventory_sync: Arc<InventorySyncService>,
        event_sender: Option<EventSender>,
    ) -> Self {
        Self {
            db,
            inventory_sync,
            event_sender,
        }
    }

    /// Validates if an order can be fulfilled
    #[instrument(skip(self))]
    pub async fn validate_order_fulfillment(
        &self,
        header_id: i64,
    ) -> Result<FulfillmentValidation, ServiceError> {
        let db = &*self.db;

        // Get order header
        let header = SalesOrderHeaderEntity::find_by_id(header_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Sales order {} not found", header_id))
            })?;

        let location_id = header
            .location_id
            .ok_or_else(|| ServiceError::InvalidOperation("Order has no location".to_string()))?;

        // Get order lines
        let lines = SalesOrderLineEntity::find()
            .filter(sales_order_line::Column::HeaderId.eq(header_id))
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let mut can_fulfill = true;
        let mut shortages = Vec::new();
        let mut total_items = 0;
        let mut available_items = 0;

        for line in lines {
            total_items += 1;

            if let (Some(item_id), Some(quantity)) = (line.inventory_item_id, line.ordered_quantity)
            {
                let line_location = line.location_id.unwrap_or(location_id);

                // Check availability
                let available = self
                    .inventory_sync
                    .check_availability(item_id, line_location, quantity)
                    .await?;

                if available {
                    available_items += 1;
                } else {
                    can_fulfill = false;

                    // Get current balance to report shortage
                    if let Some(balance) = self
                        .inventory_sync
                        .get_inventory_balance(item_id, line_location)
                        .await?
                    {
                        shortages.push(LineShortage {
                            line_id: line.line_id,
                            item_id,
                            ordered_quantity: quantity,
                            available_quantity: balance.quantity_available,
                            shortage: quantity - balance.quantity_available,
                        });
                    } else {
                        shortages.push(LineShortage {
                            line_id: line.line_id,
                            item_id,
                            ordered_quantity: quantity,
                            available_quantity: Decimal::ZERO,
                            shortage: quantity,
                        });
                    }
                }
            }
        }

        Ok(FulfillmentValidation {
            can_fulfill,
            total_lines: total_items,
            available_lines: available_items,
            shortages,
        })
    }

    /// Allocates inventory for an order (reserves it)
    #[instrument(skip(self))]
    pub async fn allocate_order_inventory(&self, header_id: i64) -> Result<(), ServiceError> {
        let db = &*self.db;
        let txn = db.begin().await.map_err(|e| ServiceError::db_error(e))?;

        // Get order header
        let header = SalesOrderHeaderEntity::find_by_id(header_id)
            .one(&txn)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Sales order {} not found", header_id))
            })?;

        let location_id = header
            .location_id
            .ok_or_else(|| ServiceError::InvalidOperation("Order has no location".to_string()))?;

        // Get order lines
        let lines = SalesOrderLineEntity::find()
            .filter(sales_order_line::Column::HeaderId.eq(header_id))
            .all(&txn)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        // Allocate inventory for each line
        for line in lines {
            if let (Some(item_id), Some(quantity)) = (line.inventory_item_id, line.ordered_quantity)
            {
                let line_location = line.location_id.unwrap_or(location_id);

                self.inventory_sync
                    .update_inventory_balance(
                        item_id,
                        line_location,
                        quantity,
                        TransactionType::Reservation,
                        Some(line.line_id),
                        Some("SALES_ORDER".to_string()),
                    )
                    .await?;
            }
        }

        // Update order status
        let mut active: sales_order_header::ActiveModel = header.into();
        active.status_code = Set(Some("ALLOCATED".to_string()));
        active.updated_at = Set(Utc::now().into());
        active
            .update(&txn)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        txn.commit().await.map_err(|e| ServiceError::db_error(e))?;

        info!("Inventory allocated for sales order {}", header_id);

        Ok(())
    }

    /// Fulfills a sales order line (ships it and deducts inventory)
    #[instrument(skip(self))]
    pub async fn fulfill_order_line(
        &self,
        header_id: i64,
        line_id: i64,
        shipped_quantity: Decimal,
        shipped_date: NaiveDate,
    ) -> Result<order_fulfillments::Model, ServiceError> {
        let db = &*self.db;
        let txn = db.begin().await.map_err(|e| ServiceError::db_error(e))?;

        // Get order line
        let line = SalesOrderLineEntity::find_by_id(line_id)
            .one(&txn)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Sales order line {} not found", line_id))
            })?;

        // Verify line belongs to the header
        if line.header_id != Some(header_id) {
            return Err(ServiceError::InvalidOperation(format!(
                "Line {} does not belong to order {}",
                line_id, header_id
            )));
        }

        let item_id = line
            .inventory_item_id
            .ok_or_else(|| ServiceError::InvalidOperation("Order line has no item".to_string()))?;
        let ordered_quantity = line.ordered_quantity.unwrap_or(Decimal::ZERO);

        // Get location from line or header
        let header = SalesOrderHeaderEntity::find_by_id(header_id)
            .one(&txn)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Sales order {} not found", header_id))
            })?;

        let location_id = line
            .location_id
            .or(header.location_id)
            .ok_or_else(|| ServiceError::InvalidOperation("No location specified".to_string()))?;

        // Check if already fulfilled
        let existing_fulfillments = OrderFulfillmentEntity::find()
            .filter(order_fulfillments::Column::SalesOrderLineId.eq(line_id))
            .all(&txn)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let total_fulfilled: Decimal = existing_fulfillments
            .iter()
            .filter_map(|f| {
                // Parse quantity from a field if it exists in your entity
                // For now, assuming full quantity per fulfillment
                Some(ordered_quantity)
            })
            .sum();

        if total_fulfilled + shipped_quantity > ordered_quantity {
            return Err(ServiceError::InvalidOperation(
                format!("Cannot ship more than ordered. Ordered: {}, Already shipped: {}, Trying to ship: {}",
                    ordered_quantity, total_fulfilled, shipped_quantity)
            ));
        }

        // Create fulfillment record
        let fulfillment = order_fulfillments::ActiveModel {
            fulfillment_id: Set(0), // Auto-generated
            sales_order_header_id: Set(Some(header_id)),
            sales_order_line_id: Set(Some(line_id)),
            shipped_date: Set(Some(shipped_date)),
            released_status: Set(Some("SHIPPED".to_string())),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        };

        let created = fulfillment.insert(&txn).await.map_err(|e| {
            error!("Failed to create fulfillment: {}", e);
            ServiceError::db_error(e)
        })?;

        // Deduct inventory
        self.inventory_sync
            .update_inventory_balance(
                item_id,
                location_id,
                -shipped_quantity,
                TransactionType::SalesOrder,
                Some(created.fulfillment_id),
                Some("SALES_FULFILLMENT".to_string()),
            )
            .await?;

        // Release any remaining allocation
        self.inventory_sync
            .update_inventory_balance(
                item_id,
                location_id,
                shipped_quantity,
                TransactionType::ReleaseReservation,
                Some(line_id),
                Some("SALES_ORDER".to_string()),
            )
            .await?;

        // Update line status
        let mut active_line: sales_order_line::ActiveModel = line.into();
        if total_fulfilled + shipped_quantity >= ordered_quantity {
            active_line.line_status = Set(Some("SHIPPED".to_string()));
        } else {
            active_line.line_status = Set(Some("PARTIALLY_SHIPPED".to_string()));
        }
        active_line.updated_at = Set(Utc::now().into());
        active_line
            .update(&txn)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        // Check if entire order is fulfilled
        self.update_order_status(&txn, header_id).await?;

        txn.commit().await.map_err(|e| ServiceError::db_error(e))?;

        // Send event
        if let Some(sender) = &self.event_sender {
            let _ = sender
                .send(Event::SalesOrderFulfilled {
                    order_id: header_id,
                    line_id,
                    item_id,
                    quantity: shipped_quantity,
                })
                .await;
        }

        info!(
            "Sales order line {} fulfilled: {} units of item {}",
            line_id, shipped_quantity, item_id
        );

        Ok(created)
    }

    /// Updates order header status based on line statuses
    async fn update_order_status(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        header_id: i64,
    ) -> Result<(), ServiceError> {
        let lines = SalesOrderLineEntity::find()
            .filter(sales_order_line::Column::HeaderId.eq(header_id))
            .all(txn)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let mut all_shipped = true;
        let mut any_shipped = false;

        for line in lines {
            match line.line_status.as_deref() {
                Some("SHIPPED") => any_shipped = true,
                Some("PARTIALLY_SHIPPED") => {
                    any_shipped = true;
                    all_shipped = false;
                }
                _ => all_shipped = false,
            }
        }

        let new_status = if all_shipped {
            "SHIPPED"
        } else if any_shipped {
            "PARTIALLY_SHIPPED"
        } else {
            return Ok(());
        };

        let header = SalesOrderHeaderEntity::find_by_id(header_id)
            .one(txn)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Sales order {} not found", header_id))
            })?;

        let mut active: sales_order_header::ActiveModel = header.into();
        active.status_code = Set(Some(new_status.to_string()));
        active.updated_at = Set(Utc::now().into());
        active
            .update(txn)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        Ok(())
    }

    /// Processes a sales return (adds inventory back)
    #[instrument(skip(self))]
    pub async fn process_return(
        &self,
        fulfillment_id: i64,
        return_quantity: Decimal,
        location_id: i32,
        reason: String,
    ) -> Result<(), ServiceError> {
        let db = &*self.db;

        // Get fulfillment
        let fulfillment = OrderFulfillmentEntity::find_by_id(fulfillment_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Fulfillment {} not found", fulfillment_id))
            })?;

        let line_id = fulfillment
            .sales_order_line_id
            .ok_or_else(|| ServiceError::InvalidOperation("Fulfillment has no line".to_string()))?;

        // Get order line
        let line = SalesOrderLineEntity::find_by_id(line_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Sales order line {} not found", line_id))
            })?;

        let item_id = line
            .inventory_item_id
            .ok_or_else(|| ServiceError::InvalidOperation("Order line has no item".to_string()))?;

        // Add inventory back
        self.inventory_sync
            .update_inventory_balance(
                item_id,
                location_id,
                return_quantity,
                TransactionType::SalesReturn,
                Some(fulfillment_id),
                Some(format!("RETURN: {}", reason)),
            )
            .await?;

        // Send event
        if let Some(sender) = &self.event_sender {
            let _ = sender
                .send(Event::SalesOrderReturned {
                    fulfillment_id,
                    item_id,
                    quantity: return_quantity,
                    reason,
                })
                .await;
        }

        info!(
            "Sales return processed: {} units of item {} returned",
            return_quantity, item_id
        );

        Ok(())
    }

    /// Gets fulfillment status for an order
    #[instrument(skip(self))]
    pub async fn get_order_fulfillment_status(
        &self,
        header_id: i64,
    ) -> Result<OrderFulfillmentStatus, ServiceError> {
        let db = &*self.db;

        // Get order lines
        let lines = SalesOrderLineEntity::find()
            .filter(sales_order_line::Column::HeaderId.eq(header_id))
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let mut total_lines = 0;
        let mut shipped_lines = 0;
        let mut partially_shipped_lines = 0;

        for line in lines {
            total_lines += 1;
            match line.line_status.as_deref() {
                Some("SHIPPED") => shipped_lines += 1,
                Some("PARTIALLY_SHIPPED") => partially_shipped_lines += 1,
                _ => {}
            }
        }

        let status = if shipped_lines == total_lines {
            "FULLY_SHIPPED"
        } else if shipped_lines > 0 || partially_shipped_lines > 0 {
            "PARTIALLY_SHIPPED"
        } else {
            "NOT_SHIPPED"
        };

        Ok(OrderFulfillmentStatus {
            header_id,
            total_lines,
            shipped_lines,
            partially_shipped_lines,
            pending_lines: total_lines - shipped_lines - partially_shipped_lines,
            status: status.to_string(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct FulfillmentValidation {
    pub can_fulfill: bool,
    pub total_lines: i32,
    pub available_lines: i32,
    pub shortages: Vec<LineShortage>,
}

#[derive(Debug, Clone)]
pub struct LineShortage {
    pub line_id: i64,
    pub item_id: i64,
    pub ordered_quantity: Decimal,
    pub available_quantity: Decimal,
    pub shortage: Decimal,
}

#[derive(Debug, Clone)]
pub struct OrderFulfillmentStatus {
    pub header_id: i64,
    pub total_lines: i32,
    pub shipped_lines: i32,
    pub partially_shipped_lines: i32,
    pub pending_lines: i32,
    pub status: String,
}
