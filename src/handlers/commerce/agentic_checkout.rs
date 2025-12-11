use crate::{
    errors::{ACPErrorResponse, ApiError, ServiceError},
    services::commerce::agentic_checkout::{
        Address, CheckoutSession, CheckoutSessionCompleteRequest, CheckoutSessionCreateRequest,
        CheckoutSessionUpdateRequest, CheckoutSessionWithOrder, Item, Message, PaymentData,
    },
    AppState,
};
use axum::extract::Request;
use axum::{
    body::{self, Body},
    extract::{Json, Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use chrono::Utc;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tracing::error;
use utoipa::{IntoParams, ToSchema};

const MAX_SIGNED_BODY_SIZE: usize = 1024 * 1024; // 1 MB
const DEFAULT_SIGNATURE_TOLERANCE_SECS: i64 = 300;
const SIGNATURE_HEADER: &str = "signature";
const TIMESTAMP_HEADER: &str = "timestamp";

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApiMoney {
    amount: i64,
    currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApiLineItem {
    id: String,
    title: String,
    quantity: i32,
    unit_price: ApiMoney,
    #[serde(skip_serializing_if = "Option::is_none")]
    variant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sku: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApiTotals {
    subtotal: ApiMoney,
    #[serde(skip_serializing_if = "Option::is_none")]
    tax: Option<ApiMoney>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping: Option<ApiMoney>,
    #[serde(skip_serializing_if = "Option::is_none")]
    discount: Option<ApiMoney>,
    grand_total: ApiMoney,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApiDeliveryWindow {
    #[serde(skip_serializing_if = "Option::is_none")]
    earliest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApiFulfillmentChoice {
    id: String,
    label: String,
    price: ApiMoney,
    #[serde(skip_serializing_if = "Option::is_none")]
    est_delivery: Option<ApiDeliveryWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApiFulfillment {
    #[serde(skip_serializing_if = "Option::is_none")]
    selected_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<Vec<ApiFulfillmentChoice>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApiAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    line1: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    line2: Option<String>,
    city: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<String>,
    postal_code: String,
    country: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApiCustomer {
    #[serde(skip_serializing_if = "Option::is_none")]
    billing_address: Option<ApiAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_address: Option<ApiAddress>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApiMessage {
    #[serde(rename = "type")]
    message_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    param: Option<String>,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApiLinks {
    #[serde(skip_serializing_if = "Option::is_none")]
    terms: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    privacy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    order_permalink: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApiCheckoutSession {
    id: String,
    status: String,
    items: Vec<ApiLineItem>,
    totals: ApiTotals,
    #[serde(skip_serializing_if = "Option::is_none")]
    fulfillment: Option<ApiFulfillment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    customer: Option<ApiCustomer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    links: Option<ApiLinks>,
    #[serde(skip_serializing_if = "Option::is_none")]
    messages: Option<Vec<ApiMessage>>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApiOrder {
    id: String,
    checkout_session_id: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    permalink_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refunds: Option<Vec<ApiRefund>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApiRefund {
    #[serde(rename = "type")]
    refund_type: String,
    amount: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApiCheckoutSessionWithOrder {
    #[serde(flatten)]
    session: ApiCheckoutSession,
    order: ApiOrder,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Header)]
pub(crate) struct AgenticCheckoutHeaders {
    /// Bearer token used to authorize the caller.
    #[param(value_type = String, rename = "Authorization", required = false)]
    authorization: Option<String>,
    /// Version of the Agentic Commerce API (YYYY-MM-DD).
    #[param(value_type = String, rename = "API-Version")]
    api_version: String,
    /// Hex-encoded HMAC signature of the request body.
    #[param(value_type = String, rename = "Signature")]
    signature: String,
    /// Unix timestamp (seconds) used when computing the signature.
    #[param(value_type = String, rename = "Timestamp")]
    timestamp: String,
    /// Optional idempotency key for safely retrying writes.
    #[param(value_type = String, rename = "Idempotency-Key", required = false)]
    idempotency_key: Option<String>,
    /// Optional client-supplied correlation id.
    #[param(value_type = String, rename = "Request-Id", required = false)]
    request_id: Option<String>,
    /// Preferred locale for localized messaging.
    #[param(value_type = String, rename = "Accept-Language", required = false)]
    accept_language: Option<String>,
    /// Calling client identifier.
    #[param(value_type = String, rename = "User-Agent", required = false)]
    user_agent: Option<String>,
    /// Request payload content type (default application/json).
    #[param(value_type = String, rename = "Content-Type", required = false)]
    content_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApiItemInput {
    id: String,
    quantity: i32,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApiFulfillmentSelection {
    #[serde(default)]
    selected_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApiCreateCheckoutSessionRequest {
    items: Vec<ApiItemInput>,
    #[serde(default)]
    customer: Option<ApiCustomer>,
    #[serde(default)]
    fulfillment: Option<ApiFulfillmentSelection>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApiUpdateCheckoutSessionRequest {
    #[serde(default)]
    customer: Option<ApiCustomer>,
    #[serde(default)]
    items: Option<Vec<ApiItemInput>>,
    #[serde(default)]
    fulfillment: Option<ApiFulfillmentSelection>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApiPaymentRequest {
    #[serde(default)]
    delegated_token: Option<String>,
    #[serde(default)]
    method: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(crate) struct ApiCompleteCheckoutSessionRequest {
    payment: ApiPaymentRequest,
    #[serde(default)]
    customer: Option<ApiCustomer>,
    #[serde(default)]
    fulfillment: Option<ApiFulfillmentSelection>,
}

#[derive(Default)]
struct ConvertedCustomer {
    buyer: Option<crate::services::commerce::agentic_checkout::Buyer>,
    shipping: Option<Address>,
    billing: Option<Address>,
}

fn to_api_money(money: &crate::services::commerce::agentic_checkout::Money) -> ApiMoney {
    ApiMoney {
        amount: money.amount,
        currency: money.currency.to_uppercase(),
    }
}

fn parse_amount_str(value: &str) -> i64 {
    value.trim().parse::<i64>().unwrap_or(0)
}

fn to_api_line_item(item: &crate::services::commerce::agentic_checkout::LineItem) -> ApiLineItem {
    ApiLineItem {
        id: item.id.clone(),
        title: item.title.clone(),
        quantity: item.quantity,
        unit_price: to_api_money(&item.unit_price),
        variant_id: item.variant_id.clone(),
        sku: item.sku.clone(),
        image_url: item.image_url.clone(),
    }
}

fn totals_from_session(session: &CheckoutSession) -> ApiTotals {
    let mut subtotal: Option<ApiMoney> = None;
    let mut tax: Option<ApiMoney> = None;
    let mut shipping: Option<ApiMoney> = None;
    let mut discount: Option<ApiMoney> = None;
    let mut grand_total = ApiMoney {
        amount: 0,
        currency: session.currency.to_uppercase(),
    };

    for total in &session.totals {
        let money = ApiMoney {
            amount: total.amount,
            currency: session.currency.to_uppercase(),
        };
        match total.total_type.as_str() {
            "subtotal" => subtotal = Some(money),
            "tax" => tax = Some(money),
            "shipping" => shipping = Some(money),
            "discount" => discount = Some(money),
            "total" => grand_total = money,
            _ => {}
        }
    }

    ApiTotals {
        subtotal: subtotal.unwrap_or(ApiMoney {
            amount: 0,
            currency: session.currency.to_uppercase(),
        }),
        tax,
        shipping,
        discount,
        grand_total,
    }
}

fn fulfillment_from_session(session: &CheckoutSession) -> Option<ApiFulfillment> {
    if session.fulfillment_options.is_empty()
        && session.fulfillment_option_id.is_none()
        && session.fulfillment_address.is_none()
    {
        return None;
    }

    let options = session
        .fulfillment_options
        .iter()
        .map(|opt| match opt {
            crate::services::commerce::agentic_checkout::FulfillmentOption::Shipping(option) => {
                ApiFulfillmentChoice {
                    id: option.id.clone(),
                    label: option.title.clone(),
                    price: ApiMoney {
                        amount: parse_amount_str(&option.total),
                        currency: session.currency.to_uppercase(),
                    },
                    est_delivery: Some(ApiDeliveryWindow {
                        earliest: option.earliest_delivery_time.clone(),
                        latest: option.latest_delivery_time.clone(),
                    }),
                }
            }
            crate::services::commerce::agentic_checkout::FulfillmentOption::Digital(option) => {
                ApiFulfillmentChoice {
                    id: option.id.clone(),
                    label: option.title.clone(),
                    price: ApiMoney {
                        amount: parse_amount_str(&option.total),
                        currency: session.currency.to_uppercase(),
                    },
                    est_delivery: None,
                }
            }
        })
        .collect::<Vec<_>>();

    Some(ApiFulfillment {
        selected_id: session.fulfillment_option_id.clone(),
        options: if options.is_empty() {
            None
        } else {
            Some(options)
        },
    })
}

fn customer_from_session(session: &CheckoutSession) -> Option<ApiCustomer> {
    if session.buyer.is_none() && session.fulfillment_address.is_none() {
        return None;
    }

    let shipping_address = session.fulfillment_address.as_ref().map(|addr| ApiAddress {
        name: Some(addr.name.clone()),
        line1: addr.line_one.clone(),
        line2: addr.line_two.clone(),
        city: addr.city.clone(),
        region: Some(addr.state.clone()),
        postal_code: addr.postal_code.clone(),
        country: addr.country.clone(),
        phone: addr.phone.clone(),
        email: addr.email.clone(),
    });

    Some(ApiCustomer {
        billing_address: None,
        shipping_address,
    })
}

fn links_from_session(session: &CheckoutSession) -> Option<ApiLinks> {
    if session.links.is_empty() {
        return None;
    }

    let mut links = ApiLinks {
        terms: None,
        privacy: None,
        order_permalink: None,
    };

    for link in &session.links {
        match link.link_type.as_str() {
            "terms_of_use" => links.terms = Some(link.url.clone()),
            "privacy_policy" => links.privacy = Some(link.url.clone()),
            "order_permalink" => links.order_permalink = Some(link.url.clone()),
            _ => {}
        }
    }

    if links.terms.is_none() && links.privacy.is_none() && links.order_permalink.is_none() {
        None
    } else {
        Some(links)
    }
}

fn messages_from_session(session: &CheckoutSession) -> Option<Vec<ApiMessage>> {
    if session.messages.is_empty() {
        return None;
    }

    let messages = session
        .messages
        .iter()
        .map(|msg| match msg {
            Message::Info(info) => ApiMessage {
                message_type: "info".to_string(),
                code: None,
                param: info.param.clone(),
                message: info.content.clone(),
            },
            Message::Error(err) => ApiMessage {
                message_type: "error".to_string(),
                code: Some(err.code.clone()),
                param: err.param.clone(),
                message: err.content.clone(),
            },
        })
        .collect::<Vec<_>>();

    Some(messages)
}

fn to_api_session(session: &CheckoutSession) -> ApiCheckoutSession {
    let totals = totals_from_session(session);

    ApiCheckoutSession {
        id: session.id.clone(),
        status: session.status.clone(),
        items: session.line_items.iter().map(to_api_line_item).collect(),
        totals,
        fulfillment: fulfillment_from_session(session),
        customer: customer_from_session(session),
        links: links_from_session(session),
        messages: messages_from_session(session),
        created_at: session.created_at.to_rfc3339(),
        updated_at: session
            .updated_at
            .unwrap_or(session.created_at)
            .to_rfc3339(),
    }
}

fn to_api_session_with_order(result: &CheckoutSessionWithOrder) -> ApiCheckoutSessionWithOrder {
    let mut session_view = to_api_session(&result.session);
    if session_view.links.is_none() && !result.order.permalink_url.is_empty() {
        session_view.links = Some(ApiLinks {
            terms: None,
            privacy: None,
            order_permalink: Some(result.order.permalink_url.clone()),
        });
    }

    ApiCheckoutSessionWithOrder {
        session: session_view,
        order: ApiOrder {
            id: result.order.id.clone(),
            checkout_session_id: result.order.checkout_session_id.clone(),
            status: result
                .order
                .status
                .clone()
                .unwrap_or_else(|| "placed".to_string()),
            permalink_url: if result.order.permalink_url.is_empty() {
                None
            } else {
                Some(result.order.permalink_url.clone())
            },
            refunds: None,
        },
    }
}

fn acp_invalid_request(message: impl Into<String>, param: Option<&str>, code: &str) -> ApiError {
    let response =
        ACPErrorResponse::invalid_request(code, message.into(), param.map(|p| p.to_string()));
    ApiError::acp(StatusCode::BAD_REQUEST, response)
}

fn acp_authentication_error(message: impl Into<String>) -> ApiError {
    let response = ACPErrorResponse::authentication_error(message.into());
    ApiError::acp(StatusCode::UNAUTHORIZED, response)
}

fn api_address_to_service(address: &ApiAddress) -> Result<Address, ApiError> {
    let name = address
        .name
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Customer".to_string());

    let region = address
        .region
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            acp_invalid_request(
                "region is required for shipping address",
                Some("customer.shipping_address.region"),
                "validation_error",
            )
        })?;

    if address.line1.trim().is_empty() {
        return Err(acp_invalid_request(
            "line1 is required for shipping address",
            Some("customer.shipping_address.line1"),
            "validation_error",
        ));
    }
    if address.city.trim().is_empty() {
        return Err(acp_invalid_request(
            "city is required for shipping address",
            Some("customer.shipping_address.city"),
            "validation_error",
        ));
    }
    if address.postal_code.trim().is_empty() {
        return Err(acp_invalid_request(
            "postal_code is required for shipping address",
            Some("customer.shipping_address.postal_code"),
            "validation_error",
        ));
    }
    if address.country.trim().len() != 2 {
        return Err(acp_invalid_request(
            "country must be a two-letter ISO code",
            Some("customer.shipping_address.country"),
            "validation_error",
        ));
    }

    Ok(Address {
        name,
        line_one: address.line1.trim().to_string(),
        line_two: address.line2.as_ref().map(|v| v.trim().to_string()),
        city: address.city.trim().to_string(),
        state: region,
        country: address.country.trim().to_uppercase(),
        postal_code: address.postal_code.trim().to_string(),
        phone: address.phone.as_ref().map(|v| v.trim().to_string()),
        email: address.email.as_ref().map(|v| v.trim().to_string()),
    })
}

fn derive_buyer(
    customer: &ApiCustomer,
    require: bool,
) -> Result<Option<crate::services::commerce::agentic_checkout::Buyer>, ApiError> {
    let candidate = customer
        .shipping_address
        .as_ref()
        .or(customer.billing_address.as_ref());

    let Some(address) = candidate else {
        if require {
            return Err(acp_invalid_request(
                "customer shipping address required",
                Some("customer.shipping_address"),
                "validation_error",
            ));
        }
        return Ok(None);
    };

    let email = address
        .email
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            customer.billing_address.as_ref().and_then(|addr| {
                addr.email
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
            })
        });

    let Some(email) = email else {
        if require {
            return Err(acp_invalid_request(
                "customer email required",
                Some("customer.shipping_address.email"),
                "validation_error",
            ));
        }
        return Ok(None);
    };

    let name = address
        .name
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            acp_invalid_request(
                "customer name required",
                Some("customer.shipping_address.name"),
                "validation_error",
            )
        })?;

    let mut parts = name.split_whitespace();
    let first = parts.next().unwrap_or(name).to_string();
    let last = parts.collect::<Vec<_>>().join(" ");
    let last = if last.is_empty() {
        "Customer".to_string()
    } else {
        last
    };

    Ok(Some(crate::services::commerce::agentic_checkout::Buyer {
        first_name: first,
        last_name: last,
        email: email.to_string(),
        phone_number: address.phone.clone(),
    }))
}

fn convert_customer(
    customer: Option<ApiCustomer>,
    require_buyer: bool,
) -> Result<ConvertedCustomer, ApiError> {
    let mut converted = ConvertedCustomer::default();
    if let Some(customer) = customer {
        if let Some(shipping) = customer.shipping_address.as_ref() {
            converted.shipping = Some(api_address_to_service(shipping)?);
        }
        if let Some(billing) = customer.billing_address.as_ref() {
            converted.billing = Some(api_address_to_service(billing)?);
        }
        converted.buyer = derive_buyer(&customer, require_buyer)?;
    } else if require_buyer {
        return Err(acp_invalid_request(
            "customer information required",
            Some("customer"),
            "validation_error",
        ));
    }
    Ok(converted)
}

fn convert_items(items: Vec<ApiItemInput>) -> Vec<Item> {
    items
        .into_iter()
        .map(|item| Item {
            id: item.id,
            quantity: item.quantity,
        })
        .collect()
}

fn convert_payment_request(
    payment: &ApiPaymentRequest,
    provider: &str,
    billing_address: Option<Address>,
) -> Result<PaymentData, ApiError> {
    let token = payment
        .delegated_token
        .clone()
        .or_else(|| payment.method.clone())
        .ok_or_else(|| {
            acp_invalid_request(
                "payment.delegated_token or payment.method required",
                Some("payment"),
                "validation_error",
            )
        })?;

    Ok(PaymentData {
        token,
        provider: provider.to_string(),
        billing_address,
    })
}

fn extract_idempotency(
    headers: &HeaderMap,
) -> Result<(Option<String>, Option<HeaderValue>), ApiError> {
    match headers.get("Idempotency-Key") {
        Some(value) => {
            let value_str = value.to_str().map_err(|_| {
                acp_invalid_request(
                    "Idempotency-Key must be valid ASCII",
                    Some("headers.Idempotency-Key"),
                    "invalid_request",
                )
            })?;
            Ok((Some(value_str.to_string()), Some(value.clone())))
        }
        None => Ok((None, None)),
    }
}

fn extract_request_id(headers: &HeaderMap) -> Option<HeaderValue> {
    headers.get("Request-Id").cloned()
}

fn build_acp_response<T: Serialize>(
    payload: &T,
    status: StatusCode,
    idempotency: Option<HeaderValue>,
    request_id: Option<HeaderValue>,
    location: Option<String>,
) -> Result<Response, ApiError> {
    let body = serde_json::to_vec(payload).map_err(|e| {
        ApiError::ServiceError(ServiceError::InternalError(format!(
            "Serialization error: {}",
            e
        )))
    })?;

    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CACHE_CONTROL, "no-store");

    if let Some(idempotency_value) = idempotency {
        builder = builder.header("Idempotency-Key", idempotency_value);
    }
    if let Some(request_id_value) = request_id {
        builder = builder.header("Request-Id", request_id_value);
    }
    if let Some(location_value) = location {
        builder = builder.header(header::LOCATION, location_value);
    }

    builder.body(body.into()).map_err(|e| {
        ApiError::ServiceError(ServiceError::InternalError(format!(
            "Response build error: {}",
            e
        )))
    })
}

/// Creates the router for agentic checkout endpoints
pub fn agentic_checkout_routes() -> Router<AppState> {
    Router::new()
        .route("/checkout_sessions", post(create_checkout_session))
        .route(
            "/checkout_sessions/:checkout_session_id",
            get(get_checkout_session),
        )
        .route(
            "/checkout_sessions/:checkout_session_id",
            post(update_checkout_session),
        )
        .route(
            "/checkout_sessions/:checkout_session_id/complete",
            post(complete_checkout_session),
        )
        .route(
            "/checkout_sessions/:checkout_session_id/cancel",
            post(cancel_checkout_session),
        )
        .layer(middleware::from_fn(verify_acp_signature))
}

/// Create a checkout session
#[utoipa::path(
    post,
    path = "/checkout_sessions",
    tag = "Agentic Checkout",
    request_body = ApiCreateCheckoutSessionRequest,
    params(AgenticCheckoutHeaders),
    responses(
        (status = 201, description = "Checkout session created", body = ApiCheckoutSession,
            headers(
                ("Location" = String, description = "Canonical URL for the new checkout session"),
                ("Idempotency-Key" = String, description = "Echo of the request idempotency key"),
                ("Request-Id" = String, description = "Echo of the client correlation id")
            )
        ),
        (status = 200, description = "Checkout session already existed", body = ApiCheckoutSession,
            headers(
                ("Idempotency-Key" = String, description = "Echo of the request idempotency key"),
                ("Request-Id" = String, description = "Echo of the client correlation id")
            )
        ),
        (status = 400, description = "Invalid request payload", body = crate::errors::ACPErrorResponse),
        (status = 401, description = "Authentication or signature failure", body = crate::errors::ACPErrorResponse),
        (status = 409, description = "Unable to honor the request because of a conflict", body = crate::errors::ACPErrorResponse),
        (status = 422, description = "Request failed validation", body = crate::errors::ACPErrorResponse),
        (status = 500, description = "Unexpected error", body = crate::errors::ACPErrorResponse)
    )
)]
async fn create_checkout_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ApiCreateCheckoutSessionRequest>,
) -> Result<Response, ApiError> {
    let (idempotency_key, idempotency_header) = extract_idempotency(&headers)?;
    let request_id = extract_request_id(&headers);

    let ApiCreateCheckoutSessionRequest {
        items,
        customer,
        fulfillment: _,
    } = payload;

    let converted_customer = convert_customer(customer, false)?;
    let service_request = CheckoutSessionCreateRequest {
        buyer: converted_customer.buyer,
        items: convert_items(items),
        fulfillment_address: converted_customer.shipping,
        promotion_code: None,
    };

    let create_result = state
        .services
        .agentic_checkout
        .create_session(service_request, idempotency_key.as_deref())
        .await
        .map_err(map_service_error)?;

    let api_session = to_api_session(&create_result.session);
    let status = if create_result.was_created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    let location = if create_result.was_created {
        Some(format!("/checkout_sessions/{}", api_session.id))
    } else {
        None
    };

    build_acp_response(
        &api_session,
        status,
        idempotency_header,
        request_id,
        location,
    )
}

/// Get checkout session
#[utoipa::path(
    get,
    path = "/checkout_sessions/:checkout_session_id}",
    tag = "Agentic Checkout",
    params(
        AgenticCheckoutHeaders,
        ("checkout_session_id" = String, Path, description = "Checkout session identifier")
    ),
    responses(
        (status = 200, description = "Checkout session retrieved", body = ApiCheckoutSession,
            headers(
                ("Request-Id" = String, description = "Echo of the client correlation id")
            )
        ),
        (status = 401, description = "Authentication or signature failure", body = crate::errors::ACPErrorResponse),
        (status = 404, description = "Checkout session not found", body = crate::errors::ACPErrorResponse),
        (status = 500, description = "Unexpected error", body = crate::errors::ACPErrorResponse)
    )
)]
async fn get_checkout_session(
    State(state): State<AppState>,
    Path(checkout_session_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let session = state
        .services
        .agentic_checkout
        .get_session(&checkout_session_id)
        .await
        .map_err(|e| match e {
            crate::errors::ServiceError::NotFound(_) => ApiError::NotFound(format!(
                "Checkout session {} not found",
                checkout_session_id
            )),
            _ => map_service_error(e),
        })?;

    let api_session = to_api_session(&session);
    let request_id = extract_request_id(&headers);
    build_acp_response(&api_session, StatusCode::OK, None, request_id, None)
}

/// Update checkout session
#[utoipa::path(
    post,
    path = "/checkout_sessions/:checkout_session_id}",
    tag = "Agentic Checkout",
    request_body = ApiUpdateCheckoutSessionRequest,
    params(
        AgenticCheckoutHeaders,
        ("checkout_session_id" = String, Path, description = "Checkout session identifier")
    ),
    responses(
        (status = 200, description = "Checkout session updated", body = ApiCheckoutSession,
            headers(
                ("Idempotency-Key" = String, description = "Echo of the request idempotency key"),
                ("Request-Id" = String, description = "Echo of the client correlation id")
            )
        ),
        (status = 400, description = "Invalid request payload", body = crate::errors::ACPErrorResponse),
        (status = 401, description = "Authentication or signature failure", body = crate::errors::ACPErrorResponse),
        (status = 404, description = "Checkout session not found", body = crate::errors::ACPErrorResponse),
        (status = 409, description = "Operation conflicts with session state", body = crate::errors::ACPErrorResponse),
        (status = 422, description = "Provided data failed validation", body = crate::errors::ACPErrorResponse),
        (status = 500, description = "Unexpected error", body = crate::errors::ACPErrorResponse)
    )
)]
async fn update_checkout_session(
    State(state): State<AppState>,
    Path(checkout_session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<ApiUpdateCheckoutSessionRequest>,
) -> Result<Response, ApiError> {
    let (_, idempotency_header) = extract_idempotency(&headers)?;
    let request_id = extract_request_id(&headers);
    let ApiUpdateCheckoutSessionRequest {
        customer,
        items,
        fulfillment,
    } = payload;
    let converted_customer = convert_customer(customer, false)?;

    let items = items.map(convert_items);
    let fulfillment_option_id = fulfillment.and_then(|selection| selection.selected_id);

    let service_request = CheckoutSessionUpdateRequest {
        buyer: converted_customer.buyer,
        items,
        fulfillment_address: converted_customer.shipping,
        fulfillment_option_id,
        promotion_code: None,
    };

    let session = state
        .services
        .agentic_checkout
        .update_session(&checkout_session_id, service_request)
        .await
        .map_err(|e| match e {
            crate::errors::ServiceError::NotFound(_) => ApiError::NotFound(format!(
                "Checkout session {} not found",
                checkout_session_id
            )),
            crate::errors::ServiceError::InvalidOperation(msg) => ApiError::BadRequest {
                message: msg,
                error_code: Some("INVALID_REQUEST".to_string()),
            },
            _ => map_service_error(e),
        })?;

    let api_session = to_api_session(&session);
    build_acp_response(
        &api_session,
        StatusCode::OK,
        idempotency_header,
        request_id,
        None,
    )
}

/// Complete checkout session
#[utoipa::path(
    post,
    path = "/checkout_sessions/:checkout_session_id}/complete",
    tag = "Agentic Checkout",
    request_body = ApiCompleteCheckoutSessionRequest,
    params(
        AgenticCheckoutHeaders,
        ("checkout_session_id" = String, Path, description = "Checkout session identifier")
    ),
    responses(
        (status = 200, description = "Checkout session completed and order created", body = ApiCheckoutSessionWithOrder,
            headers(
                ("Idempotency-Key" = String, description = "Echo of the request idempotency key"),
                ("Request-Id" = String, description = "Echo of the client correlation id")
            )
        ),
        (status = 400, description = "Invalid request payload", body = crate::errors::ACPErrorResponse),
        (status = 401, description = "Authentication or signature failure", body = crate::errors::ACPErrorResponse),
        (status = 402, description = "Payment failed", body = crate::errors::ACPErrorResponse),
        (status = 404, description = "Checkout session not found", body = crate::errors::ACPErrorResponse),
        (status = 409, description = "Completion not allowed in current state", body = crate::errors::ACPErrorResponse),
        (status = 422, description = "Provided data failed validation", body = crate::errors::ACPErrorResponse),
        (status = 500, description = "Unexpected error", body = crate::errors::ACPErrorResponse)
    )
)]
async fn complete_checkout_session(
    State(state): State<AppState>,
    Path(checkout_session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<ApiCompleteCheckoutSessionRequest>,
) -> Result<Response, ApiError> {
    let (_, idempotency_header) = extract_idempotency(&headers)?;
    let request_id = extract_request_id(&headers);

    let ApiCompleteCheckoutSessionRequest {
        payment,
        customer,
        fulfillment: _,
    } = payload;

    let converted_customer = convert_customer(customer, true)?;
    let provider = state
        .config
        .payment_provider
        .clone()
        .unwrap_or_else(|| "stripe".to_string());
    let billing_address = converted_customer
        .billing
        .clone()
        .or(converted_customer.shipping.clone());

    let payment_data = convert_payment_request(&payment, &provider, billing_address)?;
    let service_request = CheckoutSessionCompleteRequest {
        buyer: converted_customer.buyer,
        payment_data,
    };

    let result = state
        .services
        .agentic_checkout
        .complete_session(&checkout_session_id, service_request)
        .await
        .map_err(|e| match e {
            crate::errors::ServiceError::NotFound(_) => ApiError::NotFound(format!(
                "Checkout session {} not found",
                checkout_session_id
            )),
            crate::errors::ServiceError::InvalidOperation(msg) => ApiError::BadRequest {
                message: msg,
                error_code: Some("INVALID_REQUEST".to_string()),
            },
            _ => map_service_error(e),
        })?;

    let api = to_api_session_with_order(&result);
    build_acp_response(&api, StatusCode::OK, idempotency_header, request_id, None)
}

/// Cancel checkout session
#[utoipa::path(
    post,
    path = "/checkout_sessions/:checkout_session_id}/cancel",
    tag = "Agentic Checkout",
    params(
        AgenticCheckoutHeaders,
        ("checkout_session_id" = String, Path, description = "Checkout session identifier")
    ),
    responses(
        (status = 200, description = "Checkout session cancelled", body = ApiCheckoutSession,
            headers(
                ("Idempotency-Key" = String, description = "Echo of the request idempotency key"),
                ("Request-Id" = String, description = "Echo of the client correlation id")
            )
        ),
        (status = 401, description = "Authentication or signature failure", body = crate::errors::ACPErrorResponse),
        (status = 404, description = "Checkout session not found", body = crate::errors::ACPErrorResponse),
        (status = 405, description = "Cancellation not permitted because session is terminal", body = crate::errors::ACPErrorResponse),
        (status = 409, description = "Operation conflicts with session state", body = crate::errors::ACPErrorResponse),
        (status = 500, description = "Unexpected error", body = crate::errors::ACPErrorResponse)
    )
)]
async fn cancel_checkout_session(
    State(state): State<AppState>,
    Path(checkout_session_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let (_, idempotency_header) = extract_idempotency(&headers)?;
    let request_id = extract_request_id(&headers);

    let session = state
        .services
        .agentic_checkout
        .cancel_session(&checkout_session_id)
        .await
        .map_err(|e| match e {
            crate::errors::ServiceError::NotFound(_) => ApiError::NotFound(format!(
                "Checkout session {} not found",
                checkout_session_id
            )),
            crate::errors::ServiceError::InvalidOperation(msg) => {
                // If already completed/canceled, return 405
                return ApiError::MethodNotAllowed { message: msg };
            }
            _ => map_service_error(e),
        })?;

    let api_session = to_api_session(&session);
    build_acp_response(
        &api_session,
        StatusCode::OK,
        idempotency_header,
        request_id,
        None,
    )
}

async fn verify_acp_signature(req: Request, next: Next) -> Response {
    let state_opt = req.extensions().get::<AppState>().cloned();
    let Some(state) = state_opt else {
        return ApiError::InternalServerError.into_response();
    };

    let Some(secret) = state
        .config
        .agentic_commerce_signing_secret
        .as_ref()
        .map(|s| s.clone())
    else {
        return next.run(req).await;
    };

    // Clone headers before consuming the body
    let signature_header = req
        .headers()
        .get(SIGNATURE_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string());
    let timestamp_header = req
        .headers()
        .get(TIMESTAMP_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string());

    let signature = if let Some(sig) = signature_header {
        sig
    } else {
        return acp_authentication_error("missing signature header").into_response();
    };

    let timestamp = if let Some(ts) = timestamp_header {
        ts
    } else {
        return acp_authentication_error("missing timestamp header").into_response();
    };

    let tolerance = state
        .config
        .agentic_commerce_signature_tolerance_secs
        .map(|v| v as i64)
        .unwrap_or(DEFAULT_SIGNATURE_TOLERANCE_SECS);

    let parsed_ts = match timestamp.parse::<i64>() {
        Ok(value) => value,
        Err(_) => {
            return acp_authentication_error("timestamp must be integer seconds since epoch")
                .into_response();
        }
    };

    let now = Utc::now().timestamp();
    if (now - parsed_ts).abs() > tolerance {
        return acp_authentication_error("request timestamp outside allowable tolerance")
            .into_response();
    }

    let (parts, body) = req.into_parts();
    let bytes = match body::to_bytes(body, MAX_SIGNED_BODY_SIZE).await {
        Ok(buf) => buf,
        Err(err) => {
            error!(
                "failed to buffer request body for signature verification: {}",
                err
            );
            return ApiError::InternalServerError.into_response();
        }
    };

    let expected = compute_signature_hex(&secret, &timestamp, &bytes);
    let provided = match hex::decode(signature) {
        Ok(decoded) => decoded,
        Err(_) => {
            return acp_authentication_error("signature must be valid hex").into_response();
        }
    };

    if !constant_time_eq(&provided, &expected) {
        return acp_authentication_error("signature verification failed").into_response();
    }

    let rebuilt = Request::from_parts(parts, Body::from(bytes));

    next.run(rebuilt).await
}

fn compute_signature_hex(secret: &str, timestamp: &str, body: &[u8]) -> Vec<u8> {
    type HmacSha256 = Hmac<Sha256>;
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");

    mac.update(timestamp.as_bytes());
    mac.update(b".");
    mac.update(body);

    mac.finalize().into_bytes().to_vec()
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }

    diff == 0
}

fn map_service_error(error: crate::errors::ServiceError) -> ApiError {
    error!("Service error: {:?}", error);
    let status = service_error_status(&error);
    let response = ACPErrorResponse::from(&error);
    ApiError::acp(status, response)
}

fn service_error_status(error: &ServiceError) -> StatusCode {
    match error {
        ServiceError::NotFound(_) => StatusCode::NOT_FOUND,
        ServiceError::ValidationError(_)
        | ServiceError::InvalidStatus(_)
        | ServiceError::InvalidOperation(_)
        | ServiceError::BadRequest(_)
        | ServiceError::InvalidInput(_)
        | ServiceError::OrderError(_)
        | ServiceError::InventoryError(_) => StatusCode::BAD_REQUEST,
        ServiceError::AuthError(_)
        | ServiceError::JwtError(_)
        | ServiceError::Unauthorized(_)
        | ServiceError::Forbidden(_) => StatusCode::UNAUTHORIZED,
        ServiceError::PaymentFailed(_) => StatusCode::PAYMENT_REQUIRED,
        ServiceError::Conflict(_) | ServiceError::ConcurrentModification(_) => StatusCode::CONFLICT,
        ServiceError::InsufficientStock(_) => StatusCode::UNPROCESSABLE_ENTITY,
        ServiceError::ExternalServiceError(_) | ServiceError::ExternalApiError(_) => {
            StatusCode::BAD_GATEWAY
        }
        ServiceError::RateLimitExceeded => StatusCode::TOO_MANY_REQUESTS,
        ServiceError::CircuitBreakerOpen | ServiceError::ServiceUnavailable(_) => {
            StatusCode::SERVICE_UNAVAILABLE
        }
        ServiceError::DatabaseError(_)
        | ServiceError::InternalError(_)
        | ServiceError::EventError(_)
        | ServiceError::HashError(_)
        | ServiceError::CacheError(_)
        | ServiceError::QueueError(_)
        | ServiceError::SerializationError(_)
        | ServiceError::MigrationError(_)
        | ServiceError::InternalServerError
        | ServiceError::Other(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
