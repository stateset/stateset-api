use std::sync::Arc;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
    PaginatorTrait, QueryFilter, Set, Statement,
};
use tracing::{error, info, instrument};
use uuid::Uuid;

use crate::{
    entities::inventory_items::{self, Entity as InventoryItemsEntity},
    errors::ServiceError,
    events::{Event, EventSender},
};

// Temporary command structures until commands module is re-enabled
#[derive(Debug, Clone)]
pub struct AdjustInventoryCommand {
    pub product_id: Option<Uuid>,
    pub location_id: Option<Uuid>,
    pub adjustment_quantity: Option<i32>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SetInventoryLevelsCommand {
    pub levels: Vec<(String, i32)>,
}

/// Service for managing inventory
#[derive(Clone)]
#[allow(dead_code)]
pub struct InventoryService {
    db_pool: Arc<DatabaseConnection>,
    event_sender: EventSender,
}

impl InventoryService {
    /// Creates a new inventory service instance
    pub fn new(db_pool: Arc<DatabaseConnection>, event_sender: EventSender) -> Self {
        Self {
            db_pool,
            event_sender,
        }
    }

    /// Adjusts inventory quantity for a product
    #[instrument(skip(self))]
    pub async fn adjust_inventory(
        &self,
        command: AdjustInventoryCommand,
    ) -> Result<(), ServiceError> {
        let product_id = command
            .product_id
            .ok_or_else(|| ServiceError::ValidationError("Product ID is required".to_string()))?;
        let location_id = command
            .location_id
            .ok_or_else(|| ServiceError::ValidationError("Location ID is required".to_string()))?;
        let adjustment_quantity = command.adjustment_quantity.ok_or_else(|| {
            ServiceError::ValidationError("Adjustment quantity is required".to_string())
        })?;

        // Get current inventory level
        let db = &*self.db_pool;
        let inventory = self.get_inventory(&product_id, &location_id).await?;
        let old_quantity = inventory.as_ref().map(|i| i.available).unwrap_or(0);
        let new_quantity = old_quantity + adjustment_quantity;

        if new_quantity < 0 {
            return Err(ServiceError::ValidationError(
                "Adjustment would result in negative inventory".to_string(),
            ));
        }

        // Update or create inventory record
        if let Some(inv) = inventory {
            let mut active: inventory_items::ActiveModel = inv.into();
            active.available = Set(new_quantity);
            active.updated_at = Set(Utc::now().naive_utc());
            active.update(db).await.map_err(ServiceError::db_error)?;
        } else {
            // Create new inventory record
            let new_inv = inventory_items::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                sku: Set(product_id.to_string()),
                warehouse: Set(location_id.to_string()),
                available: Set(new_quantity),
                allocated_quantity: Set(Some(0)),
                reserved_quantity: Set(Some(0)),
                unit_cost: Set(None),
                last_movement_date: Set(Some(Utc::now().naive_utc())),
                arrival_date: Set(Utc::now().date_naive()),
                created_at: Set(Utc::now().naive_utc()),
                updated_at: Set(Utc::now().naive_utc()),
            };
            new_inv.insert(db).await.map_err(ServiceError::db_error)?;
        }

        // Send event
        let event = Event::InventoryAdjusted {
            product_id,
            warehouse_id: location_id,
            old_quantity,
            new_quantity,
            reason_code: command
                .reason
                .unwrap_or_else(|| "MANUAL_ADJUSTMENT".to_string()),
            transaction_id: Uuid::new_v4(),
            reference_number: None,
        };

        self.event_sender
            .send(event)
            .await
            .map_err(|e| ServiceError::EventError(e.to_string()))?;

        // Outbox event: InventoryAdjusted (best-effort)
        let payload = serde_json::json!({
            "product_id": product_id.to_string(),
            "warehouse_id": location_id.to_string(),
            "old_quantity": old_quantity,
            "new_quantity": new_quantity,
            "reason": "MANUAL_ADJUSTMENT",
        });
        let _ =
            crate::events::outbox::enqueue(db, "inventory", None, "InventoryAdjusted", &payload)
                .await;

        info!(
            "Inventory adjusted for product {} at location {}: {} -> {}",
            product_id, location_id, old_quantity, new_quantity
        );

