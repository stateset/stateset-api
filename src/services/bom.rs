use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    ModelTrait, QueryFilter, TransactionTrait,
};
use std::sync::Arc;
use tracing::{error, info, instrument, warn};

use crate::{
    entities::{
        bom_header::{self, Entity as BomHeaderEntity},
        bom_line::{self, Entity as BomLineEntity},
        item_master::{self, Entity as ItemMasterEntity},
    },
    errors::ServiceError,
    services::inventory_sync::{InventorySyncService, TransactionType},
};

/// Bill of Materials service for managing product assembly structures
#[derive(Clone)]
pub struct BomService {
    db: Arc<DatabaseConnection>,
    inventory_sync: Arc<InventorySyncService>,
}

impl BomService {
    pub fn new(db: Arc<DatabaseConnection>, inventory_sync: Arc<InventorySyncService>) -> Self {
        Self { db, inventory_sync }
    }

    /// Creates a new BOM header
    #[instrument(skip(self))]
    pub async fn create_bom(
        &self,
        bom_name: String,
        item_id: i64,
        organization_id: i64,
        revision: Option<String>,
    ) -> Result<bom_header::Model, ServiceError> {
        let db = &*self.db;

        let bom = bom_header::ActiveModel {
            bom_id: Set(0), // Auto-generated
            bom_name: Set(bom_name),
            item_id: Set(Some(item_id)),
            organization_id: Set(organization_id),
            revision: Set(revision),
            status_code: Set(Some("ACTIVE".to_string())),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        };

        let created = bom.insert(db).await.map_err(|e| {
            error!("Failed to create BOM: {}", e);
            ServiceError::db_error(e)
        })?;

        info!(
            "BOM created: id={}, name={}",
            created.bom_id, created.bom_name
        );
        Ok(created)
    }

    /// Adds a component to a BOM
    #[instrument(skip(self))]
    pub async fn add_bom_component(
        &self,
        bom_id: i64,
        component_item_id: i64,
        quantity_per_assembly: Decimal,
        uom_code: String,
        operation_seq_num: Option<i32>,
    ) -> Result<bom_line::Model, ServiceError> {
        let db = &*self.db;

        // Verify BOM exists
        let bom = BomHeaderEntity::find_by_id(bom_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| ServiceError::NotFound(format!("BOM {} not found", bom_id)))?;

        // Verify component item exists
        let component = ItemMasterEntity::find_by_id(component_item_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Item {} not found", component_item_id))
            })?;

        let bom_line = bom_line::ActiveModel {
            bom_line_id: Set(0), // Auto-generated
            bom_id: Set(Some(bom_id)),
            component_item_id: Set(Some(component_item_id)),
            quantity_per_assembly: Set(Some(quantity_per_assembly)),
            uom_code: Set(Some(uom_code)),
            operation_seq_num: Set(operation_seq_num),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        };

        let created = bom_line.insert(db).await.map_err(|e| {
            error!("Failed to add BOM component: {}", e);
            ServiceError::db_error(e)
        })?;

        info!(
            "BOM component added: bom_id={}, component_id={}, quantity={}",
            bom_id, component_item_id, quantity_per_assembly
        );

