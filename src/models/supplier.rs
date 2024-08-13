use serde::{Serialize, Deserialize};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Queryable, Insertable, AsChangeset, Validate)]
#[table_name = "suppliers"]
pub struct Supplier {
    pub id: i32,
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 1, max = 20))]
    pub phone: String,
    #[validate(length(min = 1, max = 255))]
    pub address: String,
    pub is_active: bool,
    pub credit_terms: Option<i32>,
    pub payment_method: PaymentMethod,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[sql_type = "diesel::sql_types::Text"]
pub enum PaymentMethod {
    CreditCard,
    BankTransfer,
    Check,
    Cash,
}

#[derive(Debug, Serialize, Deserialize, Associations, Queryable, Insertable)]
#[belongs_to(Supplier)]
#[belongs_to(Product)]
#[table_name = "supplier_products"]
pub struct SupplierProduct {
    pub supplier_id: i32,
    pub product_id: i32,
    pub unit_price: f64,
    pub lead_time_days: i32,
}