        Ok(())
    }

    /// Simple reservation that increments reserved_quantity if available
    #[instrument(skip(self))]
    pub async fn reserve_inventory_simple(
        &self,
        product_id: &Uuid,
        location_id: &Uuid,
        quantity: i32,
    ) -> Result<String, ServiceError> {
        if quantity <= 0 {
            return Err(ServiceError::ValidationError(
                "Reservation quantity must be positive".to_string(),
            ));
        }

        let db = &*self.db_pool;
        // Atomic update to avoid oversell: reserve only if available - reserved >= quantity
        let backend = db.get_database_backend();
        let now = Utc::now().naive_utc();
        // Use backend-appropriate placeholders
        let (sql, values) = match backend {
            DbBackend::Postgres => (
                r#"
                UPDATE inventory_items
                SET reserved_quantity = COALESCE(reserved_quantity, 0) + $3,
                    updated_at = $4
                WHERE sku = $1
                  AND warehouse = $2
                  AND (available - COALESCE(reserved_quantity, 0)) >= $3
                "#,
                vec![
                    product_id.to_string().into(),
                    location_id.to_string().into(),
                    quantity.into(),
                    now.into(),
                ],
            ),
            _ => (
                r#"
                UPDATE inventory_items
                SET reserved_quantity = COALESCE(reserved_quantity, 0) + ?,
                    updated_at = ?
                WHERE sku = ?
                  AND warehouse = ?
                  AND (available - COALESCE(reserved_quantity, 0)) >= ?
                "#,
                vec![
                    quantity.into(),
                    now.into(),
                    product_id.to_string().into(),
                    location_id.to_string().into(),
                    quantity.into(),
                ],
            ),
        };
        let stmt = Statement::from_sql_and_values(backend, sql, values);
        let res = db.execute(stmt).await.map_err(ServiceError::db_error)?;
        if res.rows_affected() == 0 {
            return Err(ServiceError::ValidationError(format!(
                "Insufficient inventory for reservation of {} units",
                quantity
            )));
        }

        // Outbox event: InventoryReserved (best-effort)
        let payload = serde_json::json!({
            "product_id": product_id.to_string(),
            "warehouse_id": location_id.to_string(),
            "quantity": quantity,
        });
        let _ =
            crate::events::outbox::enqueue(db, "inventory", None, "InventoryReserved", &payload)
                .await;

        let reservation_id = Uuid::new_v4().to_string();

        // Send reservation event
        let event = Event::InventoryReserved {
            product_id: *product_id,
            warehouse_id: *location_id,
            quantity,
            reference_id: Uuid::new_v4(),
            reference_type: "manual_reservation".to_string(),
            partial: false,
        };

        self.event_sender
            .send(event)
            .await
            .map_err(|e| ServiceError::EventError(e.to_string()))?;

        info!(
            "Reserved {} units of product {} at location {}, reservation ID: {}",
            quantity, product_id, location_id, reservation_id
        );

        Ok(reservation_id)
    }

