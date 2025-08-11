use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    prelude::*, EntityTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait,
    sea_query::{Func, Expr},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    db::DbPool, 
    errors::ServiceError, 
    models::*,
    entities::return_entity::{Entity as Return},
    entities::order_entity::{Entity as Order},
    entities::order_item_entity::{Entity as OrderItem},
    entities::product::{Entity as Product},
};

/// Trait representing a generic asynchronous query.
#[async_trait]
pub trait Query: Send + Sync {
    type Result: Send + Sync;

    /// Executes the query using the provided database pool.
    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError>;
}

// Note: DbPool is already a DatabaseConnection, so we can use it directly

/// Struct to get a specific return by ID.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetReturnByIdQuery {
    pub return_id: i32,
}

#[async_trait]
impl Query for GetReturnByIdQuery {
    type Result = Option<<crate::models::return_entity::Entity as sea_orm::EntityTrait>::Model>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
                Return::find_by_id(self.return_id)
            .one(db_pool)
            .await
            .map_err(|e| {
                log::error!("Database error in GetReturnByIdQuery: {:?}", e);
            })
    }
}

/// Struct to get all returns associated with a specific order.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetReturnsByOrderQuery {
    pub order_id: i32,
}

#[async_trait]
impl Query for GetReturnsByOrderQuery {
    type Result = Vec<<crate::models::return_entity::Entity as sea_orm::EntityTrait>::Model>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
                Return::find()
            .filter(<crate::models::return_entity::Entity as sea_orm::EntityTrait>::Column::OrderId.eq(self.order_id))
            .all(db_pool)
            .await
            .map_err(|e| {
                log::error!("Database error in GetReturnsByOrderQuery: {:?}", e);
            })
    }
}

/// Struct to get returns within a specific date range with pagination.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetReturnsByDateRangeQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: u64,
    pub offset: u64,
}

#[async_trait]
impl Query for GetReturnsByDateRangeQuery {
    type Result = Vec<<crate::models::return_entity::Entity as sea_orm::EntityTrait>::Model>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
                Return::find()
            .filter(<crate::models::return_entity::Entity as sea_orm::EntityTrait>::Column::CreatedAt.between(self.start_date, self.end_date))
            .order_by_desc(<crate::models::return_entity::Entity as sea_orm::EntityTrait>::Column::CreatedAt)
            .limit(self.limit)
            .offset(self.offset)
            .all(db_pool)
            .await
            .map_err(|e| {
                log::error!("Database error in GetReturnsByDateRangeQuery: {:?}", e);
            })
    }
}

/// Struct to get top return reasons within a date range.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetTopReturnReasonsQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: u64,
}

/// Struct representing a summary of a return reason.
#[derive(Debug, Serialize)]
pub struct ReturnReasonSummary {
    pub reason: String,
    pub count: i64,
}

#[async_trait]
impl Query for GetTopReturnReasonsQuery {
    type Result = Vec<ReturnReasonSummary>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        let summaries = Return::find()
            .select_only()
            .column(<crate::models::return_entity::Entity as sea_orm::EntityTrait>::Column::Reason)
            .column_as(Func::count(Expr::col("*")), "count")
            .filter(<crate::models::return_entity::Entity as sea_orm::EntityTrait>::Column::CreatedAt.between(self.start_date, self.end_date))
            .group_by(<crate::models::return_entity::Entity as sea_orm::EntityTrait>::Column::Reason)
            .order_by_desc(Func::count(Expr::col("*")))
            .limit(self.limit)
            .into_model::<ReturnReasonSummary>()
            .all(db_pool)
            .await
            .map_err(|e| {
                log::error!("Database error in GetTopReturnReasonsQuery: {:?}", e);
            })?;

        Ok(summaries)
    }
}

/// Struct to get the return rate within a specific date range.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetReturnRateQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

/// Struct representing the return rate statistics.
#[derive(Debug, Serialize)]
pub struct ReturnRate {
    pub total_orders: i64,
    pub total_returns: i64,
    pub return_rate: f64,
}

#[async_trait]
impl Query for GetReturnRateQuery {
    type Result = ReturnRate;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        // Fetch total number of orders within the date range
        let total_orders = Order::find()
            .filter(<crate::models::order_entity::Entity as sea_orm::EntityTrait>::Column::CreatedAt.between(self.start_date, self.end_date))
            .count(db_pool)
            .await
            .map_err(|e| {
                log::error!(
                    "Database error fetching total_orders in GetReturnRateQuery: {:?}",
                    e
                );
            })?;

        // Fetch total number of returns within the date range
        let total_returns = Return::find()
            .filter(<crate::models::return_entity::Entity as sea_orm::EntityTrait>::Column::CreatedAt.between(self.start_date, self.end_date))
            .count(db_pool)
            .await
            .map_err(|e| {
                log::error!(
                    "Database error fetching total_returns in GetReturnRateQuery: {:?}",
                    e
                );
            })?;

