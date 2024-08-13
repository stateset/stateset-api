use diesel::prelude::*;
use serde::{Serialize, Deserialize};
use validator::Validate;
use crate::schema::products;

#[derive(Debug, Serialize, Deserialize, Queryable, Insertable, AsChangeset, Validate)]
#[table_name = "products"]
pub struct Product {
    pub id: i32,
    #[validate(length(min = 1, max = 100))]
    pub sku: String,
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    #[validate(length(min = 0, max = 1000))]
    pub description: Option<String>,
    pub price: f64,
    pub stock_quantity: i32,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, Insertable, Validate)]
#[table_name = "products"]
pub struct NewProduct {
    #[validate(length(min = 1, max = 100))]
    pub sku: String,
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    #[validate(length(min = 0, max = 1000))]
    pub description: Option<String>,
    pub price: f64,
    pub stock_quantity: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProductSearchParams {
    pub sku: Option<String>,
    pub name: Option<String>,
    pub min_price: Option<f64>,
    pub max_price: Option<f64>,
    pub in_stock: Option<bool>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct StockAdjustment {
    pub product_id: i32,
    #[validate(range(min = -1000000, max = 1000000))]
    pub quantity_change: i32,
    #[validate(length(min = 0, max = 255))]
    pub reason: Option<String>,
}

#[get("/{id}/stock-history")]
async fn get_stock_history(
    pool: web::Data<DbPool>,
    id: web::Path<i32>,
    query: web::Query<DateRangeParams>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let stock_history = get_product_stock_history(&pool, id.into_inner(), query.into_inner()).await?;
    Ok(HttpResponse::Ok().json(stock_history))
}

#[post("/{id}/low-stock-threshold")]
async fn set_low_stock_threshold(
    pool: web::Data<DbPool>,
    id: web::Path<i32>,
    threshold: web::Json<i32>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let updated_threshold = set_product_low_stock_threshold(&pool, id.into_inner(), threshold.into_inner()).await?;
    Ok(HttpResponse::Ok().json(updated_threshold))
}