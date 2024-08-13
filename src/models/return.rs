use serde::{Serialize, Deserialize};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Queryable, Insertable, AsChangeset, Validate)]
#[table_name = "returns"]
pub struct Return {
    pub id: i32,
    pub order_id: i32,
    pub customer_id: i32,
    pub status: ReturnStatus,
    pub reason: String,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[sql_type = "diesel::sql_types::Text"]
pub enum ReturnStatus {
    Requested,
    Approved,
    Rejected,
    Received,
    Refunded,
}

#[derive(Debug, Serialize, Deserialize, Associations, Queryable, Insertable)]
#[belongs_to(Return)]
#[belongs_to(Product)]
#[table_name = "return_items"]
pub struct ReturnItem {
    pub id: i32,
    pub return_id: i32,
    pub product_id: i32,
    pub quantity: i32,
    pub reason: String,
}