        // Calculate return rate
        let return_rate = if total_orders > 0 {
            (total_returns as f64 / total_orders as f64) * 100.0
        } else {
            0.0
        };

        Ok(ReturnRate {
            total_orders,
            total_returns,
            return_rate,
        })
    }
}

/// Struct to get products with the highest return rates within a date range.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetProductsWithHighestReturnRateQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: u64,
}

/// Struct representing the return rate statistics of a product.
#[derive(Debug, Serialize)]
pub struct ProductReturnRate {
    pub product_id: i32,
    pub product_name: String,
    pub total_sold: i64,
    pub total_returned: i64,
    pub return_rate: f64,
}

#[async_trait]
impl Query for GetProductsWithHighestReturnRateQuery {
    type Result = Vec<ProductReturnRate>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        // Subquery to calculate total sold per product
        let sold_subquery = OrderItem::find()
            .select_only()
            .column(<crate::models::order_item_entity::Entity as sea_orm::EntityTrait>::Column::ProductId)
            .column_as(Func::sum(<crate::models::order_item_entity::Entity as sea_orm::EntityTrait>::Column::Quantity), "total_sold")
            .join(
                sea_orm::JoinType::InnerJoin,
                <crate::models::order_item_entity::Entity as sea_orm::EntityTrait>::Relation::Order.def(),
            )
            .filter(<crate::models::order_entity::Entity as sea_orm::EntityTrait>::Column::CreatedAt.between(self.start_date, self.end_date))
            .group_by(<crate::models::order_item_entity::Entity as sea_orm::EntityTrait>::Column::ProductId)
            .into_tuple::<(i32, i64)>()
            .to_owned();

        let sold_data = sold_subquery.all(db_pool).await.map_err(|e| {
            log::error!(
                "Database error fetching total_sold in GetProductsWithHighestReturnRateQuery: {:?}",
                e
            );
        })?;

        // Subquery to calculate total returned per product
        let returned_subquery = Return::find()
            .select_only()
            .column(<crate::models::return_entity::Entity as sea_orm::EntityTrait>::Column::ProductId)
            .column_as(Func::count(<crate::models::return_entity::Entity as sea_orm::EntityTrait>::Column::Id), "total_returned")
            .filter(<crate::models::return_entity::Entity as sea_orm::EntityTrait>::Column::CreatedAt.between(self.start_date, self.end_date))
            .group_by(<crate::models::return_entity::Entity as sea_orm::EntityTrait>::Column::ProductId)
            .into_tuple::<(i32, i64)>()
            .to_owned();

        let returned_data = returned_subquery.all(db_pool).await.map_err(|e| {
            log::error!(
                "Database error fetching total_returned in GetProductsWithHighestReturnRateQuery: {:?}",
                e
            );
        })?;

        // Create hash maps for quick lookup
        let sold_map: std::collections::HashMap<i32, i64> = sold_data.into_iter().collect();
        let returned_map: std::collections::HashMap<i32, i64> = returned_data.into_iter().collect();

        // Fetch all products that have been sold
        let product_ids: Vec<i32> = sold_map.keys().cloned().collect();
        let products = Product::find()
            .filter(<crate::models::product::Entity as sea_orm::EntityTrait>::Column::Id.is_in(product_ids.clone()))
            .all(db_pool)
            .await
            .map_err(|e| {
                log::error!(
                    "Database error fetching products in GetProductsWithHighestReturnRateQuery: {:?}",
                    e
                );
            })?;

        // Map product IDs to product names
        let product_map: std::collections::HashMap<i32, String> =
            products.into_iter().map(|p| (p.id, p.name)).collect();

        // Combine the data into ProductReturnRate structs
        let mut product_return_rates: Vec<ProductReturnRate> = sold_map
            .into_iter()
            .filter_map(|(product_id, total_sold)| {
                product_map.get(&product_id).map(|product_name| {
                    let total_returned = returned_map.get(&product_id).cloned().unwrap_or(0);
                    let return_rate = if total_sold > 0 {
                        (total_returned as f64 / total_sold as f64) * 100.0
                    } else {
                        0.0
                    };
                    ProductReturnRate {
                        product_id,
                        product_name: product_name.clone(),
                        total_sold,
                        total_returned,
                        return_rate,
                    }
                })
            })
            .collect();

        // Sort the products by return rate in descending order and apply the limit
        product_return_rates.sort_by(|a, b| b.return_rate.partial_cmp(&a.return_rate).unwrap());
        product_return_rates.truncate(self.limit as usize);

        Ok(product_return_rates)
    }
}
