use serde::{Serialize, Deserialize};
use chrono::NaiveDateTime;

#[derive(Debug, Serialize, Deserialize, Queryable, Insertable)]
#[table_name = "inventory_transactions"]
pub struct InventoryTransaction {
    pub id: i32,
    pub product_id: i32,
    pub quantity_change: i32,
    pub transaction_type: InventoryTransactionType,
    pub reference_id: Option<i32>,
    pub notes: Option<String>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[sql_type = "diesel::sql_types::Text"]
pub enum InventoryTransactionType {
    Purchase,
    Sale,
    Adjustment,
    Return,
}

#[derive(Debug, Serialize, Deserialize, Queryable, Insertable, AsChangeset)]
#[table_name = "product_stock_thresholds"]
pub struct ProductStockThreshold {
    pub product_id: i32,
    pub low_stock_threshold: i32,
}