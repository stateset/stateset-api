use std::sync::Arc;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait, TransactionTrait,
};
use tracing::{error, info, instrument};
use uuid::Uuid;

use crate::{
    entities::order::{ActiveModel as OrderActiveModel, Entity as OrderEntity, Model as OrderModel},
    errors::ServiceError,
};

// Valid order statuses
const VALID_STATUSES: &[&str] = &[
    "pending",
    "processing",
    "shipped",
    "delivered",
    "cancelled",
    "refunded",
    "on_hold",
    "failed",
];

#[derive(Clone)]
pub struct OrderStatusService {
	db: Arc<DatabaseConnection>,
}

impl OrderStatusService {
	pub fn new(db: Arc<DatabaseConnection>) -> Self {
		Self { db }
	}

	/// Updates the status of an order with validation
	#[instrument(skip(self), fields(order_id = %order_id, new_status = %new_status))]
	pub async fn update_status(&self, order_id: Uuid, new_status: String) -> Result<OrderModel, ServiceError> {
		// Validate the new status
		if !VALID_STATUSES.contains(&new_status.as_str()) {
			error!("Invalid order status: {}", new_status);
			return Err(ServiceError::ValidationError(
				format!("Invalid status: {}. Valid statuses are: {:?}", new_status, VALID_STATUSES)
			));
		}

		let db = &*self.db;
		let txn = db.begin().await.map_err(|e| {
			error!("Failed to begin transaction: {}", e);
			ServiceError::DatabaseError(e.into())
		})?;

		// Fetch the current order
		let order = OrderEntity::find_by_id(order_id)
			.one(&txn)
			.await
			.map_err(|e| {
				error!("Failed to fetch order {}: {}", order_id, e);
				ServiceError::DatabaseError(e.into())
			})?
			.ok_or_else(|| {
				error!("Order {} not found", order_id);
				ServiceError::NotFound(format!("Order {} not found", order_id))
			})?;

		let old_status = order.status.clone();

		// Validate status transition
		if !self.is_valid_transition(&old_status, &new_status) {
			error!("Invalid status transition from {} to {}", old_status, new_status);
			return Err(ServiceError::ValidationError(
				format!("Cannot transition from status '{}' to '{}'", old_status, new_status)
			));
		}

		// Update the order
		let mut active: OrderActiveModel = order.into();
		active.status = Set(new_status.clone());
		active.updated_at = Set(Some(Utc::now()));
		let current_version = active.version.as_ref();
		active.version = Set(current_version + 1);
		
		let updated = active.update(&txn).await.map_err(|e| {
			error!("Failed to update order {} status: {}", order_id, e);
			ServiceError::DatabaseError(e.into())
		})?;

		txn.commit().await.map_err(|e| {
			error!("Failed to commit transaction for order {}: {}", order_id, e);
			ServiceError::DatabaseError(e.into())
		})?;
		
		info!(
			"Order {} status updated from '{}' to '{}'",
			order_id, old_status, new_status
		);

		Ok(updated)
	}

	/// Validates if a status transition is allowed
	fn is_valid_transition(&self, from_status: &str, to_status: &str) -> bool {
		match (from_status, to_status) {
			// From pending
			("pending", "processing") => true,
			("pending", "cancelled") => true,
			("pending", "on_hold") => true,
			
			// From processing
			("processing", "shipped") => true,
			("processing", "cancelled") => true,
			("processing", "on_hold") => true,
			("processing", "failed") => true,
			
			// From shipped
			("shipped", "delivered") => true,
			("shipped", "returned") => true,
			
			// From delivered
			("delivered", "refunded") => true,
			
			// From on_hold
			("on_hold", "processing") => true,
			("on_hold", "cancelled") => true,
			
			// From cancelled
			("cancelled", "refunded") => true,
			
			// From failed
			("failed", "processing") => true,
			("failed", "cancelled") => true,
			
			// Allow transitioning to the same status (no-op)
			_ if from_status == to_status => true,
			
			// All other transitions are invalid
			_ => false,
		}
	}

	/// Gets the current status of an order
	#[instrument(skip(self), fields(order_id = %order_id))]
	pub async fn get_status(&self, order_id: Uuid) -> Result<String, ServiceError> {
		let db = &*self.db;
		
		let order = OrderEntity::find_by_id(order_id)
			.one(db)
			.await
			.map_err(|e| {
				error!("Failed to fetch order {}: {}", order_id, e);
				ServiceError::DatabaseError(e.into())
			})?
			.ok_or_else(|| {
				error!("Order {} not found", order_id);
				ServiceError::NotFound(format!("Order {} not found", order_id))
			})?;
		
		Ok(order.status)
	}

	/// Batch update status for multiple orders
	#[instrument(skip(self, order_ids), fields(count = order_ids.len()))]
	pub async fn batch_update_status(
		&self,
		order_ids: Vec<Uuid>,
		new_status: String,
	) -> Result<Vec<OrderModel>, ServiceError> {
		// Validate the new status
		if !VALID_STATUSES.contains(&new_status.as_str()) {
			return Err(ServiceError::ValidationError(
				format!("Invalid status: {}", new_status)
			));
		}

		let mut updated_orders = Vec::new();
		
		for order_id in order_ids {
			match self.update_status(order_id, new_status.clone()).await {
				Ok(order) => updated_orders.push(order),
				Err(e) => {
					error!("Failed to update order {} status: {}", order_id, e);
					// Continue with other orders even if one fails
				}
			}
		}
		
		info!("Batch updated {} orders to status '{}'", updated_orders.len(), new_status);
		
		Ok(updated_orders)
	}
} 