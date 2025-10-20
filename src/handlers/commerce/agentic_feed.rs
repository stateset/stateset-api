use axum::{
    extract::{Json, Query, State},
    http::{header, HeaderValue},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::str::FromStr;
use tracing::instrument;

use crate::{
    errors::ApiError,
    handlers::common::success_response,
    services::commerce::product_feed_service::{
        FeedGenerationMetadata, FeedPayload, GeneratedFeed, ProductFeedFormat, ProductFeedOptions,
        ProductFeedRequest,
    },
    AppState,
};

/// Routes that expose Agentic Commerce product feeds built from the Stateset catalog.
pub fn agentic_feed_routes() -> Router<AppState> {
    Router::new()
        .route("/catalog", get(get_catalog_feed))
        .route("/catalog", post(post_catalog_feed))
}

#[derive(Debug, Deserialize)]
pub struct CatalogFeedQuery {
    pub format: Option<String>,
    pub max_products: Option<usize>,
    pub seller_name: Option<String>,
    pub seller_url: Option<String>,
    pub product_base_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CatalogFeedRequestBody {
    pub format: Option<String>,
    pub max_products: Option<usize>,
    pub enable_search: Option<bool>,
    pub enable_checkout: Option<bool>,
    pub currency: Option<String>,
    pub default_shipping_cost: Option<String>,
    pub seller_name: Option<String>,
    pub seller_url: Option<String>,
    pub product_base_url: Option<String>,
    pub seller_privacy_policy: Option<String>,
    pub seller_tos: Option<String>,
    pub return_policy: Option<String>,
    pub return_window: Option<u32>,
    pub push_to_openai: Option<bool>,
    pub openai_endpoint: Option<String>,
    pub openai_api_key: Option<String>,
}

#[instrument(skip(state))]
async fn get_catalog_feed(
    State(state): State<AppState>,
    Query(query): Query<CatalogFeedQuery>,
) -> Result<Response, ApiError> {
    let mut request = build_request_from_query(&query)?;

    let feed = state
        .services
        .catalog_product_feed
        .create_agentic_commerce_feed(request)
        .await
        .map_err(|err| ApiError::ServiceError(err.into()))?;

    Ok(into_feed_response(feed))
}

#[instrument(skip(state, body))]
async fn post_catalog_feed(
    State(state): State<AppState>,
    Json(body): Json<CatalogFeedRequestBody>,
) -> Result<Response, ApiError> {
    let mut request = ProductFeedRequest::default();

    if let Some(format) = body.format.as_deref() {
        request.format = parse_format(format)?;
    }
    request.max_products = body.max_products;
    request.push_to_openai = body.push_to_openai.unwrap_or(false);
    request.openai_endpoint = body.openai_endpoint.clone();
    request.openai_api_key = body.openai_api_key.clone();

    request.options = ProductFeedOptions {
        enable_search: body.enable_search,
        enable_checkout: body.enable_checkout,
        currency: body.currency.clone(),
        default_shipping_cost: body.default_shipping_cost.clone(),
        seller_name: body.seller_name.clone(),
        seller_url: body.seller_url.clone(),
        product_base_url: body.product_base_url.clone(),
        seller_privacy_policy: body.seller_privacy_policy.clone(),
        seller_tos: body.seller_tos.clone(),
        return_policy: body.return_policy.clone(),
        return_window: body.return_window,
    };

    let feed = state
        .services
        .catalog_product_feed
        .create_agentic_commerce_feed(request)
        .await
        .map_err(|err| ApiError::ServiceError(err.into()))?;

    Ok(into_feed_response(feed))
}

fn build_request_from_query(query: &CatalogFeedQuery) -> Result<ProductFeedRequest, ApiError> {
    let mut request = ProductFeedRequest::default();
    if let Some(format) = query.format.as_deref() {
        request.format = parse_format(format)?;
    }
    request.max_products = query.max_products;

    let mut options = ProductFeedOptions::default();
    options.seller_name = query.seller_name.clone();
    options.seller_url = query.seller_url.clone();
    options.product_base_url = query.product_base_url.clone();

    request.options = options;
    Ok(request)
}

fn parse_format(input: &str) -> Result<ProductFeedFormat, ApiError> {
    ProductFeedFormat::from_str(input).map_err(|_| ApiError::BadRequest {
        message: format!(
            "Unsupported format '{}'. Expected one of: json, csv, tsv",
            input
        ),
        error_code: Some("invalid_format".to_string()),
    })
}

fn metadata_to_json(metadata: &FeedGenerationMetadata) -> Value {
    json!({
        "format": metadata.format.to_string(),
        "product_count": metadata.product_count,
        "generated_at": metadata.generated_at.to_rfc3339(),
        "merchant": metadata.merchant,
        "enable_checkout": metadata.enable_checkout,
        "pushed_to_openai": metadata.pushed_to_openai,
    })
}

fn into_feed_response(feed: GeneratedFeed) -> Response {
    match feed.payload {
        FeedPayload::Json(envelope) => success_response(json!({
            "feed": envelope,
            "metadata": metadata_to_json(&feed.metadata),
            "openai_response": feed.openai_response,
        }))
        .into_response(),
        FeedPayload::Text { body, content_type } => {
            let mut response = Response::new(body.into());
            response
                .headers_mut()
                .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));

            if let Ok(value) = HeaderValue::from_str(&feed.metadata.product_count.to_string()) {
                response.headers_mut().insert("X-Feed-Product-Count", value);
            }

            if let Ok(value) = HeaderValue::from_str(&feed.metadata.generated_at.to_rfc3339()) {
                response.headers_mut().insert("X-Feed-Generated-At", value);
            }

            let pushed = if feed.metadata.pushed_to_openai {
                "true"
            } else {
                "false"
            };
            if let Ok(value) = HeaderValue::from_str(pushed) {
                response
                    .headers_mut()
                    .insert("X-Feed-Pushed-To-OpenAI", value);
            }

            response
        }
    }
}
