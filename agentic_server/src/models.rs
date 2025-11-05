use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckoutSessionStatus {
    NotReadyForPayment,
    ReadyForPayment,
    Completed,
    Canceled,
}

impl Default for CheckoutSessionStatus {
    fn default() -> Self {
        Self::NotReadyForPayment
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSessionCreateRequest {
    pub items: Vec<RequestItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<Customer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment: Option<FulfillmentState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSessionUpdateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<RequestItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<Customer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment: Option<FulfillmentState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSessionCompleteRequest {
    pub payment: PaymentRequest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<Customer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment: Option<FulfillmentState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegated_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestItem {
    pub id: String,
    pub quantity: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSession {
    pub id: String,
    pub status: CheckoutSessionStatus,
    pub items: Vec<LineItem>,
    pub totals: Totals,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment: Option<FulfillmentState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<Customer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<Links>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<Message>>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSessionWithOrder {
    #[serde(flatten)]
    pub session: CheckoutSession,
    pub order: Order,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineItem {
    pub id: String,
    pub title: String,
    pub quantity: i32,
    pub unit_price: Money,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sku: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Money {
    pub amount: i64,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Totals {
    pub subtotal: Money,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tax: Option<Money>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping: Option<Money>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discount: Option<Money>,
    pub grand_total: Money,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FulfillmentState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<FulfillmentChoice>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FulfillmentChoice {
    pub id: String,
    pub label: String,
    pub price: Money,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub est_delivery: Option<EstimatedDelivery>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstimatedDelivery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub earliest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Customer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_address: Option<Address>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping_address: Option<Address>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub line1: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line2: Option<String>,
    pub city: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    pub postal_code: String,
    pub country: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Links {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terms: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub privacy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_permalink: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    #[serde(rename = "type")]
    pub message_type: MessageType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageType {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub checkout_session_id: String,
    pub status: OrderStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permalink_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Placed,
    Failed,
    Refunded,
}
