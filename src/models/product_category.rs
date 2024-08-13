use serde::{Serialize, Deserialize};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Queryable, Insertable, AsChangeset, Validate)]
#[table_name = "product_categories"]
pub struct ProductCategory {
    pub id: i32,
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    pub parent_id: Option<i32>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Associations, Queryable, Insertable)]
#[belongs_to(Product)]
#[belongs_to(ProductCategory)]
#[table_name = "product_category_associations"]
pub struct ProductCategoryAssociation {
    pub product_id: i32,
    pub category_id: i32,
}