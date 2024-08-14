use serde::{Serialize, Deserialize};
use validator::{Validate, ValidationError};
use chrono::{NaiveDateTime, Utc};
use diesel::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Insertable, AsChangeset, Validate)]
#[table_name = "warranties"]
pub struct Warranty {
    pub id: i32,
    #[validate(range(min = 1, message = "Order ID must be positive"))]
    pub order_id: i32,
    #[validate(range(min = 1, message = "Customer ID must be positive"))]
    pub customer_id: i32,
    #[validate(range(min = 1, message = "Product ID must be positive"))]
    pub product_id: i32,
    #[validate(length(min = 1, max = 255, message = "Warranty number must be between 1 and 255 characters"))]
    pub warranty_number: String,
    pub status: WarrantyStatus,
    pub start_date: NaiveDateTime,
    pub end_date: NaiveDateTime,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[sql_type = "diesel::sql_types::Text"]
pub enum WarrantyStatus {
    Active,
    Expired,
    Claimed,
    Void,
}

#[derive(Debug, Clone, Serialize, Deserialize, Associations, Queryable, Insertable, Validate)]
#[belongs_to(Warranty)]
#[table_name = "warranty_claims"]
pub struct WarrantyClaim {
    pub id: i32,
    #[validate(range(min = 1, message = "Warranty ID must be positive"))]
    pub warranty_id: i32,
    pub claim_date: NaiveDateTime,
    #[validate(length(min = 1, max = 1000, message = "Description must be between 1 and 1000 characters"))]
    pub description: String,
    pub status: ClaimStatus,
    #[validate(length(max = 1000, message = "Resolution must not exceed 1000 characters"))]
    pub resolution: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[sql_type = "diesel::sql_types::Text"]
pub enum ClaimStatus {
    Submitted,
    UnderReview,
    Approved,
    Rejected,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize, Insertable, Validate)]
#[table_name = "warranties"]
pub struct NewWarranty {
    #[validate(range(min = 1, message = "Order ID must be positive"))]
    pub order_id: i32,
    #[validate(range(min = 1, message = "Customer ID must be positive"))]
    pub customer_id: i32,
    #[validate(range(min = 1, message = "Product ID must be positive"))]
    pub product_id: i32,
    #[validate(length(min = 1, max = 255, message = "Warranty number must be between 1 and 255 characters"))]
    pub warranty_number: String,
    pub start_date: NaiveDateTime,
    pub end_date: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, Insertable, Validate)]
#[table_name = "warranty_claims"]
pub struct NewWarrantyClaim {
    #[validate(range(min = 1, message = "Warranty ID must be positive"))]
    pub warranty_id: i32,
    pub claim_date: NaiveDateTime,
    #[validate(length(min = 1, max = 1000, message = "Description must be between 1 and 1000 characters"))]
    pub description: String,
}

impl Warranty {
    pub fn new(new_warranty: NewWarranty) -> Result<Self, ValidationError> {
        let now = Utc::now().naive_utc();
        let warranty = Self {
            id: 0, // Assuming database will auto-increment this
            order_id: new_warranty.order_id,
            customer_id: new_warranty.customer_id,
            product_id: new_warranty.product_id,
            warranty_number: new_warranty.warranty_number,
            status: WarrantyStatus::Active,
            start_date: new_warranty.start_date,
            end_date: new_warranty.end_date,
            created_at: now,
            updated_at: now,
        };
        warranty.validate()?;
        Ok(warranty)
    }

    pub fn update_status(&mut self, new_status: WarrantyStatus) -> Result<(), String> {
        if self.status == WarrantyStatus::Void {
            return Err("Cannot update status of a void warranty".into());
        }
        self.status = new_status;
        self.updated_at = Utc::now().naive_utc();
        Ok(())
    }

    pub fn is_valid(&self) -> bool {
        let now = Utc::now().naive_utc();
        self.status == WarrantyStatus::Active && self.start_date <= now && now <= self.end_date
    }
}

impl WarrantyStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            WarrantyStatus::Active => "Active",
            WarrantyStatus::Expired => "Expired",
            WarrantyStatus::Claimed => "Claimed",
            WarrantyStatus::Void => "Void",
        }
    }
}

impl WarrantyClaim {
    pub fn new(new_claim: NewWarrantyClaim) -> Result<Self, ValidationError> {
        let now = Utc::now().naive_utc();
        let claim = Self {
            id: 0, // Assuming database will auto-increment this
            warranty_id: new_claim.warranty_id,
            claim_date: new_claim.claim_date,
            description: new_claim.description,
            status: ClaimStatus::Submitted,
            resolution: None,
            created_at: now,
            updated_at: now,
        };
        claim.validate()?;
        Ok(claim)
    }

    pub fn update_status(&mut self, new_status: ClaimStatus) -> Result<(), String> {
        if self.status == ClaimStatus::Resolved {
            return Err("Cannot update status of a resolved claim".into());
        }
        self.status = new_status;
        self.updated_at = Utc::now().naive_utc();
        Ok(())
    }
}

impl ClaimStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClaimStatus::Submitted => "Submitted",
            ClaimStatus::UnderReview => "Under Review",
            ClaimStatus::Approved => "Approved",
            ClaimStatus::Rejected => "Rejected",
            ClaimStatus::Resolved => "Resolved",
        }
    }

    pub fn is_final(&self) -> bool {
        matches!(self, ClaimStatus::Resolved)
    }
}