use crate::{
    db::DbPool,
    entities::manufacturing::{
        bom, bom::Entity as BomEntity, bom_audit, bom_audit::Entity as BomAuditEntity,
        bom_component, bom_component::Entity as BomComponentEntity,
    },
    errors::ServiceError,
    events::{Event, EventSender},
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, ModelTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tracing::instrument;
use uuid::Uuid;

/// Summary view returned when listing BOMs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomSummary {
    pub id: Uuid,
    pub bom_number: String,
    pub name: String,
    pub revision: String,
    pub lifecycle_status: String,
    pub product_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Detailed component information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomComponentView {
    pub id: Uuid,
    pub component_product_id: Option<Uuid>,
    pub component_item_id: Option<i64>,
    pub quantity: Decimal,
    pub unit_of_measure: String,
    pub position: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Audit entries associated with a BOM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomAuditView {
    pub id: Uuid,
    pub event_type: String,
    pub user_id: Option<Uuid>,
    pub notes: Option<String>,
    pub event_at: DateTime<Utc>,
}

/// Detailed BOM view including components and audit trail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomDetail {
    pub id: Uuid,
    pub bom_number: String,
    pub product_id: Uuid,
    pub item_master_id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    pub revision: String,
    pub lifecycle_status: String,
    pub metadata: Option<Value>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub components: Vec<BomComponentView>,
    pub audits: Vec<BomAuditView>,
}

/// Input payload for creating a BOM
#[derive(Debug, Clone)]
pub struct CreateBomInput {
    pub product_id: Uuid,
    pub item_master_id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    pub revision: String,
    pub components: Vec<CreateBomComponentInput>,
    pub created_by: Option<Uuid>,
    pub lifecycle_status: Option<String>,
    pub metadata: Option<Value>,
    pub bom_number: Option<String>,
}

/// Input payload for creating or adding a component to a BOM
#[derive(Debug, Clone)]
pub struct CreateBomComponentInput {
    pub component_product_id: Option<Uuid>,
    pub component_item_id: Option<i64>,
    pub quantity: Decimal,
    pub unit_of_measure: String,
    pub position: Option<String>,
    pub notes: Option<String>,
}

/// Input payload for updating high-level BOM fields
#[derive(Debug, Clone)]
pub struct UpdateBomInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub revision: Option<String>,
    pub lifecycle_status: Option<String>,
    pub metadata: Option<Value>,
    pub updated_by: Option<Uuid>,
}

/// Input payload for recording BOM audit events
#[derive(Debug, Clone)]
pub struct AuditBomInput {
    pub event_type: String,
    pub user_id: Option<Uuid>,
    pub notes: Option<String>,
    pub event_at: Option<DateTime<Utc>>,
}

/// Service for managing Bill of Materials records in the manufacturing schema
#[derive(Clone)]
pub struct BillOfMaterialsService {
    db_pool: Arc<DbPool>,
    event_sender: Arc<EventSender>,
}

