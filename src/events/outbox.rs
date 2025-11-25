use crate::errors::ServiceError;
use crate::events::{Event, EventSender};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement};
use serde_json::Value;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub enum OutboxStatus {
    Pending,
    Processing,
    Delivered,
    Failed,
}

impl OutboxStatus {
    fn as_str(&self) -> &'static str {
        match self {
            OutboxStatus::Pending => "pending",
            OutboxStatus::Processing => "processing",
            OutboxStatus::Delivered => "delivered",
            OutboxStatus::Failed => "failed",
        }
    }
}

/// Enqueue a domain event into the outbox table. Use inside the same transaction as your write (future work).
pub async fn enqueue(
    db: &impl ConnectionTrait,
    aggregate_type: &str,
    aggregate_id: Option<Uuid>,
    event_type: &str,
    payload: &Value,
) -> Result<(), ServiceError> {
    if db.get_database_backend() != DbBackend::Postgres {
        debug!(
            "outbox enqueue skipped for non-Postgres backend (aggregate_type={}, event_type={})",
            aggregate_type, event_type
        );
        return Ok(());
    }

    let id = Uuid::new_v4();
    let sql = r#"INSERT INTO outbox_events
        (id, aggregate_type, aggregate_id, event_type, payload, status, attempts, created_at)
        VALUES ($1, $2, $3, $4, $5::jsonb, 'pending', 0, NOW())"#;
    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        vec![
            id.into(),
            aggregate_type.into(),
            aggregate_id.map(|v| v.into()).unwrap_or(Value::Null.into()),
            event_type.into(),
            payload.clone().into(),
        ],
    );
    db.execute(stmt).await.map_err(ServiceError::db_error)?;
    info!(
        "enqueued outbox event {} type={} agg={}",
        id, event_type, aggregate_type
    );
    Ok(())
}

/// Background worker to poll and dispatch outbox events via in-process EventSender.
pub async fn start_worker(db: Arc<DatabaseConnection>, sender: EventSender) {
    if db.get_database_backend() != DbBackend::Postgres {
        info!(
            "Outbox worker disabled for {:?} backend; relying on direct event emission",
            db.get_database_backend()
        );
        return;
    }

    tokio::spawn(async move {
        loop {
            if let Err(e) = drain_once(&db, &sender, 50).await {
                error!("outbox worker error: {}", e);
            }
            sleep(Duration::from_millis(500)).await;
        }
    });
}

