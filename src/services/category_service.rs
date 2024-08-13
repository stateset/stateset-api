use diesel::prelude::*;
use crate::db::DbPool;
use crate::models::product_category::{ProductCategory, ProductCategoryAssociation};
use crate::models::product::Product;
use crate::errors::ServiceError;
use crate::schema::{product_categories, product_category_associations, products};

pub async fn create_category(pool: &DbPool, new_category: ProductCategory) -> Result<ProductCategory, ServiceError> {
    let conn = pool.get()?;
    let category = diesel::insert_into(product_categories::table)
        .values(&new_category)
        .get_result::<ProductCategory>(&conn)?;
    Ok(category)
}

pub async fn get_category(pool: &DbPool, id: i32) -> Result<ProductCategory, ServiceError> {
    let conn = pool.get()?;
    let category = product_categories::table
        .filter(product_categories::id.eq(id))
        .first::<ProductCategory>(&conn)?;
    Ok(category)
}

pub async fn update_category(pool: &DbPool, id: i32, updated_category: ProductCategory) -> Result<ProductCategory, ServiceError> {
    let conn = pool.get()?;
    let category = diesel::update(product_categories::table)
        .filter(product_categories::id.eq(id))
        .set(&updated_category)
        .get_result::<ProductCategory>(&conn)?;
    Ok(category)
}

pub async fn delete_category(pool: &DbPool, id: i32) -> Result<(), ServiceError> {
    let conn = pool.get()?;
    diesel::delete(product_categories::table)
        .filter(product_categories::id.eq(id))
        .execute(&conn)?;
    Ok(())
}

pub async fn list_categories(pool: &DbPool) -> Result<Vec<ProductCategory>, ServiceError> {
    let conn = pool.get()?;
    let categories = product_categories::table
        .load::<ProductCategory>(&conn)?;
    Ok(categories)
}

pub async fn get_category_products(pool: &DbPool, category_id: i32, pagination: PaginationParams) -> Result<Vec<Product>, ServiceError> {
    let conn = pool.get()?;
    let products = Product::belonging_to(&ProductCategory::find(category_id))
        .inner_join(product_category_associations::table)
        .select(products::all_columns)
        .limit(pagination.limit)
        .offset(pagination.offset)
        .load::<Product>(&conn)?;
    Ok(products)
}