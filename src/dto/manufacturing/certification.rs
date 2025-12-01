use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::entities::manufacturing::robot_certification::{CertStatus, CertificationType};

#[derive(Debug, Deserialize, Validate, Serialize)]
pub struct CreateCertificationRequest {
    pub robot_serial_id: Uuid,
    pub certification_type: CertificationType,
    pub certification_number: Option<String>,
    pub issuing_authority: Option<String>,
    pub issue_date: NaiveDate,
    pub expiration_date: Option<NaiveDate>,
    pub certification_scope: Option<String>,
    pub certificate_document_url: Option<String>,
}

#[derive(Debug, Deserialize, Validate, Serialize)]
pub struct UpdateCertificationRequest {
    pub status: Option<CertStatus>,
    pub expiration_date: Option<NaiveDate>,
    pub certificate_document_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CertificationResponse {
    pub id: Uuid,
    pub robot_serial_id: Uuid,
    pub robot_serial_number: Option<String>,
    pub certification_type: CertificationType,
    pub certification_number: Option<String>,
    pub issuing_authority: Option<String>,
    pub issue_date: NaiveDate,
    pub expiration_date: Option<NaiveDate>,
    pub certification_scope: Option<String>,
    pub certificate_document_url: Option<String>,
    pub status: CertStatus,
    pub is_valid: bool,
    pub days_until_expiration: Option<i64>,
    pub needs_renewal: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct RobotCertificationSummary {
    pub robot_serial_id: Uuid,
    pub robot_serial_number: String,
    pub certifications: Vec<CertificationSummaryItem>,
    pub all_valid: bool,
    pub expiring_soon: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CertificationSummaryItem {
    pub certification_type: CertificationType,
    pub is_valid: bool,
    pub expiration_date: Option<NaiveDate>,
    pub needs_renewal: bool,
}