async fn drain_once(
    db: &DatabaseConnection,
    sender: &EventSender,
    batch_size: i64,
) -> Result<(), ServiceError> {
    const MAX_ATTEMPTS: i32 = 8;
    const BASE_BACKOFF_SECS: u64 = 2; // exponential backoff base
                                      // Mark a batch as processing and return them (advisory lock-like behavior)
    let sql_claim = r#"
        WITH cte AS (
            SELECT id FROM outbox_events
            WHERE status = 'pending' AND available_at <= NOW()
            ORDER BY created_at ASC
            FOR UPDATE SKIP LOCKED
            LIMIT $1
        )
        UPDATE outbox_events o
        SET status = 'processing', updated_at = NOW(), attempts = o.attempts + 1
        FROM cte
        WHERE o.id = cte.id
        RETURNING o.id, o.event_type, o.payload
    "#;
    let stmt =
        Statement::from_sql_and_values(DbBackend::Postgres, sql_claim, vec![batch_size.into()]);
    let rows: Vec<QueryResult> = db.query_all(stmt).await.map_err(ServiceError::db_error)?;

    for row in rows {
        let id: Uuid = row.try_get("", "id").unwrap_or_default();
        let et: String = row.try_get("", "event_type").unwrap_or_default();
        let payload: Value = row.try_get("", "payload").unwrap_or(Value::Null);

        // Best-effort: map event_type+payload to our internal Event
        let evt =
            map_to_event(&et, &payload).unwrap_or_else(|| Event::with_data(format!("{}", et)));

        let dispatch_ok = sender.send(evt).await.is_ok();
        if dispatch_ok {
            let sql_update = r#"UPDATE outbox_events SET status = 'delivered', processed_at = NOW(), updated_at = NOW(), error_message = NULL WHERE id = $1"#;
            let stmt_upd =
                Statement::from_sql_and_values(DbBackend::Postgres, sql_update, vec![id.into()]);
            if let Err(e) = db.execute(stmt_upd).await {
                warn!("failed updating outbox {}: {}", id, e);
            }
        } else {
            // Check attempts and schedule retry using exponential backoff with jitter
            let sql_attempts = r#"SELECT attempts FROM outbox_events WHERE id = $1"#;
            let row = db
                .query_one(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    sql_attempts,
                    vec![id.into()],
                ))
                .await
                .map_err(ServiceError::db_error)?;
            let attempts: i32 = row
                .and_then(|r| r.try_get("", "attempts").ok())
                .unwrap_or(1);
            if attempts < MAX_ATTEMPTS {
                let backoff = (BASE_BACKOFF_SECS.saturating_pow(attempts as u32)) as u64;
                let now_ms = chrono::Utc::now().timestamp_millis() as u64;
                let jitter = now_ms % 1000; // ms
                let sql_retry = r#"UPDATE outbox_events SET status = 'pending', available_at = NOW() + make_interval(secs := $2::int) + ($3::int * interval '1 millisecond'), updated_at = NOW(), error_message = 'send failed' WHERE id = $1"#;
                let stmt_retry = Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    sql_retry,
                    vec![id.into(), (backoff as i64).into(), (jitter as i64).into()],
                );
                if let Err(e) = db.execute(stmt_retry).await {
                    warn!("failed scheduling retry for outbox {}: {}", id, e);
                }
            } else {
                let sql_fail = r#"UPDATE outbox_events SET status = 'failed', updated_at = NOW(), error_message = 'max attempts exceeded' WHERE id = $1"#;
                let stmt_fail =
                    Statement::from_sql_and_values(DbBackend::Postgres, sql_fail, vec![id.into()]);
                if let Err(e) = db.execute(stmt_fail).await {
                    warn!("failed marking outbox {} failed: {}", id, e);
                }
            }
        }
    }
    Ok(())
}

