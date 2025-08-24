use chrono::{DateTime, NaiveDate, Utc, Datelike};
use async_trait::async_trait;
// use phonelib::PhoneValidator; // Commented out - dependency not available
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, Set, Condition, DatabaseConnection, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
use validator::{Validate, ValidationError};
use uuid::Uuid;

/// Custom error type for supplier operations
#[derive(Error, Debug)]
pub enum SupplierError {
    #[error("Database error: {0}")]
    Database(#[from] DbErr),

    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Operation error: {0}")]
    Operation(String),
}

/// Supplier status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum SupplierStatus {
    #[sea_orm(string_value = "Active")]
    Active,

    #[sea_orm(string_value = "Inactive")]
    Inactive,

    #[sea_orm(string_value = "OnHold")]
    OnHold,

    #[sea_orm(string_value = "Blacklisted")]
    Blacklisted,
}

impl fmt::Display for SupplierStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SupplierStatus::Active => write!(f, "Active"),
            SupplierStatus::Inactive => write!(f, "Inactive"),
            SupplierStatus::OnHold => write!(f, "On Hold"),
            SupplierStatus::Blacklisted => write!(f, "Blacklisted"),
        }
    }
}

/// Supplier rating enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum SupplierRating {
    #[sea_orm(string_value = "Unrated")]
    Unrated,

    #[sea_orm(string_value = "Bronze")]
    Bronze,

    #[sea_orm(string_value = "Silver")]
    Silver,

    #[sea_orm(string_value = "Gold")]
    Gold,

    #[sea_orm(string_value = "Platinum")]
    Platinum,
}

impl fmt::Display for SupplierRating {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SupplierRating::Unrated => write!(f, "Unrated"),
            SupplierRating::Bronze => write!(f, "Bronze"),
            SupplierRating::Silver => write!(f, "Silver"),
            SupplierRating::Gold => write!(f, "Gold"),
            SupplierRating::Platinum => write!(f, "Platinum"),
        }
    }
}

/// Supplier entity model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "suppliers")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,

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

    #[validate(custom = "validate_phone")]
    pub phone: String,

    #[validate(length(
        min = 1,
        max = 255,
        message = "Address must be between 1 and 255 characters"
    ))]
    pub address: String,

    #[validate(length(
        min = 1,
        max = 50,
        message = "City must be between 1 and 50 characters"
    ))]
    pub city: String,

    #[validate(length(
        min = 1,
        max = 50,
        message = "State/province must be between 1 and 50 characters"
    ))]
    pub state_province: String,

    #[validate(length(
        min = 1,
        max = 20,
        message = "Postal code must be between 1 and 20 characters"
    ))]
    pub postal_code: String,

    #[validate(length(
        min = 1,
        max = 50,
        message = "Country must be between 1 and 50 characters"
    ))]
    pub country: String,

    #[validate(range(
        min = 0,
        max = 1000000,
        message = "Loyalty points must be between 0 and 1,000,000"
    ))]
    pub loyalty_points: i32,

    #[validate(custom = "validate_birthdate")]
    pub birthdate: Option<NaiveDate>,

    pub status: SupplierStatus,

    pub rating: SupplierRating,

    pub created_at: DateTime<Utc>,

    pub updated_at: DateTime<Utc>,

    pub company_name: Option<String>,

    pub tax_id: Option<String>,

    #[validate(url(message = "Invalid website URL format"))]
    pub website: Option<String>,

    pub notes: Option<String>,

    pub preferred_payment_method: Option<String>,

    pub payment_terms: Option<String>,

    pub credit_limit: Option<f64>,

    pub is_international: bool,

    #[validate(email(message = "Invalid secondary email format"))]
    pub secondary_email: Option<String>,

    #[validate(custom = "validate_optional_phone")]
    pub secondary_phone: Option<String>,

    pub tags: Option<String>,

    pub last_order_date: Option<DateTime<Utc>>,

    pub total_orders: i32,

    pub average_fulfillment_days: Option<f32>,
}