        Ok(created)
    }

    /// Gets all components for a BOM
    #[instrument(skip(self))]
    pub async fn get_bom_components(
        &self,
        bom_id: i64,
    ) -> Result<Vec<bom_line::Model>, ServiceError> {
        let db = &*self.db;

        BomLineEntity::find()
            .filter(bom_line::Column::BomId.eq(bom_id))
            .all(db)
            .await
            .map_err(|e| {
                error!("Failed to fetch BOM components: {}", e);
                ServiceError::db_error(e)
            })
    }

    /// Calculates total component requirements for a production quantity
    #[instrument(skip(self))]
    pub async fn calculate_component_requirements(
        &self,
        bom_id: i64,
        production_quantity: Decimal,
    ) -> Result<Vec<ComponentRequirement>, ServiceError> {
        let components = self.get_bom_components(bom_id).await?;

        let requirements: Vec<ComponentRequirement> = components
            .into_iter()
            .filter_map(|component| {
                component
                    .component_item_id
                    .map(|item_id| ComponentRequirement {
                        item_id,
                        required_quantity: component.quantity_per_assembly.unwrap_or(Decimal::ZERO)
                            * production_quantity,
                        uom_code: component.uom_code,
                    })
            })
            .collect();

        Ok(requirements)
    }

    /// Explodes multi-level BOM to get all components (recursive)
    #[instrument(skip(self))]
    pub async fn explode_bom(
        &self,
        item_id: i64,
        quantity: Decimal,
        level: i32,
    ) -> Result<Vec<ExplodedComponent>, ServiceError> {
        let db = &*self.db;

        // Find BOM for this item
        let bom = BomHeaderEntity::find()
            .filter(bom_header::Column::ItemId.eq(item_id))
            .filter(bom_header::Column::StatusCode.eq("ACTIVE"))
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let mut exploded_components = Vec::new();

        if let Some(bom) = bom {
            let components = self.get_bom_components(bom.bom_id).await?;

            for component in components {
                if let Some(component_item_id) = component.component_item_id {
                    let component_quantity =
                        component.quantity_per_assembly.unwrap_or(Decimal::ZERO) * quantity;

                    exploded_components.push(ExplodedComponent {
                        item_id: component_item_id,
                        quantity: component_quantity,
                        level,
                        uom_code: component.uom_code,
                    });

                    // Recursively explode sub-assemblies
                    let sub_components = Box::pin(self.explode_bom(
                        component_item_id,
                        component_quantity,
                        level + 1,
                    ))
                    .await?;

                    exploded_components.extend(sub_components);
                }
            }
        }

        Ok(exploded_components)
    }

    /// Validates if sufficient components are available for production
    #[instrument(skip(self))]
    pub async fn validate_component_availability(
        &self,
        bom_id: i64,
        production_quantity: Decimal,
        location_id: i32,
    ) -> Result<ComponentAvailability, ServiceError> {
        let requirements = self
            .calculate_component_requirements(bom_id, production_quantity)
            .await?;
        let mut all_available = true;
        let mut shortages = Vec::new();

        for req in &requirements {
            let available = self
                .inventory_sync
                .check_availability(req.item_id, location_id, req.required_quantity)
                .await?;

            if !available {
                all_available = false;
                if let Some(balance) = self
                    .inventory_sync
                    .get_inventory_balance(req.item_id, location_id)
                    .await?
                {
                    shortages.push(ComponentShortage {
                        item_id: req.item_id,
                        required: req.required_quantity,
                        available: balance.quantity_available,
                        shortage: req.required_quantity - balance.quantity_available,
                    });
                } else {
                    shortages.push(ComponentShortage {
                        item_id: req.item_id,
                        required: req.required_quantity,
                        available: Decimal::ZERO,
                        shortage: req.required_quantity,
                    });
                }
            }
        }

        Ok(ComponentAvailability {
            can_produce: all_available,
            shortages,
        })
    }

    /// Consumes components for production (updates inventory)
    #[instrument(skip(self))]
    pub async fn consume_components_for_production(
        &self,
        bom_id: i64,
        production_quantity: Decimal,
        location_id: i32,
        work_order_id: i64,
    ) -> Result<(), ServiceError> {
        let db = &*self.db;
        let txn = db.begin().await.map_err(|e| ServiceError::db_error(e))?;

        // Validate availability first
        let availability = self
            .validate_component_availability(bom_id, production_quantity, location_id)
            .await?;

        if !availability.can_produce {
            return Err(ServiceError::InsufficientStock(format!(
                "Insufficient components for production. Shortages: {:?}",
                availability.shortages
            )));
        }

        // Get component requirements
        let requirements = self
            .calculate_component_requirements(bom_id, production_quantity)
            .await?;

        // Consume each component
        for req in requirements {
            self.inventory_sync
                .update_inventory_balance(
                    req.item_id,
                    location_id,
                    -req.required_quantity,
                    TransactionType::ManufacturingConsumption,
                    Some(work_order_id),
                    Some("WORK_ORDER".to_string()),
                )
                .await?;
        }

        txn.commit().await.map_err(|e| ServiceError::db_error(e))?;

        info!(
            "Components consumed for production: bom_id={}, quantity={}, work_order_id={}",
            bom_id, production_quantity, work_order_id
        );

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ComponentRequirement {
    pub item_id: i64,
    pub required_quantity: Decimal,
    pub uom_code: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ExplodedComponent {
    pub item_id: i64,
    pub quantity: Decimal,
    pub level: i32,
    pub uom_code: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ComponentAvailability {
    pub can_produce: bool,
    pub shortages: Vec<ComponentShortage>,
}

#[derive(Debug, Clone)]
pub struct ComponentShortage {
    pub item_id: i64,
    pub required: Decimal,
    pub available: Decimal,
    pub shortage: Decimal,
}
