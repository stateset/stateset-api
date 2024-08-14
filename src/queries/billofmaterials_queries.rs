use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::*};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::dsl::*;

#[async_trait]
pub trait Query: Send + Sync {
    type Result: Send + Sync;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetBOMByProductQuery {
    pub product_id: i32,
}

#[derive(Debug, Serialize)]
pub struct BOMItem {
    pub component_id: i32,
    pub component_name: String,
    pub quantity: f64,
    pub unit: String,
}

#[derive(Debug, Serialize)]
pub struct BillOfMaterials {
    pub product_id: i32,
    pub product_name: String,
    pub items: Vec<BOMItem>,
}

#[async_trait]
impl Query for GetBOMByProductQuery {
    type Result = BillOfMaterials;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let product = products::table
            .find(self.product_id)
            .first::<Product>(&conn)
            .map_err(|_| ServiceError::NotFound)?;

        let items = bom_items::table
            .inner_join(components::table)
            .filter(bom_items::product_id.eq(self.product_id))
            .select((
                components::id,
                components::name,
                bom_items::quantity,
                components::unit,
            ))
            .load::<(i32, String, f64, String)>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        let bom_items = items
            .into_iter()
            .map(|(component_id, component_name, quantity, unit)| BOMItem {
                component_id,
                component_name,
                quantity,
                unit,
            })
            .collect();

        Ok(BillOfMaterials {
            product_id: product.id,
            product_name: product.name,
            items: bom_items,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetBOMCostAnalysisQuery {
    pub product_id: i32,
}

#[derive(Debug, Serialize)]
pub struct BOMCostItem {
    pub component_id: i32,
    pub component_name: String,
    pub quantity: f64,
    pub unit_cost: f64,
    pub total_cost: f64,
}

#[derive(Debug, Serialize)]
pub struct BOMCostAnalysis {
    pub product_id: i32,
    pub product_name: String,
    pub total_cost: f64,
    pub items: Vec<BOMCostItem>,
}

#[async_trait]
impl Query for GetBOMCostAnalysisQuery {
    type Result = BOMCostAnalysis;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let product = products::table
            .find(self.product_id)
            .first::<Product>(&conn)
            .map_err(|_| ServiceError::NotFound)?;

        let items = bom_items::table
            .inner_join(components::table)
            .inner_join(inventory_items::table.on(components::id.eq(inventory_items::product_id)))
            .filter(bom_items::product_id.eq(self.product_id))
            .select((
                components::id,
                components::name,
                bom_items::quantity,
                inventory_items::unit_cost,
            ))
            .load::<(i32, String, f64, f64)>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        let mut total_cost = 0.0;
        let cost_items: Vec<BOMCostItem> = items
            .into_iter()
            .map(|(component_id, component_name, quantity, unit_cost)| {
                let item_total_cost = quantity * unit_cost;
                total_cost += item_total_cost;
                BOMCostItem {
                    component_id,
                    component_name,
                    quantity,
                    unit_cost,
                    total_cost: item_total_cost,
                }
            })
            .collect();

        Ok(BOMCostAnalysis {
            product_id: product.id,
            product_name: product.name,
            total_cost,
            items: cost_items,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetComponentUsageQuery {
    pub component_id: i32,
}

#[derive(Debug, Serialize)]
pub struct ComponentUsage {
    pub component_id: i32,
    pub component_name: String,
    pub products: Vec<ComponentProductUsage>,
}

#[derive(Debug, Serialize)]
pub struct ComponentProductUsage {
    pub product_id: i32,
    pub product_name: String,
    pub quantity: f64,
}

#[async_trait]
impl Query for GetComponentUsageQuery {
    type Result = ComponentUsage;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let component = components::table
            .find(self.component_id)
            .first::<Component>(&conn)
            .map_err(|_| ServiceError::NotFound)?;

        let usages = bom_items::table
            .inner_join(products::table)
            .filter(bom_items::component_id.eq(self.component_id))
            .select((
                products::id,
                products::name,
                bom_items::quantity,
            ))
            .load::<(i32, String, f64)>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        let product_usages = usages
            .into_iter()
            .map(|(product_id, product_name, quantity)| ComponentProductUsage {
                product_id,
                product_name,
                quantity,
            })
            .collect();

        Ok(ComponentUsage {
            component_id: component.id,
            component_name: component.name,
            products: product_usages,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetBOMShortagesQuery {
    pub product_id: i32,
    pub production_quantity: i32,
}

#[derive(Debug, Serialize)]
pub struct BOMShortage {
    pub component_id: i32,
    pub component_name: String,
    pub required_quantity: f64,
    pub available_quantity: f64,
    pub shortage: f64,
}

#[async_trait]
impl Query for GetBOMShortagesQuery {
    type Result = Vec<BOMShortage>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let shortages = bom_items::table
            .inner_join(components::table)
            .inner_join(inventory_items::table.on(components::id.eq(inventory_items::product_id)))
            .filter(bom_items::product_id.eq(self.product_id))
            .select((
                components::id,
                components::name,
                (bom_items::quantity * self.production_quantity as f64),
                inventory_items::quantity,
            ))
            .load::<(i32, String, f64, i32)>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(shortages
            .into_iter()
            .filter_map(|(component_id, component_name, required_quantity, available_quantity)| {
                let shortage = required_quantity - available_quantity as f64;
                if shortage > 0.0 {
                    Some(BOMShortage {
                        component_id,
                        component_name,
                        required_quantity,
                        available_quantity: available_quantity as f64,
                        shortage,
                    })
                } else {
                    None
                }
            })
            .collect())
    }
}