    /// Releases reserved inventory
    #[instrument(skip(self))]
    pub async fn release_reservation(
        &self,
        product_id: &Uuid,
        location_id: &Uuid,
        quantity: i32,
    ) -> Result<(), ServiceError> {
        if quantity <= 0 {
            return Err(ServiceError::ValidationError(
                "Release quantity must be positive".to_string(),
            ));
        }

        let db = &*self.db_pool;
        let backend = db.get_database_backend();
        let now = Utc::now().naive_utc();
        // Atomic release: only release if reserved_quantity >= quantity
        let (sql, values) = match backend {
            DbBackend::Postgres => (
                r#"
                UPDATE inventory_items
                SET reserved_quantity = COALESCE(reserved_quantity, 0) - $3,
                    updated_at = $4
                WHERE sku = $1
                  AND warehouse = $2
                  AND COALESCE(reserved_quantity, 0) >= $3
                "#,
                vec![
                    product_id.to_string().into(),
                    location_id.to_string().into(),
                    quantity.into(),
                    now.into(),
                ],
            ),
            _ => (
                r#"
                UPDATE inventory_items
                SET reserved_quantity = COALESCE(reserved_quantity, 0) - ?,
                    updated_at = ?
                WHERE sku = ?
                  AND warehouse = ?
                  AND COALESCE(reserved_quantity, 0) >= ?
                "#,
                vec![
                    quantity.into(),
                    now.into(),
                    product_id.to_string().into(),
                    location_id.to_string().into(),
                    quantity.into(),
                ],
            ),
        };
        let stmt = Statement::from_sql_and_values(backend, sql, values);
        let res = db.execute(stmt).await.map_err(ServiceError::db_error)?;
        if res.rows_affected() == 0 {
            return Err(ServiceError::ValidationError(format!(
                "Cannot release {} units; not enough reserved",
                quantity
            )));
        }

        // Outbox event: InventoryDeallocated
        let payload = serde_json::json!({
            "product_id": product_id.to_string(),
            "warehouse_id": location_id.to_string(),
            "quantity": quantity,
        });
        let _ =
            crate::events::outbox::enqueue(db, "inventory", None, "InventoryDeallocated", &payload)
                .await;

        // Send deallocated event (closest to release)
        let event = Event::InventoryDeallocated {
            item_id: *product_id,
            quantity,
        };

        self.event_sender
            .send(event)
            .await
            .map_err(|e| ServiceError::EventError(e.to_string()))?;

        info!(
            "Released {} units of product {} at location {}",
            quantity, product_id, location_id
        );

        Ok(())
    }

    /// Sets inventory levels for multiple products
    #[instrument(skip(self))]
    pub async fn set_inventory_levels(
        &self,
        command: SetInventoryLevelsCommand,
    ) -> Result<(), ServiceError> {
        let db = &*self.db_pool;

        for (sku, new_level) in command.levels.iter() {
            if *new_level < 0 {
                return Err(ServiceError::ValidationError(format!(
                    "Invalid inventory level {} for SKU {}",
                    new_level, sku
                )));
            }

            // Update inventory level for each SKU
            let inv = InventoryItemsEntity::find()
                .filter(inventory_items::Column::Sku.eq(sku))
                .one(db)
                .await
                .map_err(ServiceError::db_error)?;

            if let Some(existing) = inv {
                let mut active: inventory_items::ActiveModel = existing.into();
                active.available = Set(*new_level);
                active.updated_at = Set(Utc::now().naive_utc());
                active.update(db).await.map_err(ServiceError::db_error)?;

                info!("Updated inventory level for SKU {} to {}", sku, new_level);
            } else {
                error!("SKU {} not found when setting inventory level", sku);
            }
        }

        Ok(())
    }

    /// Gets inventory level for a product at a location
    #[instrument(skip(self))]
    pub async fn get_inventory(
        &self,
        product_id: &Uuid,
        location_id: &Uuid,
    ) -> Result<Option<inventory_items::Model>, ServiceError> {
        let db = &*self.db_pool;

        let inventory = InventoryItemsEntity::find()
            .filter(inventory_items::Column::Sku.eq(product_id.to_string()))
            .filter(inventory_items::Column::Warehouse.eq(location_id.to_string()))
            .one(db)
            .await
            .map_err(ServiceError::db_error)?;

        Ok(inventory)
    }

    /// Checks if a product is in stock at a location
    #[instrument(skip(self))]
    pub async fn is_in_stock(
        &self,
        product_id: &Uuid,
        location_id: &Uuid,
        quantity: i32,
    ) -> Result<bool, ServiceError> {
        let inventory = self.get_inventory(product_id, location_id).await?;

        match inventory {
            Some(inv) => {
                let available = inv.available - inv.reserved_quantity.unwrap_or(0);
                Ok(available >= quantity)
            }
            None => Ok(false),
        }
    }

