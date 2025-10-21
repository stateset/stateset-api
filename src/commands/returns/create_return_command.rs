use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        r#return::ReturnStatus,
        return_entity::{self, Entity as Return},
    },
};
use sea_orm::{Set, *};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct InitiateReturnCommand {
    pub order_id: Uuid,

    #[validate(length(min = 1, message = "Reason cannot be empty"))]
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitiateReturnResult {
    pub id: Uuid,
    pub order_id: Uuid,
    pub reason: String,
    pub status: String,
    pub created_at: chrono::NaiveDateTime,
}

#[async_trait::async_trait]
impl Command for InitiateReturnCommand {
    type Result = InitiateReturnResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        let saved_return = self.create_return_request(db).await?;

        self.log_and_trigger_event(&event_sender, &saved_return)
            .await?;

        Ok(InitiateReturnResult {
            id: saved_return.id,
            order_id: saved_return.order_id,
            status: saved_return.status,
            reason: saved_return.reason,
            created_at: saved_return.created_at,
        })
    }
}

impl InitiateReturnCommand {
    async fn create_return_request(
        &self,
        db: &DatabaseConnection,
    ) -> Result<return_entity::Model, ServiceError> {
        let return_request = return_entity::ActiveModel {
            order_id: Set(self.order_id),
            reason: Set(self.reason.clone()),
            status: Set(ReturnStatus::Requested.as_str().to_owned()),
            ..Default::default()
        };

        return_request.insert(db).await.map_err(|e| {
            let msg = format!(
                "Failed to initiate return for order ID {}: {}",
                self.order_id, e
            );
            error!("{}", msg);
            ServiceError::db_error(msg)
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        saved_return: &return_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(
            "Return initiated for order ID: {}. Reason: {}",
            self.order_id, self.reason
        );
        event_sender
            .send(Event::ReturnCreated(saved_return.id))
            .await
            .map_err(|e| {
                let msg = format!("Failed to send event for initiated return: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    use super::*;
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
    async fn test_validate_return_command() {
        // Test with valid data
        let valid_command = InitiateReturnCommand {
            order_id: Uuid::new_v4(),
            reason: "Product is damaged".to_string(),
        };

        assert!(valid_command.validate().is_ok());

        // Test with invalid data - empty reason
        let invalid_command = InitiateReturnCommand {
            order_id: Uuid::new_v4(),
            reason: "".to_string(),
        };

        assert!(invalid_command.validate().is_err());
    }
}
