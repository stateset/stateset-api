use uuid::Uuid;
use crate::{
    commands::Command,
    events::{Event, EventSender},
    db::DbPool,
    errors::ServiceError,
    models::bom_line_item::{self, Entity as BOMLineItem, Model as BOMLineItemModel},
    models::billofmaterials::{LineType, SupplyType, LineItemStatus},
};
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, DbErr, Set, TransactionError, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AddComponentToBOMCommand {
    pub bom_id: i32,
    pub component_id: i32, // ID of the component to add
    #[validate(range(min = 1))]
    pub quantity: i32, // Quantity of the component
}

#[async_trait::async_trait]
impl Command for AddComponentToBOMCommand {
    type Result = BOMLineItemModel;

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

        let component = db
            .transaction::<_, BOMLineItemModel, ServiceError>(|txn| {
                Box::pin(async move { self.add_component(txn).await })
            })
            .await
            .map_err(|e| {
                error!(
                    "Transaction failed for adding component {} to BOM ID {}: {}",
                    self.component_id, self.bom_id, e
                );
                match e {
                    TransactionError::Connection(db_err) => ServiceError::DatabaseError(db_err),
                    TransactionError::Transaction(service_err) => service_err,
                }
            })?;

        self.log_and_trigger_event(event_sender, &component).await?;

        Ok(component)
    }
}

impl AddComponentToBOMCommand {
    async fn add_component(&self, db: &DatabaseConnection) -> Result<BOMLineItemModel, ServiceError> {
        let new_component = bom_line_item::ActiveModel {
            bill_of_materials_number: Set(self.bom_id.to_string()),
            part_number: Set(self.component_id.to_string()),
            quantity: Set(self.quantity as f64),
            line_type: Set(LineType::Component),
            part_name: Set(format!("Component {}", self.component_id)),
            purchase_supply_type: Set(SupplyType::Purchase), // Using an existing variant
            status: Set(LineItemStatus::Active), // Need to import and use the correct status
            bill_of_materials_id: Set(self.bom_id),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            ..Default::default()
        };

        new_component.insert(db).await.map_err(|e| {
            error!(
                "Failed to add component {} to BOM ID {}: {}",
                self.component_id, self.bom_id, e
            );
            ServiceError::DatabaseError(e)
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        component: &BOMLineItemModel,
    ) -> Result<(), ServiceError> {
        info!(
            "Component ID: {} added to BOM ID: {}",
            self.component_id, self.bom_id
        );
        event_sender
            .send(Event::ComponentAddedToBOM { 
                bom_id: Uuid::parse_str(&self.bom_id.to_string()).unwrap_or_default(),
                component_id: Uuid::parse_str(&component.part_number).unwrap_or_default(),
            })
            .await
            .map_err(|e| {
                let msg = format!("Failed to send event for BOM component addition: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}
