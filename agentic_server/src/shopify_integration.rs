use crate::errors::ServiceError;
use crate::models::{
    Address, CheckoutSession, CheckoutSessionCompleteRequest, CheckoutSessionCreateRequest,
    CheckoutSessionStatus, CheckoutSessionUpdateRequest, CheckoutSessionWithOrder, Customer,
    EstimatedDelivery, FulfillmentChoice, FulfillmentState, LineItem, Links, Message, MessageType,
    Money, Order, OrderStatus, RequestItem, Totals,
};
use chrono::Utc;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    StatusCode,
};
use serde::{Deserialize, Serialize};
use tracing::warn;

#[derive(Debug, Clone)]
pub struct ShopifyConfig {
    pub domain: String,
    pub access_token: String,
    pub api_version: String,
}

impl ShopifyConfig {
    /// Attempt to construct Shopify configuration from environment variables.
    /// Returns `Ok(None)` when Shopify integration is not configured.
    pub fn from_env() -> Result<Option<Self>, String> {
        let domain = std::env::var("SHOPIFY_DOMAIN").ok();
        let access_token = std::env::var("SHOPIFY_ACCESS_TOKEN").ok();

        match (domain, access_token) {
            (None, None) => Ok(None),
            (Some(domain), Some(access_token)) => {
                let api_version =
                    std::env::var("SHOPIFY_API_VERSION").unwrap_or_else(|_| "2024-01".to_string());
                Ok(Some(Self {
                    domain,
                    access_token,
                    api_version,
                }))
            }
            (None, Some(_)) => {
                Err("SHOPIFY_DOMAIN must be provided when SHOPIFY_ACCESS_TOKEN is set.".to_string())
            }
            (Some(_), None) => {
                Err("SHOPIFY_ACCESS_TOKEN must be provided when SHOPIFY_DOMAIN is set.".to_string())
            }
        }
    }
}

#[derive(Clone)]
pub struct ShopifyClient {
    client: reqwest::Client,
    config: ShopifyConfig,
}

impl ShopifyClient {
    pub fn new(config: ShopifyConfig) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .user_agent("agentic-commerce-shopify-integration/0.1")
            .build()?;

