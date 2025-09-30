use serde::{Deserialize, Serialize};

// Request types

#[derive(Debug, Deserialize)]
pub struct CheckoutSessionCreateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buyer: Option<Buyer>,
    pub items: Vec<Item>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment_address: Option<Address>,
}

#[derive(Debug, Deserialize)]
pub struct CheckoutSessionUpdateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buyer: Option<Buyer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<Item>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment_address: Option<Address>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment_option_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CheckoutSessionCompleteRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buyer: Option<Buyer>,
    pub payment_data: PaymentData,
}

// Response types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSession {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buyer: Option<Buyer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_provider: Option<PaymentProvider>,
    pub status: String,
    pub currency: String,
    pub line_items: Vec<LineItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment_address: Option<Address>,
    pub fulfillment_options: Vec<FulfillmentOption>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment_option_id: Option<String>,
    pub totals: Vec<Total>,
    pub messages: Vec<Message>,
    pub links: Vec<Link>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSessionWithOrder {
    #[serde(flatten)]
    pub session: CheckoutSession,
    pub order: Order,
}

// Data models

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    pub name: String,
    pub line_one: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_two: Option<String>,
    pub city: String,
    pub state: String,
    pub country: String,
    pub postal_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Buyer {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub quantity: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentProvider {
    pub provider: String,
    pub supported_payment_methods: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineItem {
    pub id: String,
    pub item: Item,
    pub base_amount: i64,
    pub discount: i64,
    pub subtotal: i64,
    pub tax: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Total {
    #[serde(rename = "type")]
    pub total_type: String,
    pub display_text: String,
    pub amount: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FulfillmentOption {
    #[serde(rename = "shipping")]
    Shipping(FulfillmentOptionShipping),
    #[serde(rename = "digital")]
    Digital(FulfillmentOptionDigital),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FulfillmentOptionShipping {
    #[serde(rename = "type")]
    pub option_type: String,
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carrier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub earliest_delivery_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_delivery_time: Option<String>,
    pub subtotal: String,
    pub tax: String,
    pub total: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FulfillmentOptionDigital {
    #[serde(rename = "type")]
    pub option_type: String,
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    pub subtotal: String,
    pub tax: String,
    pub total: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    #[serde(rename = "info")]
    Info(MessageInfo),
    #[serde(rename = "error")]
    Error(MessageError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageInfo {
    #[serde(rename = "type")]
    pub message_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
    pub content_type: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageError {
    #[serde(rename = "type")]
    pub message_type: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
    pub content_type: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    #[serde(rename = "type")]
    pub link_type: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentData {
    pub token: String,
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_address: Option<Address>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub checkout_session_id: String,
    pub permalink_url: String,
} 