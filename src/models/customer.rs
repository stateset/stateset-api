use serde::{Serialize, Deserialize};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Queryable, Insertable, AsChangeset, Validate)]
#[table_name = "customers"]
pub struct Customer {
    pub id: i32,
    #[validate(length(min = 1, max = 255))]
    pub first_name: String,
    #[validate(length(min = 1, max = 255))]
    pub last_name: String,
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 1, max = 20))]
    pub phone: String,
    pub address: String,
    pub loyalty_points: i32,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}
