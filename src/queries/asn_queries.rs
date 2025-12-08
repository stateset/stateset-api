use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use sea_orm::{
    prelude::*, query::*, DatabaseConnection,
    EntityTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait,
    sea_query::{Func, Expr, Alias}, IntoSimpleExpr, FromQueryResult,
};

use crate::{
    db::DbPool, errors::ServiceError,
    models::{
        product_entity::{Entity as ProductEntity},
        asn_entity::{Entity as ASNEntity, Model as ASNModel, ASNStatus},
        asn_item_entity::{Entity as ASNItemEntity},
        warehouse::{Entity as WarehouseEntity},
        purchase_order_entity::{Entity as PurchaseOrderEntity},
        suppliers::{Entity as SupplierEntity},
    },
};
// Models imported via wildcard above

/// Trait representing a generic asynchronous query.
#[async_trait]
pub trait Query: Send + Sync {
    type Result: Send + Sync;

    /// Executes the query using the provided database pool.
    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError>;
}

// Note: DbPool is already a DatabaseConnection, so we can use it directly

/// Query to get a specific ASN by ID
#[derive(Debug, Serialize, Deserialize)]
pub struct GetASNQuery {
    pub asn_id: Uuid,
}

#[async_trait]
impl Query for GetASNQuery {
    type Result = Option<ASNModel>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        ASNEntity::find_by_id(self.asn_id)
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))
    }
}

/// Query to get ASNs by status with pagination
#[derive(Debug, Serialize, Deserialize)]
pub struct GetASNsByStatusQuery {
    pub status: ASNStatus,
    pub limit: u64,
    pub offset: u64,
}

#[async_trait]
impl Query for GetASNsByStatusQuery {
    type Result = Vec<ASNModel>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        ASNEntity::find()
            .filter(crate::models::asn_entity::Column::Status.eq(self.status.clone()))
            .limit(self.limit)
            .offset(self.offset)
            .order_by_desc(crate::models::asn_entity::Column::ExpectedDeliveryDate)
            .all(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))
    }
}

/// Detailed ASN information including related entities
#[derive(Debug, Serialize)]
pub struct ASNDetails {
    pub asn: ASNModel,
    pub supplier: crate::models::suppliers::Model,
    pub warehouse: crate::models::warehouse::Model,
    // Package and PO relations available via separate queries
}

/// Query to get detailed ASN information
#[derive(Debug, Serialize, Deserialize)]
pub struct GetASNDetailsQuery {
    pub asn_id: Uuid,
}

#[async_trait]
impl Query for GetASNDetailsQuery {
    type Result = ASNDetails;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        // Fetch the ASN
        let asn = ASNEntity::find_by_id(self.asn_id)
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| ServiceError::NotFound("ASN not found".to_string()))?;

        // Fetch the supplier
        let supplier = SupplierEntity::find_by_id(asn.supplier_id)
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| ServiceError::NotFound("Supplier not found".to_string()))?;

        // Fetch the warehouse
        let warehouse = WarehouseEntity::find_by_id(asn.warehouse_id)
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| ServiceError::NotFound("Warehouse not found".to_string()))?;

        // Fetch purchase order if exists
        let purchase_order = if let Some(po_id) = asn.purchase_order_id {
            PurchaseOrderEntity::find_by_id(po_id)
                .one(db_pool)
                .await
                .map_err(|e| ServiceError::db_error(e))?
        } else {
            None
        };

        // Fetch packages
        let packages = ASNItemEntity::find()
            .filter(crate::models::asn_item_entity::Column::AsnId.eq(self.asn_id))
            .all(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let mut packages_with_items = Vec::new();
        for package in packages {
            let items_with_products = ASNItemEntity::find()
                .filter(crate::models::asn_item_entity::Column::AsnId.eq(package.asn_id))
                .all(db_pool)
                .await
                .map_err(|e| ServiceError::db_error(e))?;
            
            packages_with_items.push((package, items_with_products));
        }

        Ok(ASNDetails {
            asn,
            supplier,
            warehouse,
            // packages: packages_with_items,
            // purchase_order,
        })
    }
}

/// Query to get ASNs by date range
#[derive(Debug, Serialize, Deserialize)]
pub struct GetASNsByDateRangeQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: u64,
    pub offset: u64,
}

#[async_trait]
impl Query for GetASNsByDateRangeQuery {
    type Result = Vec<ASNModel>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        ASNEntity::find()
            .filter(crate::models::asn_entity::Column::ExpectedDeliveryDate.between(self.start_date, self.end_date))
            .order_by_desc(crate::models::asn_entity::Column::ExpectedDeliveryDate)
            .limit(self.limit)
            .offset(self.offset)
            .all(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))
    }
}