/// Database relations
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    // TODO: Add these relations when the related entities have proper belongs_to relations
    // #[sea_orm(has_many = "super::product::Entity")]
    // Products,

    // #[sea_orm(has_many = "super::purchase_order::Entity")]
    // PurchaseOrders,

    // #[sea_orm(has_many = "super::supplier_contact::Entity")]
    // Contacts,
}

impl Related<super::product::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Products.def()
    }
}

// TODO: Uncomment when purchase_order entity is implemented
// impl Related<super::purchase_order::Entity> for Entity {
//     fn to() -> RelationDef {
//         Relation::PurchaseOrders.def()
//     }
// }

impl Related<super::supplier_contact::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contacts.def()
    }
}

/// Custom validation for birthdates
fn validate_birthdate(date: &NaiveDate) -> Result<(), ValidationError> {
    let now = Utc::now().date_naive();

    // Must be at least 18 years old
    let min_age_date =
        NaiveDate::from_ymd_opt(now.year() - 18, now.month(), now.day().min(date.day()))
            .unwrap_or(now);

    if *date > min_age_date {
        return Err(ValidationError::new("must_be_18_or_older"));
    }

    // Must not be more than 100 years old
    let max_age_date = NaiveDate::from_ymd_opt(
        now.year() - 100,
        now.month(),
        now.day().min(date.day()),
    )
    .unwrap_or(now);

    if *date < max_age_date {
        return Err(ValidationError::new("invalid_birthdate"));
    }

    Ok(())
}

/// Custom validation for phone numbers
fn validate_phone(phone: &String) -> Result<(), ValidationError> {
    // Basic phone validation - just check if it's not empty and has reasonable length
    if phone.is_empty() {
        return Err(ValidationError::new("Phone number cannot be empty"));
    }
    if phone.len() < 7 || phone.len() > 20 {
        return Err(ValidationError::new("Invalid phone number length"));
    }
    // Could add more sophisticated validation here
    Ok(())
}

/// Custom validation for optional phone numbers
fn validate_optional_phone(phone: &String) -> Result<(), ValidationError> {
    // Same validation as regular phone
    validate_phone(phone)
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C: ConnectionTrait>(
        self,
        _db: &C,
        insert: bool,
    ) -> Result<Self, DbErr> {
        let mut active_model = self;
        if insert {
            active_model.set_id_if_needed();
        }
        Ok(active_model)
    }
}

impl ActiveModel {
    fn set_id_if_needed(&mut self) {
        if self.id.is_not_set() {
            self.id = Set(Uuid::new_v4());
        }
    }
}

impl Model {
    /// Create a new supplier
    pub fn new(
        first_name: String,
        last_name: String,
        email: String,
        phone: String,
        address: String,
        city: String,
        state_province: String,
        postal_code: String,
        country: String,
    ) -> Result<Self, ValidationError> {
        let now = Utc::now();
        let supplier = Self {
            id: 0, // Will be set by database
            first_name,
            last_name,
            email,
            phone,
            address,
            city,
            state_province,
            postal_code,
            country,
            loyalty_points: 0,
            birthdate: None,
            status: SupplierStatus::Active,
            rating: SupplierRating::Unrated,
            created_at: now,
            updated_at: now,
            company_name: None,
            tax_id: None,
            website: None,
            notes: None,
            preferred_payment_method: None,
            payment_terms: None,
            credit_limit: None,
            is_international: false,
            secondary_email: None,
            secondary_phone: None,
            tags: None,
            last_order_date: None,
            total_orders: 0,
            average_fulfillment_days: None,
        };

        supplier.validate().map_err(|_| ValidationError::new("Supplier validation failed"))?;
        Ok(supplier)
    }