impl BillOfMaterialsService {
    pub fn new(db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Self {
        Self {
            db_pool,
            event_sender,
        }
    }

    /// Creates a BOM with its initial component list.
    #[instrument(skip(self, input))]
    pub async fn create_bom(&self, input: CreateBomInput) -> Result<Uuid, ServiceError> {
        let db = self.connection();
        let mut txn = db.begin().await.map_err(ServiceError::db_error)?;

        let bom_number = input
            .bom_number
            .unwrap_or_else(|| format!("BOM-{}", Uuid::new_v4().simple()));
        let now = Utc::now();

        let bom_model = bom::ActiveModel {
            id: Default::default(),
            product_id: Set(input.product_id),
            item_master_id: Set(input.item_master_id),
            bom_number: Set(bom_number.clone()),
            name: Set(input.name.clone()),
            description: Set(input.description.clone()),
            revision: Set(input.revision.clone()),
            lifecycle_status: Set(input
                .lifecycle_status
                .clone()
                .unwrap_or_else(|| "draft".to_string())),
            metadata: Set(input.metadata.clone()),
            created_by: Set(input.created_by),
            updated_by: Set(input.created_by),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let bom = bom_model
            .insert(&mut txn)
            .await
            .map_err(ServiceError::db_error)?;

        for component in input.components {
            let component_model = bom_component::ActiveModel {
                id: Default::default(),
                bom_id: Set(bom.id),
                component_product_id: Set(component.component_product_id),
                component_item_id: Set(component.component_item_id),
                quantity: Set(component.quantity),
                unit_of_measure: Set(component.unit_of_measure),
                position: Set(component.position.clone()),
                notes: Set(component.notes.clone()),
                created_at: Set(now),
                updated_at: Set(now),
            };

            component_model
                .insert(&mut txn)
                .await
                .map_err(ServiceError::db_error)?;
        }

        txn.commit().await.map_err(ServiceError::db_error)?;

        self.event_sender
            .send_or_log(Event::BOMCreated {
                bom_id: bom.id,
                product_id: bom.product_id,
                revision: bom.revision.clone(),
            })
            .await;

        Ok(bom.id)
    }

    /// Fetches a BOM and its components by identifier.
    #[instrument(skip(self))]
    pub async fn get_bom(&self, bom_id: &Uuid) -> Result<Option<BomDetail>, ServiceError> {
        let db = self.connection();
        if let Some(model) = BomEntity::find_by_id(*bom_id)
            .one(db)
            .await
            .map_err(ServiceError::db_error)?
        {
            let detail = self.map_bom_to_detail(model, db).await?;
            Ok(Some(detail))
        } else {
            Ok(None)
        }
    }

    /// Returns paginated BOM summaries.
    #[instrument(skip(self))]
    pub async fn list_boms(
        &self,
        page: u64,
        limit: u64,
    ) -> Result<(Vec<BomSummary>, u64), ServiceError> {
        let db = self.connection();
        let limit = limit.max(1);
        let page = page.max(1) - 1;
        let paginator = BomEntity::find()
            .order_by_desc(bom::Column::CreatedAt)
            .paginate(db, limit);

        let total = paginator
            .num_items()
            .await
            .map_err(ServiceError::db_error)?;

        let models = paginator
            .fetch_page(page)
            .await
            .map_err(ServiceError::db_error)?;

        let summaries = models
            .into_iter()
            .map(|model| BomSummary {
                id: model.id,
                bom_number: model.bom_number,
                name: model.name,
                revision: model.revision,
                lifecycle_status: model.lifecycle_status,
                product_id: model.product_id,
                created_at: model.created_at,
                updated_at: model.updated_at,
            })
            .collect();

        Ok((summaries, total))
    }

    /// Applies updates to mutable BOM fields.
    #[instrument(skip(self, input))]
    pub async fn update_bom(
        &self,
        bom_id: Uuid,
        input: UpdateBomInput,
    ) -> Result<(), ServiceError> {
        let db = self.connection();
        let mut model = BomEntity::find_by_id(bom_id)
            .one(db)
            .await
            .map_err(ServiceError::db_error)?
            .ok_or_else(|| ServiceError::NotFound(format!("BOM {} not found", bom_id)))?;

        if let Some(name) = input.name {
            model.name = name;
        }
        if let Some(desc) = input.description {
            model.description = Some(desc);
        }
        if let Some(revision) = input.revision {
            model.revision = revision;
        }
        if let Some(status) = input.lifecycle_status {
            model.lifecycle_status = status;
        }
        if let Some(metadata) = input.metadata {
            model.metadata = Some(metadata);
        }
        if let Some(updated_by) = input.updated_by {
            model.updated_by = Some(updated_by);
        }
        model.updated_at = Utc::now();

        let active = model.into_active_model();

        active.update(db).await.map_err(ServiceError::db_error)?;

        self.event_sender
            .send_or_log(Event::BOMUpdated { bom_id })
            .await;

        Ok(())
    }

    /// Records an audit entry for a BOM.
    #[instrument(skip(self, input))]
    pub async fn audit_bom(&self, bom_id: Uuid, input: AuditBomInput) -> Result<(), ServiceError> {
        let db = self.connection();
        // Ensure BOM exists
        let exists = BomEntity::find_by_id(bom_id)
            .select_only()
            .column(bom::Column::Id)
            .one(db)
            .await
            .map_err(ServiceError::db_error)?;

        if exists.is_none() {
            return Err(ServiceError::NotFound(format!("BOM {} not found", bom_id)));
        }

        let audit = bom_audit::ActiveModel {
            id: Default::default(),
            bom_id: Set(bom_id),
            event_type: Set(input.event_type),
            user_id: Set(input.user_id),
            notes: Set(input.notes),
            event_at: Set(input.event_at.unwrap_or_else(Utc::now)),
        };

        audit.insert(db).await.map_err(ServiceError::db_error)?;

        self.event_sender
            .send_or_log(Event::BOMAudited { bom_id })
            .await;

        Ok(())
    }

    /// Retrieves the component list for a BOM.
    #[instrument(skip(self))]
    pub async fn get_bom_components(
        &self,
        bom_id: &Uuid,
    ) -> Result<Vec<BomComponentView>, ServiceError> {
        let db = self.connection();
        let components = BomComponentEntity::find()
            .filter(bom_component::Column::BomId.eq(*bom_id))
            .order_by_asc(bom_component::Column::CreatedAt)
            .all(db)
            .await
            .map_err(ServiceError::db_error)?;

        Ok(components.into_iter().map(Self::map_component).collect())
    }

    /// Adds a component to a BOM and returns the new component identifier.
    #[instrument(skip(self, component))]
    pub async fn add_component_to_bom(
        &self,
        bom_id: &Uuid,
        component: CreateBomComponentInput,
    ) -> Result<Uuid, ServiceError> {
        let db = self.connection();

        // Ensure the BOM exists
        let exists = BomEntity::find_by_id(*bom_id)
            .select_only()
            .column(bom::Column::Id)
            .one(db)
            .await
            .map_err(ServiceError::db_error)?;

        if exists.is_none() {
            return Err(ServiceError::NotFound(format!("BOM {} not found", bom_id)));
        }

        let now = Utc::now();
        let component_model = bom_component::ActiveModel {
            id: Default::default(),
            bom_id: Set(*bom_id),
            component_product_id: Set(component.component_product_id),
            component_item_id: Set(component.component_item_id),
            quantity: Set(component.quantity),
            unit_of_measure: Set(component.unit_of_measure),
            position: Set(component.position),
            notes: Set(component.notes),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let created = component_model
            .insert(db)
            .await
            .map_err(ServiceError::db_error)?;

        self.event_sender
            .send_or_log(Event::ComponentAddedToBOM {
                bom_id: *bom_id,
                component_id: created.id,
            })
            .await;

        Ok(created.id)
    }

    /// Removes a component from a BOM.
    #[instrument(skip(self))]
    pub async fn remove_component_from_bom(
        &self,
        bom_id: &Uuid,
        component_id: &Uuid,
    ) -> Result<(), ServiceError> {
        let db = self.connection();

        let component = BomComponentEntity::find_by_id(*component_id)
            .one(db)
            .await
            .map_err(ServiceError::db_error)?;

        let component = match component {
            Some(component) if component.bom_id == *bom_id => component,
            Some(_) => {
                return Err(ServiceError::InvalidOperation(
                    "Component does not belong to BOM".to_string(),
                ))
            }
            None => {
                return Err(ServiceError::NotFound(format!(
                    "Component {} not found",
                    component_id
                )))
            }
        };

        let active = component.into_active_model();

        active.delete(db).await.map_err(ServiceError::db_error)?;

        self.event_sender
            .send_or_log(Event::ComponentRemovedFromBOM {
                bom_id: *bom_id,
                component_id: *component_id,
            })
            .await;

        Ok(())
    }

    fn connection(&self) -> &DatabaseConnection {
        self.db_pool.as_ref()
    }

    async fn map_bom_to_detail(
        &self,
        model: bom::Model,
        db: &DatabaseConnection,
    ) -> Result<BomDetail, ServiceError> {
        let components = model
            .find_related(BomComponentEntity)
            .order_by_asc(bom_component::Column::CreatedAt)
            .all(db)
            .await
            .map_err(ServiceError::db_error)?;

        let audits = model
            .find_related(BomAuditEntity)
            .order_by_desc(bom_audit::Column::EventAt)
            .all(db)
            .await
            .map_err(ServiceError::db_error)?;

        Ok(BomDetail {
            id: model.id,
            bom_number: model.bom_number,
            product_id: model.product_id,
            item_master_id: model.item_master_id,
            name: model.name,
            description: model.description,
            revision: model.revision,
            lifecycle_status: model.lifecycle_status,
            metadata: model.metadata,
            created_by: model.created_by,
            updated_by: model.updated_by,
            created_at: model.created_at,
            updated_at: model.updated_at,
            components: components.into_iter().map(Self::map_component).collect(),
            audits: audits
                .into_iter()
                .map(|audit| BomAuditView {
                    id: audit.id,
                    event_type: audit.event_type,
                    user_id: audit.user_id,
                    notes: audit.notes,
                    event_at: audit.event_at,
                })
                .collect(),
        })
    }

    fn map_component(model: bom_component::Model) -> BomComponentView {
        BomComponentView {
            id: model.id,
            component_product_id: model.component_product_id,
            component_item_id: model.component_item_id,
            quantity: model.quantity,
            unit_of_measure: model.unit_of_measure,
            position: model.position,
            notes: model.notes,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}
