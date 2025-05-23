use axum::{
    routing::post,
    extract::{State, Json},
    response::IntoResponse,
    Router,
};
use serde::Deserialize;
use validator::Validate;
use std::sync::Arc;
use rust_decimal::Decimal;
use uuid::Uuid;
use serde_json::json;

use crate::{
    AppState,
    handlers::common::{validate_input, created_response, map_service_error},
    errors::ApiError,
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
    let id = state.services.cash_sales
        .create_cash_sale(payload.order_id, payload.amount, payload.payment_method)
        .await
        .map_err(map_service_error)?;
    Ok(created_response(json!({"id": id})))
}

pub fn cash_sale_routes() -> Router {
    Router::new().route("/", post(create_cash_sale))
}