/// Query to search ASNs by tracking number
#[derive(Debug, Serialize, Deserialize)]
pub struct GetASNsByTrackingQuery {
    pub tracking_number: String,
}

#[async_trait]
impl Query for GetASNsByTrackingQuery {
    type Result = Vec<ASNModel>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        // Search both master tracking and package tracking numbers
        let condition = Condition::any()
            .add(crate::models::asn_entity::Column::TrackingNumber.eq(self.tracking_number.clone()));
            // Note: ASNItemEntity doesn't have a TrackingNumber column, so we only search main ASN tracking
            // .add(Expr::exists(
            //     Query::select()
            //         .from(ASNItemEntity)
            //         .cond_where(
            //             Expr::col(ASNItemEntity::Column::AsnId).equals(ASNEntity::Column::Id)
            //                 .and(ASNItemEntity::Column::TrackingNumber.eq(self.tracking_number.clone())),
            //         ),
            // ));

        ASNEntity::find()
            .filter(condition)
            .all(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))
    }
}

/// Statistics for ASN analysis
#[derive(Debug, Serialize)]
pub struct ASNStatistics {
    pub total_asns: i64,
    pub status_counts: Vec<(ASNStatus, i64)>,
    pub avg_processing_time: f64,
    pub on_time_delivery_rate: f64,
}

#[derive(Debug, FromQueryResult)]
struct StatusCount {
    status: ASNStatus,
    count: i64,
}

/// Query to get ASN statistics for a date range
#[derive(Debug, Serialize, Deserialize)]
pub struct GetASNStatisticsQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

#[async_trait]
impl Query for GetASNStatisticsQuery {
    type Result = ASNStatistics;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        // Get total count
        let total_asns = ASNEntity::find()
            .filter(crate::models::asn_entity::Column::CreatedAt.between(self.start_date, self.end_date))
            .count(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        // Get status counts
        let status_counts = ASNEntity::find()
            .select_only()
            .column(crate::models::asn_entity::Column::Status)
            .column_as(
                Expr::col((crate::models::asn_entity::Entity, crate::models::asn_entity::Column::Id)).count(),
                "count"
            )
            .filter(crate::models::asn_entity::Column::CreatedAt.between(self.start_date, self.end_date))
            .group_by(crate::models::asn_entity::Column::Status)
            .into_model::<StatusCount>()
            .all(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        // Convert to tuple format
        let status_counts: Vec<(ASNStatus, i64)> = status_counts
            .into_iter()
            .map(|sc| (sc.status, sc.count))
            .collect();

        // Calculate average processing time (using updated_at - created_at for completed ASNs)
        let avg_processing_time = ASNEntity::find()
            .select_only()
            .column_as(
                Expr::cust("AVG(EXTRACT(EPOCH FROM (updated_at - created_at)) / 3600)"),
                "avg_time",
            )
            .filter(
                crate::models::asn_entity::Column::CreatedAt
                    .between(self.start_date, self.end_date)
                    .and(crate::models::asn_entity::Column::Status.eq(ASNStatus::Completed.to_string())),
            )
            .into_tuple::<Option<f64>>()
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .flatten()
            .unwrap_or(0.0);

        // Calculate on-time delivery rate using expected_delivery_date and shipping_date
        let on_time_deliveries = ASNEntity::find()
            .select_only()
            .column_as(
                Expr::col((crate::models::asn_entity::Entity, crate::models::asn_entity::Column::Id)).count(),
                "count"
            )
            .filter(
                crate::models::asn_entity::Column::CreatedAt
                    .between(self.start_date, self.end_date)
                    .and(crate::models::asn_entity::Column::ShippingDate.is_not_null())
                    .and(crate::models::asn_entity::Column::ExpectedDeliveryDate.is_not_null())
                    .and(crate::models::asn_entity::Column::ShippingDate.lte(crate::models::asn_entity::Column::ExpectedDeliveryDate)),
            )
            .into_tuple::<i64>()
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .unwrap_or(0);

        let on_time_delivery_rate = if total_asns > 0 {
            (on_time_deliveries as f64 / total_asns as f64) * 100.0
        } else {
            0.0
        };

        Ok(ASNStatistics {
            total_asns: total_asns.try_into().unwrap_or(0),
            status_counts,
            avg_processing_time,
            on_time_delivery_rate,
        })
    }
}