    /// Transfers inventory between locations
    #[instrument(skip(self))]
    pub async fn transfer_inventory(
        &self,
        product_id: &Uuid,
        from_location: &Uuid,
        to_location: &Uuid,
        quantity: i32,
    ) -> Result<(), ServiceError> {
        if quantity <= 0 {
            return Err(ServiceError::ValidationError(
                "Transfer quantity must be positive".to_string(),
            ));
        }

        // Check source inventory
        let source_inv = self
            .get_inventory(product_id, from_location)
            .await?
            .ok_or_else(|| {
                ServiceError::NotFound(format!(
                    "No inventory found for product {} at source location {}",
                    product_id, from_location
                ))
            })?;

        let available = source_inv.available - source_inv.reserved_quantity.unwrap_or(0);
        if available < quantity {
            return Err(ServiceError::ValidationError(format!(
                "Insufficient inventory at source: {} available, {} requested",
                available, quantity
            )));
        }

        // Adjust source inventory (decrease)
        self.adjust_inventory(AdjustInventoryCommand {
            product_id: Some(*product_id),
            location_id: Some(*from_location),
            adjustment_quantity: Some(-quantity),
            reason: Some(format!("Transfer to location {}", to_location)),
        })
        .await?;

        // Adjust destination inventory (increase)
        self.adjust_inventory(AdjustInventoryCommand {
            product_id: Some(*product_id),
            location_id: Some(*to_location),
            adjustment_quantity: Some(quantity),
            reason: Some(format!("Transfer from location {}", from_location)),
        })
        .await?;

        info!(
            "Transferred {} units of product {} from {} to {}",
            quantity, product_id, from_location, to_location
        );

        Ok(())
    }

    /// Lists all inventory items with pagination
    #[instrument(skip(self))]
    pub async fn list_inventory(
        &self,
        page: u64,
        limit: u64,
    ) -> Result<(Vec<inventory_items::Model>, u64), ServiceError> {
        if page == 0 {
            return Err(ServiceError::ValidationError(
                "Page number must be greater than 0".to_string(),
            ));
        }

        if limit == 0 || limit > 1000 {
            return Err(ServiceError::ValidationError(
                "Limit must be between 1 and 1000".to_string(),
            ));
        }

        let db = &*self.db_pool;

        // Create a paginator for the inventory items
        let paginator = InventoryItemsEntity::find().paginate(db, limit);

        // Get the total count of inventory items
        let total = paginator.num_items().await.map_err(|e| {
            error!("Failed to count inventory items: {}", e);
            ServiceError::InternalError(format!("Failed to count inventory items: {}", e))
        })?;

        // Get the requested page of inventory items (0-indexed)
        let items = paginator.fetch_page(page - 1).await.map_err(|e| {
            error!("Failed to fetch inventory items page {}: {}", page, e);
            ServiceError::InternalError(format!("Failed to fetch inventory items: {}", e))
        })?;

        Ok((items, total))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;
    use mockall::predicate::*;
    use std::str::FromStr;
    use tokio::sync::broadcast;

    mock! {
        pub Database {}
        impl Clone for Database {
            fn clone(&self) -> Self;
        }
    }

    // NOTE: This test is disabled because MockDatabase is no longer available in SeaORM 1.0.0
    // #[tokio::test]
    // async fn test_adjust_inventory() {
    //     // Setup
    //     let (event_sender, _) = broadcast::channel(10);
    //     let event_sender = Arc::new(event_sender);
    //     let db_pool = Arc::new(MockDatabase::new());
    //     let redis_client = Arc::new(redis::Client::open("redis://localhost").unwrap());
    //     let message_queue = Arc::new(crate::message_queue::MockMessageQueue::new());
    //     let circuit_breaker = Arc::new(CircuitBreaker::new(
    //         5,
    //         std::time::Duration::from_secs(60),
    //         1,
    //     ));
    //     let logger = slog::Logger::root(slog::Discard, slog::o!());

    //     let service = InventoryService::new(
    //         db_pool,
    //         event_sender,
    //         redis_client,
    //         message_queue,
    //         circuit_breaker,
    //         logger,
    //     );

    //     // Test data
    //     let product_id = Uuid::from_str("00000000-0000-0000-0000-000000000001").unwrap();
    //     let location_id = Uuid::from_str("00000000-0000-0000-0000-000000000002").unwrap();

    //     let command = AdjustInventoryCommand {
    //         product_id,
    //         location_id,
    //         adjustment: 10,
    //         reason: "Inventory count".to_string(),
    //     };

    //     // Execute
    //     let result = service.adjust_inventory(command).await;

    //     // Assert
    //     assert!(result.is_err()); // Will fail because we're using mock DB
    // }
}
