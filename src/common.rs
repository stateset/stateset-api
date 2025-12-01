/// Common types and utilities shared across handlers and services
use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::errors::ApiError;

/// Date range parameters for filtering queries
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DateRangeParams {
    pub start_date: String,
    pub end_date: String,
}

impl DateRangeParams {
    /// Converts string dates to NaiveDateTime
    pub fn to_datetime_range(&self) -> Result<(NaiveDateTime, NaiveDateTime), ApiError> {
        let start_date = NaiveDate::parse_from_str(&self.start_date, "%Y-%m-%d")
            .map_err(|e| ApiError::ValidationError(format!("Invalid start date format: {}", e)))?;

        let end_date = NaiveDate::parse_from_str(&self.end_date, "%Y-%m-%d")
            .map_err(|e| ApiError::ValidationError(format!("Invalid end date format: {}", e)))?;

        let start_datetime = start_date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| ApiError::ValidationError("Invalid start date time".to_string()))?;

        let end_datetime = end_date
            .and_hms_opt(23, 59, 59)
            .ok_or_else(|| ApiError::ValidationError("Invalid end date time".to_string()))?;

        Ok((start_datetime, end_datetime))
    }
}
