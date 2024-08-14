use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{ProcurementRequest, NewProcurementRequest}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateProcurementRequestCommand {
    #[validate(length(min = 1))]
    pub request_details: String, // Details of the procurement request
    #[validate(range(min = 1))]
    pub requested_by: i32, // ID of the person making the request
}

#[async_trait::async_trait]
impl Command for CreateProcurementRequestCommand {
    type Result = ProcurementRequest;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let procurement_request = conn.transaction(|| {
            self.create_procurement_request(&conn)
        }).map_err(|e| {
            error!("Transaction failed for creating Procurement Request: {}", e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &procurement_request).await?;

        Ok(procurement_request)
    }
}

impl CreateProcurementRequestCommand {
    fn create_procurement_request(&self, conn: &PgConnection) -> Result<ProcurementRequest, ServiceError> {
        let new_request = NewProcurementRequest {
            request_details: self.request_details.clone(),
            requested_by: self.requested_by,
            status: "Pending".to_string(), // Initial status is "Pending"
            created_at: Utc::now(),
        };

        diesel::insert_into(procurement_requests::table)
            .values(&new_request)
            .get_result::<ProcurementRequest>(conn)
            .map_err(|e| {
                error!("Failed to create Procurement Request: {}", e);
                ServiceError::DatabaseError(format!("Failed to create Procurement Request: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, request: &ProcurementRequest) -> Result<(), ServiceError> {
        info!("Procurement Request created with ID: {}", request.id);
        event_sender.send(Event::ProcurementRequestCreated(request.id))
            .await
            .map_err(|e| {
                error!("Failed to send ProcurementRequestCreated event for Request ID {}: {}", request.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
