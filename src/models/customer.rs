use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::{Validate, ValidationError};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "customers")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    #[validate(length(
        min = 1,
        max = 100,
        message = "First name must be between 1 and 100 characters"
    ))]
    pub first_name: String,
    #[validate(length(
        min = 1,
        max = 100,
        message = "Last name must be between 1 and 100 characters"
    ))]
    pub last_name: String,
    #[validate(email(message = "Invalid email format"))]
    pub email: String,
    #[validate(phone(message = "Invalid phone number format"))]
    pub phone: String,
    #[validate(length(
        min = 1,
        max = 255,
        message = "Address must be between 1 and 255 characters"
    ))]
    pub address: String,
    #[validate(range(
        min = 0,
        max = 1000000,
        message = "Loyalty points must be between 0 and 1,000,000"
    ))]
    pub loyalty_points: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[validate(custom = "validate_birthdate")]
    pub birthdate: Option<DateTime<Utc>>,
    #[validate(length(
        min = 1,
        max = 50,
        message = "Country must be between 1 and 50 characters"
    ))]
    pub country: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

fn validate_birthdate(birthdate: &DateTime<Utc>) -> Result<(), validator::ValidationError> {
    if birthdate > &Utc::now() {
        return Err(validator::ValidationError::new("birthdate_future"));
    }
    Ok(())
}
