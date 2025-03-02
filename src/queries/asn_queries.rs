use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use chrono::{DateTime, Utc};

use sea_orm::{
    query::{Condition, Expr, Function},
    EntityTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait,
};

use crate::{
    errors::ServiceError,
    db::DbPool,
    models::*,
    asn_entity::ASN,
    asn_line_item::ASNLineItem,
    asn_package::ASNPackage,
    product::Product,
    supplier::Supplier,
    warehouse::Warehouse,
    purchase_order::PurchaseOrder,
};

/// Query to get a specific ASN by ID
#[derive(Debug, Serialize, Deserialize)]
pub struct GetASNQuery {
    pub asn_id: i32,
}

#[async_trait]
impl Query for GetASNQuery {
    type Result = Option<ASN>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = get_db(&db_pool).await?;
        ASN::find_by_id(self.asn_id)
            .one(&db)
            .await
            .map_err(|e| {
                log::error!("Database error in GetASNQuery: {:?}", e);
                ServiceError::DatabaseError
            })
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
    type Result = Vec<ASN>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = get_db(&db_pool).await?;
        ASN::find()
            .filter(ASN::Column::Status.eq(self.status))
            .limit(self.limit)
            .offset(self.offset)
            .order_by_desc(ASN::Column::ExpectedArrival)
            .all(&db)
            .await
            .map_err(|e| {
                log::error!("Database error in GetASNsByStatusQuery: {:?}", e);
                ServiceError::DatabaseError
            })
    }
}

/// Detailed ASN information including related entities
#[derive(Debug, Serialize)]
pub struct ASNDetails {
    pub asn: ASN,
    pub supplier: Supplier,
    pub warehouse: Warehouse,
    pub packages: Vec<(ASNPackage, Vec<(ASNLineItem, Product)>)>,
    pub purchase_order: Option<PurchaseOrder>,
}

/// Query to get detailed ASN information
#[derive(Debug, Serialize, Deserialize)]
pub struct GetASNDetailsQuery {
    pub asn_id: i32,
}

#[async_trait]
impl Query for GetASNDetailsQuery {
    type Result = ASNDetails;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = get_db(&db_pool).await?;

        // Fetch the ASN
        let asn = ASN::find_by_id(self.asn_id)
            .one(&db)
            .await
            .map_err(|e| {
                log::error!("Database error fetching ASN: {:?}", e);
                ServiceError::DatabaseError
            })?
            .ok_or(ServiceError::NotFound)?;

        // Fetch the supplier
        let supplier = Supplier::find_by_id(asn.supplier_id)
            .one(&db)
            .await
            .map_err(|e| {
                log::error!("Database error fetching supplier: {:?}", e);
                ServiceError::DatabaseError
            })?
            .ok_or(ServiceError::NotFound)?;

        // Fetch the warehouse
        let warehouse = Warehouse::find_by_id(asn.warehouse_id)
            .one(&db)
            .await
            .map_err(|e| {
                log::error!("Database error fetching warehouse: {:?}", e);
                ServiceError::DatabaseError
            })?
            .ok_or(ServiceError::NotFound)?;

        // Fetch purchase order if exists
        let purchase_order = if let Some(po_id) = asn.purchase_order_id {
            PurchaseOrder::find_by_id(po_id)
                .one(&db)
                .await
                .map_err(|e| {
                    log::error!("Database error fetching purchase order: {:?}", e);
                    ServiceError::DatabaseError
                })?
        } else {
            None
        };

        // Fetch packages with their items and products
        let packages = ASNPackage::find()
            .filter(ASNPackage::Column::AsnId.eq(self.asn_id))
            .all(&db)
            .await
            .map_err(|e| {
                log::error!("Database error fetching packages: {:?}", e);
                ServiceError::DatabaseError
            })?;

        let mut packages_with_items = Vec::new();
        for package in packages {
            let items_with_products = ASNLineItem::find()
                .filter(ASNLineItem::Column::PackageId.eq(package.id))
                .find_also_related(Product)
                .all(&db)
                .await
                .map_err(|e| {
                    log::error!("Database error fetching line items: {:?}", e);
                    ServiceError::DatabaseError
                })?
                .into_iter()
                .filter_map(|(item, product)| product.map(|p| (item, p)))
                .collect();

            packages_with_items.push((package, items_with_products));
        }

