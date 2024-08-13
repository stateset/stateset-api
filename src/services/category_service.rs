use diesel::prelude::*;
use crate::db::DbPool;
use crate::models::product_category::{ProductCategory, ProductCategoryAssociation};
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