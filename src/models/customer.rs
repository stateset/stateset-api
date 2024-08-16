use validator::Validate;
use chrono::NaiveDateTime;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Insertable)]
#[table_name = "customers"]
pub struct Customer {
    pub id: i32,
    #[validate(length(min = 1, max = 100, message = "First name must be between 1 and 100 characters"))]
    pub first_name: String,
    #[validate(length(min = 1, max = 100, message = "Last name must be between 1 and 100 characters"))]
    pub last_name: String,
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
    #[validate(phone(message = "Invalid phone number format"))]
    pub phone: String,
    #[validate(length(min = 1, max = 255, message = "Address must be between 1 and 255 characters"))]
    pub address: String,
    #[validate(range(min = 0, max = 1000000, message = "Loyalty points must be between 0 and 1,000,000"))]
    pub loyalty_points: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    #[validate(custom = "validate_birthdate")]
    pub birthdate: Option<NaiveDateTime>,
    #[validate(length(min = 1, max = 50, message = "Country must be between 1 and 50 characters"))]
    pub country: String,
}
