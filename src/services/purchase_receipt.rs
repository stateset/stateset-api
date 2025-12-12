use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    TransactionTrait,
};
use std::sync::Arc;
use tracing::{error, info, instrument};

use crate::{
    entities::{
        po_receipt_headers::{self, Entity as ReceiptHeaderEntity},
        po_receipt_lines::{self, Entity as ReceiptLineEntity},
        purchase_order_lines::{self, Entity as POLineEntity},
    },
    errors::ServiceError,
    events::{Event, EventSender},
    services::inventory_sync::{InventorySyncService, TransactionType},
};

/// Purchase order receipt service for receiving goods and updating inventory
#[derive(Clone)]
pub struct PurchaseReceiptService {
    db: Arc<DatabaseConnection>,
    inventory_sync: Arc<InventorySyncService>,
    event_sender: Option<EventSender>,
}

impl PurchaseReceiptService {
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

    /// Creates a new purchase receipt header
    #[instrument(skip(self))]
    pub async fn create_receipt_header(
        &self,
        receipt_num: String,
        vendor_id: i64,
        shipment_num: Option<String>,
        receipt_source: Option<String>,
    ) -> Result<po_receipt_headers::Model, ServiceError> {
        let db = &*self.db;

        let receipt = po_receipt_headers::ActiveModel {
            shipment_header_id: Set(0), // Auto-generated
            receipt_num: Set(receipt_num.clone()),
            vendor_id: Set(Some(vendor_id)),
            shipment_num: Set(shipment_num),
            receipt_source: Set(receipt_source),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        };

        let created = receipt.insert(db).await.map_err(|e| {
            error!("Failed to create receipt header: {}", e);
            ServiceError::db_error(e)
        })?;

        info!("Receipt header created: {}", receipt_num);
        Ok(created)
    }

