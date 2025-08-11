use axum::{
    extract::{Json, State},
    response::IntoResponse,
    routing::post,
    Router,
};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::{
    errors::ApiError,
    handlers::common::{created_response, map_service_error, validate_input},
    AppState,
};

#[derive(Debug, Deserialize, Validate)]
pub struct CreateCashSaleRequest {
    pub order_id: Uuid,
    pub amount: Decimal,
    pub payment_method: String,
}

async fn create_cash_sale(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateCashSaleRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;
    let id = state
        .services
        .cash_sales
        .create_cash_sale(payload.order_id, payload.amount, payload.payment_method)
        .await
        .map_err(map_service_error)?;
    Ok(created_response(json!({"id": id})))
}

pub fn cash_sale_routes() -> Router {
    Router::new().route("/", post(create_cash_sale))
}
