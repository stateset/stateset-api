use crate::commands::Command;
use crate::{
    db::DbPool,
    entities::warranty::{self, Entity as Warranty},
    errors::ServiceError,
    events::{Event, EventSender},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use prometheus::{Counter, IntCounter};
use sea_orm::{*, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref WARRANTY_CREATIONS: IntCounter = IntCounter::new(
        "warranty_creations_total",
        "Total number of warranties created"
    )
    .expect("metric can be created");
    static ref WARRANTY_CREATION_FAILURES: IntCounter = IntCounter::new(
        "warranty_creation_failures_total",
        "Total number of failed warranty creations"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateWarrantyCommand {
    pub product_id: Uuid,
    pub customer_id: Uuid,
    #[validate(length(min = 1, message = "Serial number cannot be empty"))]
    pub serial_number: String, // Note: This field isn't in the entity model but we'll use it in the description
    #[validate(length(min = 1, message = "Warranty type cannot be empty"))]
    pub warranty_type: String,
    pub expiration_date: DateTime<Utc>,
    pub terms: String,
}

#[async_trait]
impl Command for CreateWarrantyCommand {
    type Result = Uuid;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            WARRANTY_CREATION_FAILURES.inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        // Generate a unique warranty number
        let warranty_number = format!("W-{}", uuid::Uuid::new_v4().simple());

        // Create a new warranty record
        let warranty = warranty::ActiveModel {
            id: Set(Uuid::new_v4()),
            warranty_number: Set(warranty_number),
            product_id: Set(self.product_id),
            customer_id: Set(self.customer_id),
            order_id: Set(None),
            status: Set("active".to_string()),
            start_date: Set(Utc::now()),
            end_date: Set(self.expiration_date),
            description: Set(Some(format!(
                "Warranty for product {} with serial number {}",
                self.product_id, self.serial_number
            ))),
            terms: Set(Some(self.terms.clone())),
            created_at: Set(Utc::now()),
            updated_at: Set(None),
        };

        let result = warranty.insert(db).await.map_err(|e| {
            WARRANTY_CREATION_FAILURES.inc();
            let msg = format!("Failed to create warranty: {}", e);
            error!("{}", msg);
            ServiceError::DatabaseError(e)
        })?;

        // Send warranty created event
        event_sender
            .send(Event::WarrantyCreated(result.id))
            .await
            .map_err(|e| {
                WARRANTY_CREATION_FAILURES.inc();
                let msg = format!("Failed to send warranty created event: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })?;

        info!(
            warranty_id = %result.id,
            product_id = %self.product_id,
            customer_id = %self.customer_id,
            "Warranty created successfully"
        );

        WARRANTY_CREATIONS.inc();

        Ok(result.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use mockall::mock;
    use mockall::predicate::*;
    use tokio::sync::broadcast;

    mock! {
        pub Database {}
        impl Clone for Database {
            fn clone(&self) -> Self;
        }
    }

    #[tokio::test]
    async fn test_validate_warranty_command() {
        // Test with valid data
        let valid_command = CreateWarrantyCommand {
            product_id: Uuid::new_v4(),
            customer_id: Uuid::new_v4(),
            serial_number: "SN12345".to_string(),
            warranty_type: "Extended".to_string(),
            expiration_date: Utc::now() + Duration::days(365),
            terms: "Standard warranty terms".to_string(),
        };

        assert!(valid_command.validate().is_ok());

        // Test with invalid data - empty serial number
        let invalid_command = CreateWarrantyCommand {
            product_id: Uuid::new_v4(),
            customer_id: Uuid::new_v4(),
            serial_number: "".to_string(),
            warranty_type: "Extended".to_string(),
            expiration_date: Utc::now() + Duration::days(365),
            terms: "Standard warranty terms".to_string(),
        };

        assert!(invalid_command.validate().is_err());

        // Test with invalid data - empty warranty type
        let invalid_command2 = CreateWarrantyCommand {
            product_id: Uuid::new_v4(),
            customer_id: Uuid::new_v4(),
            serial_number: "SN12345".to_string(),
            warranty_type: "".to_string(),
            expiration_date: Utc::now() + Duration::days(365),
            terms: "Standard warranty terms".to_string(),
        };

        assert!(invalid_command2.validate().is_err());
    }
}