    /// Get full name
    pub fn full_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }

    /// Get full address
    pub fn full_address(&self) -> String {
        format!(
            "{}, {}, {}, {}, {}",
            self.address, self.city, self.state_province, self.postal_code, self.country
        )
    }

    /// Update supplier status
    pub fn update_status(&mut self, new_status: SupplierStatus) -> Result<(), ValidationError> {
        self.status = new_status;
        self.updated_at = Utc::now();

        self.validate().map_err(|_| ValidationError::new("Supplier validation failed"))?;
        Ok(())
    }

    /// Update supplier rating
    pub fn update_rating(&mut self, new_rating: SupplierRating) -> Result<(), ValidationError> {
        self.rating = new_rating;
        self.updated_at = Utc::now();

        self.validate().map_err(|_| ValidationError::new("Supplier validation failed"))?;
        Ok(())
    }

    /// Add loyalty points
    pub fn add_loyalty_points(&mut self, points: i32) -> Result<(), ValidationError> {
        self.loyalty_points += points;
        self.updated_at = Utc::now();

        self.validate().map_err(|_| ValidationError::new("Supplier validation failed"))?;
        Ok(())
    }

    /// Update contact information
    pub fn update_contact_info(
        &mut self,
        email: Option<String>,
        phone: Option<String>,
        website: Option<String>,
        secondary_email: Option<String>,
        secondary_phone: Option<String>,
    ) -> Result<(), ValidationError> {
        if let Some(email_str) = email {
            self.email = email_str;
        }

        if let Some(phone_str) = phone {
            self.phone = phone_str;
        }

        self.website = website;
        self.secondary_email = secondary_email;
        self.secondary_phone = secondary_phone;
        self.updated_at = Utc::now();

        self.validate().map_err(|_| ValidationError::new("Supplier validation failed"))?;
        Ok(())
    }

    /// Update address information
    pub fn update_address(
        &mut self,
        address: String,
        city: String,
        state_province: String,
        postal_code: String,
        country: String,
    ) -> Result<(), ValidationError> {
        self.address = address;
        self.city = city;
        self.state_province = state_province;
        self.postal_code = postal_code;
        self.country = country.clone();

        // Update international status based on country
        // This is a simplified check, in a real system you might have a list of international countries
        self.is_international = country != "United States";

        self.updated_at = Utc::now();

        self.validate().map_err(|_| ValidationError::new("Supplier validation failed"))?;
        Ok(())
    }

    /// Update company information
    pub fn update_company_info(
        &mut self,
        company_name: Option<String>,
        tax_id: Option<String>,
    ) -> Result<(), ValidationError> {
        self.company_name = company_name;
        self.tax_id = tax_id;
        self.updated_at = Utc::now();

        self.validate().map_err(|_| ValidationError::new("Supplier validation failed"))?;
        Ok(())
    }

    /// Update payment information
    pub fn update_payment_info(
        &mut self,
        preferred_payment_method: Option<String>,
        payment_terms: Option<String>,
        credit_limit: Option<f64>,
    ) -> Result<(), ValidationError> {
        self.preferred_payment_method = preferred_payment_method;
        self.payment_terms = payment_terms;
        self.credit_limit = credit_limit;
        self.updated_at = Utc::now();

        self.validate().map_err(|_| ValidationError::new("Supplier validation failed"))?;
        Ok(())
    }

    /// Add tags to supplier
    pub fn add_tags(&mut self, new_tags: Vec<String>) -> Result<(), ValidationError> {
        let current_tags: Vec<String> = self
            .tags
            .clone()
            .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();

        let mut updated_tags = current_tags;
        for tag in new_tags {
            if !updated_tags.contains(&tag) {
                updated_tags.push(tag);
            }
        }

        self.tags = Some(updated_tags.join(", "));
        self.updated_at = Utc::now();

        self.validate().map_err(|_| ValidationError::new("Supplier validation failed"))?;
        Ok(())
    }

    /// Remove tags from supplier
    pub fn remove_tags(&mut self, tags_to_remove: Vec<String>) -> Result<(), ValidationError> {
        if let Some(tags_str) = &self.tags {
            let current_tags: Vec<String> =
                tags_str.split(',').map(|s| s.trim().to_string()).collect();

            let updated_tags: Vec<String> = current_tags
                .into_iter()
                .filter(|tag| !tags_to_remove.contains(tag))
                .collect();

            self.tags = if updated_tags.is_empty() {
                None
            } else {
                Some(updated_tags.join(", "))
            };

            self.updated_at = Utc::now();
        }

        self.validate().map_err(|_| ValidationError::new("Supplier validation failed"))?;
        Ok(())
    }

    /// Record a new order for the supplier
    pub fn record_order(&mut self, fulfillment_days: Option<f32>) -> Result<(), ValidationError> {
        self.total_orders += 1;
        self.last_order_date = Some(Utc::now());

        // Update average fulfillment days if provided
        if let Some(days) = fulfillment_days {
            let current_avg = self.average_fulfillment_days.unwrap_or(days);
            let new_avg = ((current_avg * (self.total_orders as f32 - 1.0)) + days)
                / self.total_orders as f32;
            self.average_fulfillment_days = Some(new_avg);
        }

        self.updated_at = Utc::now();

        self.validate().map_err(|_| ValidationError::new("Supplier validation failed"))?;
        Ok(())
    }

    /// Save the supplier to database
    pub async fn save(&self, db: &DatabaseConnection) -> Result<Model, SupplierError> {
        // Validate before saving
        self.validate().map_err(|_| ValidationError::new("Supplier validation failed"))?;

        let model: ActiveModel = self.clone().into();
        let result = match self.id {
            0 => model.insert(db).await?,
            _ => model.update(db).await?,
        };

        Ok(result)
    }

    /// Find a supplier by ID
    pub async fn find_by_id(db: &DatabaseConnection, id: i32) -> Result<Option<Model>, DbErr> {
        Entity::find_by_id(id).one(db).await
    }

    /// Find suppliers by country
    pub async fn find_by_country(
        db: &DatabaseConnection,
        country: &str,
    ) -> Result<Vec<Model>, DbErr> {
        Entity::find()
            .filter(Column::Country.eq(country))
            .all(db)
            .await
    }

    /// Find suppliers by status
    pub async fn find_by_status(
        db: &DatabaseConnection,
        status: SupplierStatus,
    ) -> Result<Vec<Model>, DbErr> {
        Entity::find()
            .filter(Column::Status.eq(status))
            .all(db)
            .await
    }

    /// Find suppliers by rating
    pub async fn find_by_rating(
        db: &DatabaseConnection,
        rating: SupplierRating,
    ) -> Result<Vec<Model>, DbErr> {
        Entity::find()
            .filter(Column::Rating.eq(rating))
            .all(db)
            .await
    }

    /// Search suppliers by name (first or last)
    pub async fn search_by_name(db: &DatabaseConnection, name: &str) -> Result<Vec<Model>, DbErr> {
        let name_pattern = format!("%{}%", name);

        Entity::find()
            .filter(
                Condition::any()
                    .add(Column::FirstName.like(&name_pattern))
                    .add(Column::LastName.like(&name_pattern))
                    .add(
                        Condition::all()
                            .add(Column::CompanyName.is_not_null())
                            .add(Column::CompanyName.like(&name_pattern)),
                    ),
            )
            .all(db)
            .await
    }

    /// Find top suppliers by order count
    pub async fn find_top_suppliers(
        db: &DatabaseConnection,
        limit: u64,
    ) -> Result<Vec<Model>, DbErr> {
        Entity::find()
            .filter(Column::Status.eq(SupplierStatus::Active))
            .order_by_desc(Column::TotalOrders)
            .limit(limit)
            .all(db)
            .await
    }
}