        Ok(Self { client, config })
    }

    fn endpoint(&self, path: &str) -> String {
        format!(
            "https://{}/admin/api/{}/{}",
            self.config.domain, self.config.api_version, path
        )
    }

    fn auth_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        match HeaderValue::from_str(&self.config.access_token) {
            Ok(value) => {
                headers.insert("X-Shopify-Access-Token", value);
            }
            Err(err) => {
                warn!("Invalid Shopify access token header value: {}", err);
            }
        }
        headers
    }

    pub async fn create_session(
        &self,
        request: &CheckoutSessionCreateRequest,
    ) -> Result<CheckoutSession, ServiceError> {
        let payload = CreateCheckoutRequest {
            checkout: CheckoutPayload {
                line_items: build_line_items(&request.items)?,
                email: extract_email(request.customer.as_ref()),
                shipping_address: request
                    .customer
                    .as_ref()
                    .and_then(|c| c.shipping_address.as_ref())
                    .map(shopify_address_from),
                billing_address: request
                    .customer
                    .as_ref()
                    .and_then(|c| c.billing_address.as_ref())
                    .map(shopify_address_from),
            },
        };

        let url = self.endpoint("checkouts.json");
        let response = self
            .client
            .post(url)
            .headers(self.auth_headers())
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                ServiceError::ExternalServiceError(format!("Shopify checkout create failed: {}", e))
            })?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            return Err(ServiceError::ExternalServiceError(format!(
                "Shopify checkout create failed ({}): {}",
                status, body_text
            )));
        }

        let body = response
            .json::<ShopifyCheckoutEnvelope>()
            .await
            .map_err(|e| {
                ServiceError::ExternalServiceError(format!(
                    "Failed to parse Shopify create response: {}",
                    e
                ))
            })?;

        Ok(map_checkout(body.checkout))
    }

    pub async fn get_session(&self, session_id: &str) -> Result<CheckoutSession, ServiceError> {
        let url = self.endpoint(&format!("checkouts/{}.json", session_id));
        let response = self
            .client
            .get(url)
            .headers(self.auth_headers())
            .send()
            .await
            .map_err(|e| {
                ServiceError::ExternalServiceError(format!("Shopify checkout fetch failed: {}", e))
            })?;

        let status = response.status();

        if status == StatusCode::NOT_FOUND {
            return Err(ServiceError::NotFound(format!(
                "Checkout session {} not found in Shopify",
                session_id
            )));
        }

        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            return Err(ServiceError::ExternalServiceError(format!(
                "Shopify checkout fetch failed ({}): {}",
                status, body_text
            )));
        }

        let body = response
            .json::<ShopifyCheckoutEnvelope>()
            .await
            .map_err(|e| {
                ServiceError::ExternalServiceError(format!(
                    "Failed to parse Shopify get response: {}",
                    e
                ))
            })?;

        Ok(map_checkout(body.checkout))
    }

    pub async fn update_session(
        &self,
        session_id: &str,
        request: &CheckoutSessionUpdateRequest,
    ) -> Result<CheckoutSession, ServiceError> {
        let payload = UpdateCheckoutRequest {
            checkout: UpdateCheckoutPayload {
                line_items: if let Some(items) = request.items.as_ref() {
                    Some(build_line_items(items)?)
                } else {
                    None
                },
                email: extract_email(request.customer.as_ref()),
                shipping_address: request
                    .customer
                    .as_ref()
                    .and_then(|c| c.shipping_address.as_ref())
                    .map(shopify_address_from),
                billing_address: request
                    .customer
                    .as_ref()
                    .and_then(|c| c.billing_address.as_ref())
                    .map(shopify_address_from),
            },
        };

        let url = self.endpoint(&format!("checkouts/{}.json", session_id));
        let response = self
            .client
            .put(url)
            .headers(self.auth_headers())
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                ServiceError::ExternalServiceError(format!("Shopify checkout update failed: {}", e))
            })?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            return Err(ServiceError::ExternalServiceError(format!(
                "Shopify checkout update failed ({}): {}",
                status, body_text
            )));
        }

        self.get_session(session_id).await
    }

    pub async fn complete_session(
        &self,
        session_id: &str,
        request: &CheckoutSessionCompleteRequest,
    ) -> Result<CheckoutSessionWithOrder, ServiceError> {
        let delegated_token = request.payment.delegated_token.as_ref().ok_or_else(|| {
            ServiceError::InvalidOperation(
                "Shopify completion requires delegated_token payment".to_string(),
            )
        })?;

        let payload = CompleteCheckoutRequest {
            payment: ShopifyPaymentPayload {
                credit_card_token: delegated_token.clone(),
            },
        };

        let url = self.endpoint(&format!("checkouts/{}/complete.json", session_id));
        let response = self
            .client
            .post(url)
            .headers(self.auth_headers())
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                ServiceError::ExternalServiceError(format!(
                    "Shopify checkout completion failed: {}",
                    e
                ))
            })?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            return Err(ServiceError::ExternalServiceError(format!(
                "Shopify checkout completion failed ({}): {}",
                status, body_text
            )));
        }

        let body = response.json::<ShopifyOrderEnvelope>().await.map_err(|e| {
            ServiceError::ExternalServiceError(format!(
                "Failed to parse Shopify completion response: {}",
                e
            ))
        })?;

        let session = self.get_session(session_id).await.unwrap_or_else(|err| {
            warn!(
                "Failed to fetch Shopify checkout {} after completion: {}",
                session_id, err
            );
            CheckoutSession {
                id: session_id.to_string(),
                status: CheckoutSessionStatus::Completed,
                items: vec![],
                totals: Totals {
                    subtotal: Money {
                        amount: 0,
                        currency: "usd".to_string(),
                    },
                    tax: None,
                    shipping: None,
                    discount: None,
                    grand_total: Money {
                        amount: 0,
                        currency: "usd".to_string(),
                    },
                },
                fulfillment: None,
                customer: None,
                links: None,
                messages: None,
                created_at: Utc::now().to_rfc3339(),
                updated_at: Utc::now().to_rfc3339(),
            }
        });

        let order = map_order(body.order);

        Ok(CheckoutSessionWithOrder { session, order })
    }
}