fn map_to_event(event_type: &str, payload: &Value) -> Option<Event> {
    match event_type {
        // Map a few known event types; expand over time
        "OrderCreated" => payload
            .get("order_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .map(Event::OrderCreated),
        "ReturnCreated" => payload
            .get("return_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .map(Event::ReturnCreated),
        "ReturnCompleted" => payload
            .get("return_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .map(Event::ReturnUpdated),
        "InventoryReserved" => {
            let pid = payload
                .get("product_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())?;
            let wid = payload
                .get("warehouse_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())?;
            let qty = payload
                .get("quantity")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            Some(Event::InventoryReserved {
                warehouse_id: wid,
                product_id: pid,
                quantity: qty,
                reference_id: Uuid::new_v4(),
                reference_type: "outbox".to_string(),
                partial: false,
            })
        }
        "InventoryDeallocated" => {
            let pid = payload
                .get("product_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())?;
            let qty = payload
                .get("quantity")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            Some(Event::InventoryDeallocated {
                item_id: pid,
                quantity: qty,
            })
        }
        "InventoryAdjusted" => {
            let pid = payload
                .get("product_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())?;
            let wid = payload
                .get("warehouse_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())?;
            let old_q = payload
                .get("old_quantity")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            let new_q = payload
                .get("new_quantity")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            Some(Event::InventoryAdjusted {
                warehouse_id: wid,
                product_id: pid,
                old_quantity: old_q,
                new_quantity: new_q,
                reason_code: "OUTBOX".to_string(),
                transaction_id: Uuid::new_v4(),
                reference_number: None,
            })
        }
        "ShipmentCreated" => payload
            .get("shipment_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .map(Event::ShipmentCreated),
        "ShipmentUpdated" => payload
            .get("shipment_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .map(Event::ShipmentUpdated),
        "ShipmentDelivered" => payload
            .get("shipment_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .map(Event::ShipmentDelivered),
        "CarrierAssignedToShipment" => None, // Non-standard id type in some paths; treat as integration-only
        "WarrantyCreated" => payload
            .get("warranty_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .map(Event::WarrantyCreated),
        "WarrantyClaimed" => {
            let claim_id = payload
                .get("claim_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())?;
            let warranty_id = payload
                .get("warranty_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())?;
            Some(Event::WarrantyClaimed {
                claim_id,
                warranty_id,
            })
        }
        "WarrantyClaimApproved" => {
            let claim_id = payload
                .get("claim_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())?;
            let warranty_id = payload
                .get("warranty_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())?;
            let resolution = payload
                .get("resolution")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let notes = payload
                .get("notes")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Some(Event::WarrantyClaimApproved {
                claim_id,
                warranty_id,
                resolution,
                notes,
            })
        }
        "WarrantyClaimRejected" => {
            let claim_id = payload
                .get("claim_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())?;
            let warranty_id = payload
                .get("warranty_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())?;
            let reason = payload
                .get("reason")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())?;
            let notes = payload
                .get("notes")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Some(Event::WarrantyClaimRejected {
                claim_id,
                warranty_id,
                reason,
                notes,
            })
        }
        "PaymentSucceeded" => payload
            .get("id")
            .and_then(|v| v.as_str())
            .and_then(|_| Some(Event::PaymentCaptured(Uuid::new_v4()))),
        "PaymentRefunded" => payload
            .get("id")
            .and_then(|v| v.as_str())
            .and_then(|_| Some(Event::PaymentRefunded(Uuid::new_v4()))),
        "PaymentFailed" => payload
            .get("id")
            .and_then(|v| v.as_str())
            .and_then(|_| Some(Event::PaymentFailed(Uuid::new_v4()))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_warranty_claimed_event() {
        let claim_id = Uuid::new_v4();
        let warranty_id = Uuid::new_v4();
        let payload = serde_json::json!({
            "claim_id": claim_id.to_string(),
            "warranty_id": warranty_id.to_string(),
        });

        let event = map_to_event("WarrantyClaimed", &payload).expect("event not mapped");
        match event {
            Event::WarrantyClaimed {
                claim_id: mapped_claim_id,
                warranty_id: mapped_warranty_id,
            } => {
                assert_eq!(mapped_claim_id, claim_id);
                assert_eq!(mapped_warranty_id, warranty_id);
            }
            other => unreachable!("test expected WarrantyClaimed but got {:?}", other),
        }
    }

    #[test]
    fn maps_warranty_claim_approved_event() {
        let claim_id = Uuid::new_v4();
        let warranty_id = Uuid::new_v4();
        let payload = serde_json::json!({
            "claim_id": claim_id.to_string(),
            "warranty_id": warranty_id.to_string(),
            "resolution": "Repaired component",
            "notes": "Technician completed on-site repair"
        });

        let event = map_to_event("WarrantyClaimApproved", &payload).expect("event not mapped");
        match event {
            Event::WarrantyClaimApproved {
                claim_id: mapped_claim_id,
                warranty_id: mapped_warranty_id,
                resolution,
                notes,
            } => {
                assert_eq!(mapped_claim_id, claim_id);
                assert_eq!(mapped_warranty_id, warranty_id);
                assert_eq!(resolution.as_deref(), Some("Repaired component"));
                assert_eq!(
                    notes.as_deref(),
                    Some("Technician completed on-site repair")
                );
            }
            other => unreachable!("test expected WarrantyClaimApproved but got {:?}", other),
        }
    }

    #[test]
    fn maps_warranty_claim_rejected_event() {
        let claim_id = Uuid::new_v4();
        let warranty_id = Uuid::new_v4();
        let payload = serde_json::json!({
            "claim_id": claim_id.to_string(),
            "warranty_id": warranty_id.to_string(),
            "reason": "Damage outside coverage",
            "notes": "Photos indicated misuse"
        });

        let event = map_to_event("WarrantyClaimRejected", &payload).expect("event not mapped");
        match event {
            Event::WarrantyClaimRejected {
                claim_id: mapped_claim_id,
                warranty_id: mapped_warranty_id,
                reason,
                notes,
            } => {
                assert_eq!(mapped_claim_id, claim_id);
                assert_eq!(mapped_warranty_id, warranty_id);
                assert_eq!(reason, "Damage outside coverage");
                assert_eq!(notes.as_deref(), Some("Photos indicated misuse"));
            }
            other => unreachable!("test expected WarrantyClaimRejected but got {:?}", other),
        }
    }
}
