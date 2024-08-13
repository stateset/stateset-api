use serde::{Serialize, Deserialize};
use validator::Validate;
use chrono::NaiveDateTime;
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Queryable, Insertable, AsChangeset, Validate)]
#[table_name = "warranties"]
pub struct Warranty {
    pub id: i32,
    pub order_id: i32,
    pub customer_id: i32,
    pub product_id: i32,
    #[validate(length(min = 1, max = 255))]
    pub warranty_number: String,
    pub status: WarrantyStatus,
    pub start_date: NaiveDateTime,
    pub end_date: NaiveDateTime,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[sql_type = "diesel::sql_types::Text"]
pub enum WarrantyStatus {
    Active,
    Expired,
    Claimed,
    Void,
}

#[derive(Debug, Serialize, Deserialize, Associations, Queryable, Insertable)]
#[belongs_to(Warranty)]
#[table_name = "warranty_claims"]
pub struct WarrantyClaim {
    pub id: i32,
    pub warranty_id: i32,
    pub claim_date: NaiveDateTime,
    pub description: String,
    pub status: ClaimStatus,
    pub resolution: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[sql_type = "diesel::sql_types::Text"]
pub enum ClaimStatus {
    Submitted,
    UnderReview,
    Approved,
    Rejected,
    Resolved,
}

// New struct for creating a warranty
#[derive(Debug, Serialize, Deserialize, Insertable, Validate)]
#[table_name = "warranties"]
pub struct NewWarranty {
    pub order_id: i32,
    pub customer_id: i32,
    pub product_id: i32,
    #[validate(length(min = 1, max = 255))]
    pub warranty_number: String,
    pub start_date: NaiveDateTime,
    pub end_date: NaiveDateTime,
}

// New struct for creating a warranty claim
#[derive(Debug, Serialize, Deserialize, Insertable, Validate)]
#[table_name = "warranty_claims"]
pub struct NewWarrantyClaim {
    pub warranty_id: i32,
    pub claim_date: NaiveDateTime,
    #[validate(length(min = 1, max = 1000))]
    pub description: String,
}