fn build_line_items(items: &[RequestItem]) -> Result<Vec<ShopifyLineItemPayload>, ServiceError> {
    items
        .iter()
        .map(|item| {
            let variant_id = item.id.parse::<i64>().map_err(|_| {
                ServiceError::InvalidInput(format!(
                    "Shopify integration requires numeric variant IDs. Got: {}",
                    item.id
                ))
            })?;
            Ok(ShopifyLineItemPayload {
                variant_id,
                quantity: item.quantity,
            })
        })
        .collect()
}

fn extract_email(customer: Option<&Customer>) -> Option<String> {
    customer
        .and_then(|c| extract_email_opt(c))
        .map(|s| s.to_string())
}

fn extract_email_opt(customer: &Customer) -> Option<&str> {
    customer
        .billing_address
        .as_ref()
        .and_then(|addr| addr.email.as_deref())
        .or_else(|| {
            customer
                .shipping_address
                .as_ref()
                .and_then(|addr| addr.email.as_deref())
        })
}

fn parse_money(value: Option<&str>) -> Option<i64> {
    value.and_then(|s| {
        s.parse::<f64>()
            .ok()
            .map(|amount| (amount * 100.0).round() as i64)
    })
}

fn map_checkout(checkout: ShopifyCheckout) -> CheckoutSession {
    let status = if checkout.completed_at.is_some() {
        CheckoutSessionStatus::Completed
    } else if checkout.email.is_some() || checkout.billing_address.is_some() {
        CheckoutSessionStatus::ReadyForPayment
    } else {
        CheckoutSessionStatus::NotReadyForPayment
    };

    let items = checkout
        .line_items
        .into_iter()
        .map(|item| LineItem {
            id: item
                .variant_id
                .map(|id| id.to_string())
                .or_else(|| item.id.map(|id| id.to_string()))
                .unwrap_or_else(|| item.title.clone()),
            title: item.title,
            quantity: item.quantity as i32,
            unit_price: Money {
                amount: parse_money(Some(&item.price)).unwrap_or_default(),
                currency: checkout.currency.clone(),
            },
            variant_id: item.variant_id.map(|id| id.to_string()),
            sku: item.sku,
            image_url: item.image.and_then(|img| img.src),
        })
        .collect::<Vec<_>>();

    let totals = Totals {
        subtotal: Money {
            amount: parse_money(checkout.subtotal_price.as_deref()).unwrap_or_default(),
            currency: checkout.currency.clone(),
        },
        tax: parse_money(checkout.total_tax.as_deref()).map(|amount| Money {
            amount,
            currency: checkout.currency.clone(),
        }),
        shipping: checkout
            .shipping_lines
            .and_then(|mut lines| lines.pop())
            .and_then(|line| parse_money(Some(&line.price)))
            .map(|amount| Money {
                amount,
                currency: checkout.currency.clone(),
            }),
        discount: None,
        grand_total: Money {
            amount: parse_money(checkout.total_price.as_deref()).unwrap_or_default(),
            currency: checkout.currency.clone(),
        },
    };

    let fulfillment = checkout.shipping_rate.map(|rate| FulfillmentState {
        selected_id: rate.handle,
        options: Some(vec![FulfillmentChoice {
            id: rate
                .service_name
                .clone()
                .unwrap_or_else(|| "shipping".to_string()),
            label: rate.service_name.unwrap_or_else(|| "Shipping".to_string()),
            price: Money {
                amount: parse_money(rate.price.as_deref()).unwrap_or_default(),
                currency: checkout.currency.clone(),
            },
            est_delivery: rate
                .delivery_expectation
                .map(|expectation| EstimatedDelivery {
                    earliest: expectation.min.delivery_date,
                    latest: expectation.max.delivery_date,
                }),
        }]),
    });

    let customer = if checkout.billing_address.is_some() || checkout.shipping_address.is_some() {
        Some(Customer {
            billing_address: checkout.billing_address.map(map_shopify_address),
            shipping_address: checkout.shipping_address.map(map_shopify_address),
        })
    } else {
        None
    };

    let links = checkout.order_status_url.map(|url| Links {
        terms: None,
        privacy: None,
        order_permalink: Some(url),
    });

    let messages_payload = checkout.messages.as_ref().map(|msgs| {
        msgs.iter()
            .map(|msg| Message {
                message_type: MessageType::Error,
                code: Some(msg.clone()),
                message: "Shopify reported a checkout error".to_string(),
                param: None,
            })
            .collect::<Vec<_>>()
    });

    CheckoutSession {
        id: checkout.token,
        status,
        items,
        totals,
        fulfillment,
        customer,
        links,
        messages: messages_payload,
        created_at: checkout
            .created_at
            .unwrap_or_else(|| Utc::now().to_rfc3339()),
        updated_at: checkout
            .updated_at
            .unwrap_or_else(|| Utc::now().to_rfc3339()),
    }
}

