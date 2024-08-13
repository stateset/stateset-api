use actix_web::{error::ResponseError, HttpResponse};
use derive_more::Display;
use diesel::result::Error as DieselError;
use std::convert::From;
use validator::ValidationErrors;

#[derive(Debug, Display)]
pub enum ServiceError {
    #[display(fmt = "Internal Server Error")]
    InternalServerError,

    #[display(fmt = "BadRequest: {}", _0)]
    BadRequest(String),

    #[display(fmt = "Unauthorized")]
    Unauthorized,

    #[display(fmt = "Not Found")]
    NotFound,
}

impl ResponseError for ServiceError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ServiceError::InternalServerError => HttpResponse::InternalServerError().json("Internal Server Error"),
            ServiceError::BadRequest(ref message) => HttpResponse::BadRequest().json(message),
            ServiceError::Unauthorized => HttpResponse::Unauthorized().json("Unauthorized"),
            ServiceError::NotFound => HttpResponse::NotFound().json("Not Found"),
        }
    }
}

impl From<DieselError> for ServiceError {
    fn from(error: DieselError) -> ServiceError {
        match error {
            DieselError::NotFound => ServiceError::NotFound,
            _ => ServiceError::InternalServerError,
        }
    }
}

impl From<ValidationErrors> for ServiceError {
    fn from(errors: ValidationErrors) -> ServiceError {
        ServiceError::BadRequest(format!("Validation error: {:?}", errors))
    }
}