        Ok(ASNDetails {
            asn,
            supplier,
            warehouse,
            packages: packages_with_items,
            purchase_order,
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
    type Result = Vec<ASN>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = get_db(&db_pool).await?;
        ASN::find()
            .filter(ASN::Column::ExpectedArrival.between(self.start_date, self.end_date))
            .order_by_desc(ASN::Column::ExpectedArrival)
            .limit(self.limit)
            .offset(self.offset)
            .all(&db)
            .await
            .map_err(|e| {
                log::error!("Database error in GetASNsByDateRangeQuery: {:?}", e);
                ServiceError::DatabaseError
            })
    }
}

/// Query to get ASNs by tracking number
#[derive(Debug, Serialize, Deserialize)]
pub struct GetASNsByTrackingQuery {
    pub tracking_number: String,
}

#[async_trait]
impl Query for GetASNsByTrackingQuery {
    type Result = Vec<ASN>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = get_db(&db_pool).await?;
        
        // Search both master tracking and package tracking numbers
        let condition = Condition::any()
            .add(ASN::Column::MasterTrackingNumber.eq(self.tracking_number.clone()))
            .add(
                Expr::exists(
                    ASNPackage::find()
                        .filter(
                            ASNPackage::Column::TrackingNumber.eq(self.tracking_number.clone())
                                .and(ASNPackage::Column::AsnId.eq(ASN::Column::Id))
                        )
                        .into_query()
                )
            );

        ASN::find()
            .filter(condition)
            .all(&db)
            .await
            .map_err(|e| {
                log::error!("Database error in GetASNsByTrackingQuery: {:?}", e);
                ServiceError::DatabaseError
            })
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

/// Query to get ASN statistics for a date range
#[derive(Debug, Serialize, Deserialize)]
pub struct GetASNStatisticsQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

#[async_trait]
impl Query for GetASNStatisticsQuery {
    type Result = ASNStatistics;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = get_db(&db_pool).await?;

        // Get total count
        let total_asns = ASN::find()
            .filter(ASN::Column::CreatedAt.between(self.start_date, self.end_date))
            .count(&db)
            .await
            .map_err(|e| {
                log::error!("Database error counting ASNs: {:?}", e);
                ServiceError::DatabaseError
            })?;

        // Get status counts
        let status_counts = ASN::find()
            .select_only()
            .column(ASN::Column::Status)
            .column_as(Function::Count(ASN::Column::Id), "count")
            .filter(ASN::Column::CreatedAt.between(self.start_date, self.end_date))
            .group_by(ASN::Column::Status)
            .into_model::<(ASNStatus, i64)>()
            .all(&db)
            .await
            .map_err(|e| {
                log::error!("Database error getting status counts: {:?}", e);
                ServiceError::DatabaseError
            })?;

        // Calculate average processing time
        let avg_processing_time = ASN::find()
            .select_only()
            .column_as(
                Function::Avg(
                    Expr::expr(ASN::Column::ReceivedAt.if_null(Utc::now()))
                        .sub(ASN::Column::CreatedAt)
                ),
                "avg_time"
            )
            .filter(
                ASN::Column::CreatedAt
                    .between(self.start_date, self.end_date)
                    .and(ASN::Column::Status.eq(ASNStatus::Received))
            )
            .into_model::<Option<f64>>()
            .one(&db)
            .await
            .map_err(|e| {
                log::error!("Database error calculating processing time: {:?}", e);
                ServiceError::DatabaseError
            })?
            .unwrap_or(None)
            .unwrap_or(0.0);

        // Calculate on-time delivery rate
        let on_time_deliveries = ASN::find()
            .select_only()
            .column_as(
                Function::Count(ASN::Column::Id),
                "count"
            )
            .filter(
                ASN::Column::CreatedAt
                    .between(self.start_date, self.end_date)
                    .and(ASN::Column::ReceivedAt.is_not_null())
                    .and(ASN::Column::ReceivedAt.lte(ASN::Column::ExpectedArrival))
            )
            .into_model::<i64>()
            .one(&db)
            .await
            .map_err(|e| {
                log::error!("Database error calculating on-time deliveries: {:?}", e);
                ServiceError::DatabaseError
            })?
            .unwrap_or(0);

        let on_time_delivery_rate = if total_asns > 0 {
            (on_time_deliveries as f64 / total_asns as f64) * 100.0
        } else {
            0.0
        };

        Ok(ASNStatistics {
            total_asns,
            status_counts,
            avg_processing_time,
            on_time_delivery_rate,
        })
    }
}