fn map_order(order: ShopifyOrder) -> Order {
    Order {
        id: order.id.to_string(),
        checkout_session_id: order.checkout_token.unwrap_or_default(),
        status: match order.financial_status.as_deref() {
            Some("paid") | Some("captured") => OrderStatus::Placed,
            Some("refunded") => OrderStatus::Refunded,
            Some("voided") | Some("cancelled") => OrderStatus::Failed,
            _ => OrderStatus::Placed,
        },
        permalink_url: order.order_status_url,
    }
}

fn shopify_address_from(address: &Address) -> ShopifyAddressPayload {
    let (first_name, last_name) = split_name(address.name.as_deref());
    ShopifyAddressPayload {
        first_name: first_name.map(|s| s.to_string()),
        last_name: last_name.map(|s| s.to_string()),
        address1: address.line1.clone(),
        address2: address.line2.clone(),
        city: Some(address.city.clone()),
        province_code: address.region.clone(),
        zip: Some(address.postal_code.clone()),
        country_code: Some(address.country.clone()),
        phone: address.phone.clone(),
    }
}

fn map_shopify_address(address: ShopifyAddress) -> Address {
    let name = match (address.first_name, address.last_name) {
        (Some(first), Some(last)) => Some(format!("{} {}", first, last)),
        (Some(first), None) => Some(first),
        (None, Some(last)) => Some(last),
        _ => None,
    };

    Address {
        name,
        line1: address.address1.unwrap_or_default(),
        line2: address.address2,
        city: address.city.unwrap_or_default(),
        region: address.province_code,
        postal_code: address.zip.unwrap_or_default(),
        country: address.country_code.unwrap_or_else(|| "US".to_string()),
        phone: address.phone,
        email: None,
    }
}

fn split_name(name: Option<&str>) -> (Option<&str>, Option<&str>) {
    match name {
        Some(full) => {
            let mut parts = full.split_whitespace();
            let first = parts.next();
            let last = parts.next().or(first);
            (first, last)
        }
        None => (None, None),
    }
}

#[derive(Serialize)]
struct CreateCheckoutRequest {
    checkout: CheckoutPayload,
}

