use uuid::Uuid;
use async_trait::async_trait;
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

use crate::circuit_breaker::{CircuitBreakerError, CircuitBreaker};
use crate::errors::ServiceError;
use crate::{
    commands::Command,
    events::{Event, EventSender},
};
use crate::models::shipment;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct TrackShipmentCommand {
    pub shipment_id: Uuid,
    #[serde(skip)]
    pub circuit_breaker: Option<Arc<CircuitBreaker>>,
}

#[async_trait::async_trait]
impl Command for TrackShipmentCommand {
    type Result = shipment::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DatabaseConnection>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let shipment = self.get_shipment(&db_pool).await?;

        let tracking_info = self.fetch_tracking_info(&shipment.tracking_number).await?;

        self.log_tracking_info(&tracking_info);
        self.log_and_trigger_event(event_sender, &shipment).await?;

        Ok(shipment)
    }
}

impl TrackShipmentCommand {
    async fn get_shipment(&self, db: &DatabaseConnection) -> Result<shipment::Model, ServiceError> {
        shipment::Entity::find_by_id(self.shipment_id)
            .one(db)
            .await
            .map_err(|e| {
                error!(
                    "Failed to find shipment with ID {}: {}",
                    self.shipment_id, e
                );
                ServiceError::DatabaseError(format!("Failed to find shipment: {}", e))
            })?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Shipment with ID {} not found", self.shipment_id))
            })
    }

    async fn fetch_tracking_info(&self, tracking_number: &str) -> Result<String, ServiceError> {
        // If circuit breaker is available, use it to protect the external API call
        if let Some(cb) = &self.circuit_breaker {
            return cb
                .execute("carrier-tracking-api", || async {
                    self.fetch_tracking_info_impl(tracking_number).await
                })
                .await
                .map_err(|e| {
                    if let CircuitBreakerError::CircuitOpen = e {
                        ServiceError::ExternalServiceError(format!(
                            "Carrier API circuit breaker open for tracking number {}",
                            tracking_number
                        ))
                    } else {
                        ServiceError::ExternalServiceError(format!("Circuit breaker error: {}", e))
                    }
                });
        }

        // Otherwise call directly
        self.fetch_tracking_info_impl(tracking_number).await
    }

    // Internal implementation that makes the actual API call
    async fn fetch_tracking_info_impl(
        &self,
        tracking_number: &str,
    ) -> Result<String, ServiceError> {
        // Simulate fetching tracking info from an external API
        // This would normally involve making an HTTP request to the carrier's API
        // For example, using reqwest:
        //
        // let client = reqwest::Client::new();
        // let response = client.get(&format!("https://api.carrier.com/tracking/{}", tracking_number))
        //    .header("Authorization", "Bearer TOKEN")
        //    .send()
        //    .await
        //    .map_err(|e| ServiceError::ExternalServiceError(format!("Failed to connect to carrier API: {}", e)))?;
        //
        // if !response.status().is_success() {
        //    return Err(ServiceError::ExternalServiceError(format!("Carrier API returned error status: {}", response.status())));
        // }
        //
        // let tracking_data = response.json::<TrackingResponse>().await
        //    .map_err(|e| ServiceError::ExternalServiceError(format!("Failed to parse carrier API response: {}", e)))?;

        // Placeholder implementation
        info!("Fetching tracking info for {}", tracking_number);
        Ok(format!("Tracking info for {}", tracking_number))
    }

    fn log_tracking_info(&self, tracking_info: &str) {
        info!("Tracking info: {}", tracking_info);
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        shipment: &shipment::Model,
    ) -> Result<(), ServiceError> {
        event_sender
            .send(Event::ShipmentTracked(self.shipment_id))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send ShipmentTracked event for shipment ID {}: {}",
                    self.shipment_id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}