    /// Receives a purchase order line and updates inventory
    #[instrument(skip(self))]
    pub async fn receive_po_line(
        &self,
        shipment_header_id: i64,
        po_header_id: i64,
        po_line_id: i64,
        quantity_received: Decimal,
        location_id: i32,
    ) -> Result<po_receipt_lines::Model, ServiceError> {
        let db = &*self.db;
        let txn = db.begin().await.map_err(|e| ServiceError::db_error(e))?;

        // Verify receipt header exists
        let _header = ReceiptHeaderEntity::find_by_id(shipment_header_id)
            .one(&txn)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Receipt header {} not found", shipment_header_id))
            })?;

        // Get PO line details
        let po_line = POLineEntity::find_by_id(po_line_id)
            .one(&txn)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| ServiceError::NotFound(format!("PO line {} not found", po_line_id)))?;

        let item_id = po_line
            .item_id
            .ok_or_else(|| ServiceError::InvalidOperation("PO line has no item".to_string()))?;

        // Validate quantity doesn't exceed ordered quantity
        let ordered_quantity = po_line.quantity.unwrap_or(Decimal::ZERO);

        // Get previously received quantity for this PO line
        let previous_receipts = ReceiptLineEntity::find()
            .filter(po_receipt_lines::Column::PoLineId.eq(po_line_id))
            .all(&txn)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let total_received: Decimal = previous_receipts
            .iter()
            .map(|r| r.quantity_received.unwrap_or(Decimal::ZERO))
            .sum();

        if total_received + quantity_received > ordered_quantity {
            return Err(ServiceError::InvalidOperation(
                format!("Cannot receive more than ordered. Ordered: {}, Already received: {}, Trying to receive: {}",
                    ordered_quantity, total_received, quantity_received)
            ));
        }

        // Create receipt line
        let receipt_line = po_receipt_lines::ActiveModel {
            shipment_line_id: Set(0), // Auto-generated
            shipment_header_id: Set(Some(shipment_header_id)),
            item_id: Set(Some(item_id)),
            po_header_id: Set(Some(po_header_id)),
            po_line_id: Set(Some(po_line_id)),
            quantity_received: Set(Some(quantity_received)),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        };

        let created = receipt_line.insert(&txn).await.map_err(|e| {
            error!("Failed to create receipt line: {}", e);
            ServiceError::db_error(e)
        })?;

        // Update inventory
        self.inventory_sync
            .update_inventory_balance(
                item_id,
                location_id,
                quantity_received,
                TransactionType::PurchaseReceipt,
                Some(created.shipment_line_id),
                Some("PO_RECEIPT".to_string()),
            )
            .await?;

        txn.commit().await.map_err(|e| ServiceError::db_error(e))?;

        // Send event
        if let Some(sender) = &self.event_sender {
            sender
                .send_or_log(Event::PurchaseOrderReceived {
                    po_line_id,
                    item_id,
                    quantity: quantity_received,
                    location_id,
                })
                .await;
        }

        info!(
            "PO line {} received: {} units of item {} at location {}",
            po_line_id, quantity_received, item_id, location_id
        );

        Ok(created)
    }

    /// Receives multiple PO lines in a batch
    #[instrument(skip(self, lines))]
    pub async fn receive_multiple_lines(
        &self,
        shipment_header_id: i64,
        lines: Vec<ReceiptLineRequest>,
        location_id: i32,
    ) -> Result<Vec<po_receipt_lines::Model>, ServiceError> {
        let mut received_lines = Vec::new();

        for line in lines {
            let received = self
                .receive_po_line(
                    shipment_header_id,
                    line.po_header_id,
                    line.po_line_id,
                    line.quantity_received,
                    location_id,
                )
                .await?;

            received_lines.push(received);
        }

        Ok(received_lines)
    }

    /// Returns received goods back to vendor (updates inventory)
    #[instrument(skip(self))]
    pub async fn return_to_vendor(
        &self,
        receipt_line_id: i64,
        return_quantity: Decimal,
        location_id: i32,
        reason: String,
    ) -> Result<(), ServiceError> {
        let db = &*self.db;

        // Get receipt line
        let receipt_line = ReceiptLineEntity::find_by_id(receipt_line_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Receipt line {} not found", receipt_line_id))
            })?;

        let item_id = receipt_line.item_id.ok_or_else(|| {
            ServiceError::InvalidOperation("Receipt line has no item".to_string())
        })?;

        let received_quantity = receipt_line.quantity_received.unwrap_or(Decimal::ZERO);

        if return_quantity > received_quantity {
            return Err(ServiceError::InvalidOperation(format!(
                "Cannot return more than received. Received: {}, Trying to return: {}",
                received_quantity, return_quantity
            )));
        }

        // Update inventory (deduct returned quantity)
        self.inventory_sync
            .update_inventory_balance(
                item_id,
                location_id,
                -return_quantity,
                TransactionType::PurchaseReturn,
                Some(receipt_line_id),
                Some(format!("VENDOR_RETURN: {}", reason)),
            )
            .await?;

        // Send event
        if let Some(sender) = &self.event_sender {
            sender
                .send_or_log(Event::PurchaseOrderReturned {
                    receipt_line_id,
                    item_id,
                    quantity: return_quantity,
                    reason,
                })
                .await;
        }

        info!(
            "Returned {} units of item {} to vendor from receipt line {}",
            return_quantity, item_id, receipt_line_id
        );

        Ok(())
    }

    /// Gets receipt status for a PO
    #[instrument(skip(self))]
    pub async fn get_po_receipt_status(
        &self,
        po_header_id: i64,
    ) -> Result<POReceiptStatus, ServiceError> {
        let db = &*self.db;

        // Get PO lines
        let po_lines = POLineEntity::find()
            .filter(purchase_order_lines::Column::PoHeaderId.eq(po_header_id))
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let mut total_ordered = Decimal::ZERO;
        let mut total_received = Decimal::ZERO;

        for line in po_lines {
            let ordered = line.quantity.unwrap_or(Decimal::ZERO);
            total_ordered += ordered;

            // Get receipts for this line
            let receipts = ReceiptLineEntity::find()
                .filter(po_receipt_lines::Column::PoLineId.eq(line.po_line_id))
                .all(db)
                .await
                .map_err(|e| ServiceError::db_error(e))?;

            let line_received: Decimal = receipts
                .iter()
                .map(|r| r.quantity_received.unwrap_or(Decimal::ZERO))
                .sum();

            total_received += line_received;
        }

        let status = if total_received == Decimal::ZERO {
            "NOT_RECEIVED"
        } else if total_received < total_ordered {
            "PARTIALLY_RECEIVED"
        } else {
            "FULLY_RECEIVED"
        };

        Ok(POReceiptStatus {
            po_header_id,
            total_ordered,
            total_received,
            remaining: total_ordered - total_received,
            status: status.to_string(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct ReceiptLineRequest {
    pub po_header_id: i64,
    pub po_line_id: i64,
    pub quantity_received: Decimal,
}

#[derive(Debug, Clone)]
pub struct POReceiptStatus {
    pub po_header_id: i64,
    pub total_ordered: Decimal,
    pub total_received: Decimal,
    pub remaining: Decimal,
    pub status: String,
}