#[derive(Serialize)]
struct CheckoutPayload {
    line_items: Vec<ShopifyLineItemPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_address: Option<ShopifyAddressPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    billing_address: Option<ShopifyAddressPayload>,
}

#[derive(Serialize)]
struct UpdateCheckoutRequest {
    checkout: UpdateCheckoutPayload,
}

#[derive(Serialize)]
struct UpdateCheckoutPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    line_items: Option<Vec<ShopifyLineItemPayload>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shipping_address: Option<ShopifyAddressPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    billing_address: Option<ShopifyAddressPayload>,
}

#[derive(Serialize)]
struct ShopifyLineItemPayload {
    variant_id: i64,
    quantity: i32,
}

#[derive(Serialize)]
struct ShopifyAddressPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    first_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_name: Option<String>,
    address1: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    address2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    province_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    zip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    country_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phone: Option<String>,
}

#[derive(Serialize)]
struct CompleteCheckoutRequest {
    payment: ShopifyPaymentPayload,
}

#[derive(Serialize)]
struct ShopifyPaymentPayload {
    credit_card_token: String,
}

#[derive(Deserialize)]
struct ShopifyCheckoutEnvelope {
    checkout: ShopifyCheckout,
}

#[derive(Deserialize)]
struct ShopifyOrderEnvelope {
    order: ShopifyOrder,
}

#[derive(Deserialize)]
struct ShopifyCheckout {
    token: String,
    currency: String,
    created_at: Option<String>,
    updated_at: Option<String>,
    completed_at: Option<String>,
    email: Option<String>,
    subtotal_price: Option<String>,
    total_tax: Option<String>,
    total_price: Option<String>,
    line_items: Vec<ShopifyCheckoutLineItem>,
    shipping_rate: Option<ShopifyShippingRate>,
    shipping_lines: Option<Vec<ShopifyShippingLine>>,
    shipping_address: Option<ShopifyAddress>,
    billing_address: Option<ShopifyAddress>,
    order_status_url: Option<String>,
    #[serde(default)]
    messages: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ShopifyCheckoutLineItem {
    id: Option<i64>,
    variant_id: Option<i64>,
    title: String,
    quantity: i64,
    price: String,
    sku: Option<String>,
    image: Option<ShopifyImage>,
}

#[derive(Deserialize)]
struct ShopifyImage {
    src: Option<String>,
}

#[derive(Deserialize)]
struct ShopifyShippingRate {
    handle: Option<String>,
    price: Option<String>,
    service_name: Option<String>,
    delivery_expectation: Option<ShopifyDeliveryExpectation>,
}

#[derive(Deserialize)]
struct ShopifyDeliveryExpectation {
    min: ShopifyDeliveryWindow,
    max: ShopifyDeliveryWindow,
}

#[derive(Deserialize)]
struct ShopifyDeliveryWindow {
    delivery_date: Option<String>,
}

#[derive(Deserialize)]
struct ShopifyShippingLine {
    price: String,
}

#[derive(Deserialize)]
struct ShopifyAddress {
    first_name: Option<String>,
    last_name: Option<String>,
    address1: Option<String>,
    address2: Option<String>,
    city: Option<String>,
    province_code: Option<String>,
    zip: Option<String>,
    country_code: Option<String>,
    phone: Option<String>,
}

#[derive(Deserialize)]
struct ShopifyOrder {
    id: i64,
    checkout_token: Option<String>,
    financial_status: Option<String>,
    order_status_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_money() {
        assert_eq!(parse_money(Some("12.34")), Some(1234));
        assert_eq!(parse_money(Some("0")), Some(0));
        assert_eq!(parse_money(None), None);
    }

    #[test]
    fn test_split_name() {
        assert_eq!(split_name(Some("Jane Doe")), (Some("Jane"), Some("Doe")));
        assert_eq!(split_name(Some("Prince")), (Some("Prince"), Some("Prince")));
        assert_eq!(split_name(None), (None, None));
    }
}
