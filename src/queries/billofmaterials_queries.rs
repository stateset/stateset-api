use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use sea_orm::{
    QuerySelect,
    QueryOrder,
    QueryFilter,
    EntityTrait,
    RelationTrait,
    query::*,
    Expr,
    Function::*,
};
use crate::{errors::ServiceError, db::DbPool, models::*};
use chrono::{DateTime, Utc};

use sea_orm::Order;

// Comment out imports that don't exist yet
// These would be created properly when implementing the full application
/*
use crate::billofmaterials::BillOfMaterials;
use crate::inventory_item::InventoryItem;
use crate::order::Order;
use crate::shipment::Shipment;
use crate::tracking_event::TrackingEvent;
use crate::work_order::WorkOrder;
use crate::return_entity::ReturnEntity;
use crate::order_item::OrderItem;   
use crate::product::Product;
use crate::customer::Customer;
use crate::warehouse::Warehouse;
*/

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
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let product = product_entity::Entity::find_by_id(self.product_id)
            .one(&db)
            .await
            .map_err(|_| ServiceError::NotFound)?
            .ok_or(ServiceError::NotFound)?;

        let items = BillOfMaterialsLineItem::find()
            .filter(BillOfMaterialsLineItem::Column::ProductId.eq(self.product_id))
            .find_also_related(Component::Entity)
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let bom_items = items
            .into_iter()
            .map(|(bom_item, component)| BOMItem {
                component_id: component.unwrap().id,
                component_name: component.unwrap().name,
                quantity: bom_item.quantity,
                unit: component.unwrap().unit,
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
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let product = Product::find_by_id(self.product_id)
            .one(&db)
            .await
            .map_err(|_| ServiceError::NotFound)?
            .ok_or(ServiceError::NotFound)?;

        let items = BillOfMaterialsLineItem::find()
            .filter(BillOfMaterialsLineItem::Column::ProductId.eq(self.product_id))
            .find_also_related(Component::Entity)
            .find_also_related(InventoryItem::Entity)
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let mut total_cost = 0.0;
        let cost_items: Vec<BOMCostItem> = items
            .into_iter()
            .map(|((bom_item, component), inventory_item)| {
                let unit_cost = inventory_item.unwrap().unit_cost;
                let item_total_cost = bom_item.quantity * unit_cost;
                total_cost += item_total_cost;
                BOMCostItem {
                    component_id: component.unwrap().id,
                    component_name: component.unwrap().name,
                    quantity: bom_item.quantity,
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
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let component = Component::find_by_id(self.component_id)
            .one(&db)
            .await
            .map_err(|_| ServiceError::NotFound)?
            .ok_or(ServiceError::NotFound)?;

        let usages = BillOfMaterialsLineItem::find()
            .filter(BillOfMaterialsLineItem::Column::ComponentId.eq(self.component_id))
            .find_also_related(Product::Entity)
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let product_usages = usages
            .into_iter()
            .map(|(bom_item, product)| ComponentProductUsage {
                product_id: product.unwrap().id,
                product_name: product.unwrap().name,
                quantity: bom_item.quantity,
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
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let shortages = BillOfMaterialsLineItem::find()
            .filter(BillOfMaterialsLineItem::Column::ProductId.eq(self.product_id))
            .find_also_related(Component::Entity)
                .find_also_related(InventoryItem::Entity)
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(shortages
            .into_iter()
            .filter_map(|((bom_item, component), inventory_item)| {
                let required_quantity = bom_item.quantity * self.production_quantity as f64;
                let available_quantity = inventory_item.unwrap().quantity as f64;
                let shortage = required_quantity - available_quantity;
                if shortage > 0.0 {
                    Some(BOMShortage {
                        component_id: component.unwrap().id,
                        component_name: component.unwrap().name,
                        required_quantity,
                        available_quantity,
                        shortage,
                    })
                } else {
                    None
                }
            })
            .collect())
    }
}
