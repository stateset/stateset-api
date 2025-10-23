use crate::{
    cache::InMemoryCache,
    errors::ServiceError,
    events::{Event, EventSender},
};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, instrument};
use uuid::Uuid;

/// Agentic checkout service for ChatGPT-driven checkout flow
#[derive(Clone)]
pub struct AgenticCheckoutService {
    db: Arc<DatabaseConnection>,
    cache: Arc<InMemoryCache>,
    event_sender: Arc<EventSender>,
}

impl AgenticCheckoutService {
    pub fn new(
        db: Arc<DatabaseConnection>,
        cache: Arc<InMemoryCache>,
        event_sender: Arc<EventSender>,
    ) -> Self {
        Self {
            db,
            cache,
            event_sender,
        }
    }

    /// Create checkout session
    #[instrument(skip(self))]
    pub async fn create_session(
        &self,
        request: CheckoutSessionCreateRequest,
    ) -> Result<CheckoutSession, ServiceError> {
        // Validate items exist and calculate totals
        let line_items = self.build_line_items(&request.items).await?;

        let session_id = Uuid::new_v4();
        let currency = "USD".to_string();

        // Calculate totals
        let totals =
            self.calculate_totals(&line_items, request.fulfillment_address.as_ref(), None)?;

        // Determine fulfillment options
        let fulfillment_options =
            self.get_fulfillment_options(request.fulfillment_address.as_ref())?;

        // Determine status
        let status = self.determine_status(&request, &line_items, None);

        let session = CheckoutSession {
            id: session_id.to_string(),
            buyer: request.buyer,
            payment_provider: Some(PaymentProvider {
                provider: "stripe".to_string(),
                supported_payment_methods: vec!["card".to_string()],
            }),
            status,
            currency,
            line_items,
            fulfillment_address: request.fulfillment_address,
            fulfillment_options,
            fulfillment_option_id: None,
            totals,
            messages: vec![],
            links: vec![
                Link {
                    link_type: "terms_of_use".to_string(),
                    url: "https://merchant.example.com/terms".to_string(),
                },
                Link {
                    link_type: "privacy_policy".to_string(),
                    url: "https://merchant.example.com/privacy".to_string(),
                },
            ],
        };

        // Store session in cache with 1 hour TTL
        self.save_session(&session).await?;

        self.event_sender
            .send_or_log(Event::CheckoutStarted {
                cart_id: Uuid::nil(), // Not cart-based
                session_id: Uuid::parse_str(&session.id).unwrap(),
            })
            .await;

        info!("Created checkout session: {}", session.id);
        Ok(session)
    }

    /// Get checkout session
    #[instrument(skip(self))]
    pub async fn get_session(&self, session_id: &str) -> Result<CheckoutSession, ServiceError> {
        let cache_key = format!("checkout_session:{}", session_id);

        let cached = self
            .cache
            .get(&cache_key)
            .await
            .map_err(|e| ServiceError::CacheError(e.to_string()))?;

        match cached {
            Some(data) => {
                let session: CheckoutSession = serde_json::from_str(&data)
                    .map_err(|e| ServiceError::SerializationError(e.to_string()))?;
                Ok(session)
            }
            None => Err(ServiceError::NotFound(format!(
                "Checkout session {} not found",
                session_id
            ))),
        }
    }

    /// Update checkout session
    #[instrument(skip(self))]
    pub async fn update_session(
        &self,
        session_id: &str,
        request: CheckoutSessionUpdateRequest,
    ) -> Result<CheckoutSession, ServiceError> {
        let mut session = self.get_session(session_id).await?;

        // Check if already completed or canceled
        if session.status == "completed" || session.status == "canceled" {
            return Err(ServiceError::InvalidOperation(
                "Cannot update completed or canceled session".to_string(),
            ));
        }

        // Update fields if provided
        if let Some(buyer) = request.buyer {
            session.buyer = Some(buyer);
        }

        if let Some(items) = request.items {
            session.line_items = self.build_line_items(&items).await?;
        }

        if let Some(address) = request.fulfillment_address {
            session.fulfillment_address = Some(address);
            // Recalculate fulfillment options
            session.fulfillment_options =
                self.get_fulfillment_options(session.fulfillment_address.as_ref())?;
        }

        if let Some(option_id) = request.fulfillment_option_id {
            // Validate option exists
            if !session.fulfillment_options.iter().any(|opt| match opt {
                FulfillmentOption::Shipping(s) => s.id == option_id,
                FulfillmentOption::Digital(d) => d.id == option_id,
            }) {
                return Err(ServiceError::InvalidInput(
                    "Invalid fulfillment option".to_string(),
                ));
            }
            session.fulfillment_option_id = Some(option_id);
        }

        // Recalculate totals
        session.totals = self.calculate_totals(
            &session.line_items,
            session.fulfillment_address.as_ref(),
            session.fulfillment_option_id.as_deref(),
        )?;

        // Update status
        session.status = self.determine_status_from_session(&session);

        // Save updated session
        self.save_session(&session).await?;

        info!("Updated checkout session: {}", session.id);
        Ok(session)
    }

    /// Complete checkout session
    #[instrument(skip(self))]
    pub async fn complete_session(
        &self,
        session_id: &str,
        request: CheckoutSessionCompleteRequest,
    ) -> Result<CheckoutSessionWithOrder, ServiceError> {
        let mut session = self.get_session(session_id).await?;

        // Check if already completed or canceled
        if session.status == "completed" {
            return Err(ServiceError::InvalidOperation(
                "Session already completed".to_string(),
            ));
        }
        if session.status == "canceled" {
            return Err(ServiceError::InvalidOperation(
                "Session is canceled".to_string(),
            ));
        }

        // Update buyer if provided
        if let Some(buyer) = request.buyer {
            session.buyer = Some(buyer);
        }

        // Validate session is ready
        if session.buyer.is_none() {
            return Err(ServiceError::InvalidOperation(
                "Buyer information required".to_string(),
            ));
        }
        if session.fulfillment_address.is_none() {
            return Err(ServiceError::InvalidOperation(
                "Fulfillment address required".to_string(),
            ));
        }
        if session.fulfillment_option_id.is_none() {
            return Err(ServiceError::InvalidOperation(
                "Fulfillment option required".to_string(),
            ));
        }

        // Process payment
        self.process_payment(&request.payment_data).await?;

        // Create order
        let order = self.create_order_from_session(&session).await?;

        // Update session status
        session.status = "completed".to_string();
        self.save_session(&session).await?;

        self.event_sender
            .send_or_log(Event::CheckoutCompleted {
                session_id: Uuid::parse_str(&session.id).unwrap(),
                order_id: Uuid::parse_str(&order.id).unwrap(),
            })
            .await;

        info!("Completed checkout session: {}", session.id);

        Ok(CheckoutSessionWithOrder { session, order })
    }

    /// Cancel checkout session
    #[instrument(skip(self))]
    pub async fn cancel_session(&self, session_id: &str) -> Result<CheckoutSession, ServiceError> {
        let mut session = self.get_session(session_id).await?;

        if session.status == "completed" {
            return Err(ServiceError::InvalidOperation(
                "Cannot cancel completed session".to_string(),
            ));
        }
        if session.status == "canceled" {
            return Err(ServiceError::InvalidOperation(
                "Session already canceled".to_string(),
            ));
        }

        session.status = "canceled".to_string();
        self.save_session(&session).await?;

        info!("Canceled checkout session: {}", session.id);
        Ok(session)
    }

    // Private helper methods

    async fn build_line_items(&self, items: &[Item]) -> Result<Vec<LineItem>, ServiceError> {
        let mut line_items = Vec::new();

        for item in items {
            // In a real implementation, fetch product details from database
            // For now, use mock data
            let base_amount = 5000; // $50.00 in cents
            let discount = 0;
            let subtotal = base_amount * item.quantity as i64 - discount;
            let tax = subtotal * 875 / 10000; // 8.75% tax
            let total = subtotal + tax;

            line_items.push(LineItem {
                id: Uuid::new_v4().to_string(),
                item: item.clone(),
                base_amount,
                discount,
                subtotal,
                tax,
                total,
            });
        }

        Ok(line_items)
    }

    fn calculate_totals(
        &self,
        line_items: &[LineItem],
        address: Option<&Address>,
        fulfillment_option_id: Option<&str>,
    ) -> Result<Vec<Total>, ServiceError> {
        let mut totals = Vec::new();

        // Items base amount
        let items_base: i64 = line_items.iter().map(|item| item.base_amount).sum();
        totals.push(Total {
            total_type: "items_base_amount".to_string(),
            display_text: "Items".to_string(),
            amount: items_base,
        });

        // Items discount
        let items_discount: i64 = line_items.iter().map(|item| item.discount).sum();
        if items_discount > 0 {
            totals.push(Total {
                total_type: "items_discount".to_string(),
                display_text: "Discount".to_string(),
                amount: -items_discount,
            });
        }

        // Subtotal
        let subtotal: i64 = line_items.iter().map(|item| item.subtotal).sum();
        totals.push(Total {
            total_type: "subtotal".to_string(),
            display_text: "Subtotal".to_string(),
            amount: subtotal,
        });

        // Fulfillment
        let fulfillment_cost = if fulfillment_option_id.is_some() {
            1000 // $10.00 shipping
        } else {
            0
        };
        if fulfillment_cost > 0 {
            totals.push(Total {
                total_type: "fulfillment".to_string(),
                display_text: "Shipping".to_string(),
                amount: fulfillment_cost,
            });
        }

        // Tax
        let tax: i64 = line_items.iter().map(|item| item.tax).sum();
        totals.push(Total {
            total_type: "tax".to_string(),
            display_text: "Tax".to_string(),
            amount: tax,
        });

        // Total
        let total = subtotal + fulfillment_cost + tax;
        totals.push(Total {
            total_type: "total".to_string(),
            display_text: "Total".to_string(),
            amount: total,
        });

        Ok(totals)
    }

    fn get_fulfillment_options(
        &self,
        address: Option<&Address>,
    ) -> Result<Vec<FulfillmentOption>, ServiceError> {
        let mut options = Vec::new();

        if address.is_some() {
            // Shipping options
            options.push(FulfillmentOption::Shipping(FulfillmentOptionShipping {
                option_type: "shipping".to_string(),
                id: "standard_shipping".to_string(),
                title: "Standard Shipping".to_string(),
                subtitle: Some("5-7 business days".to_string()),
                carrier: Some("USPS".to_string()),
                earliest_delivery_time: None,
                latest_delivery_time: None,
                subtotal: "1000".to_string(), // $10.00
                tax: "88".to_string(),        // $0.88
                total: "1088".to_string(),    // $10.88
            }));

            options.push(FulfillmentOption::Shipping(FulfillmentOptionShipping {
                option_type: "shipping".to_string(),
                id: "express_shipping".to_string(),
                title: "Express Shipping".to_string(),
                subtitle: Some("2-3 business days".to_string()),
                carrier: Some("FedEx".to_string()),
                earliest_delivery_time: None,
                latest_delivery_time: None,
                subtotal: "2500".to_string(), // $25.00
                tax: "219".to_string(),       // $2.19
                total: "2719".to_string(),    // $27.19
            }));
        }

        Ok(options)
    }

    fn determine_status(
        &self,
        request: &CheckoutSessionCreateRequest,
        _line_items: &[LineItem],
        _fulfillment_option_id: Option<&str>,
    ) -> String {
        // Check if ready for payment
        if request.buyer.is_some()
            && request.fulfillment_address.is_some()
            && !_line_items.is_empty()
        {
            "ready_for_payment".to_string()
        } else {
            "not_ready_for_payment".to_string()
        }
    }

    fn determine_status_from_session(&self, session: &CheckoutSession) -> String {
        if session.buyer.is_some()
            && session.fulfillment_address.is_some()
            && session.fulfillment_option_id.is_some()
            && !session.line_items.is_empty()
        {
            "ready_for_payment".to_string()
        } else {
            "not_ready_for_payment".to_string()
        }
    }

    async fn process_payment(&self, payment_data: &PaymentData) -> Result<(), ServiceError> {
        // Simulate payment processing with Stripe
        info!(
            "Processing payment with provider: {}",
            payment_data.provider
        );

        // In real implementation, call Stripe API with payment_data.token
        // For now, simulate success

        Ok(())
    }

    async fn create_order_from_session(
        &self,
        session: &CheckoutSession,
    ) -> Result<Order, ServiceError> {
        let order_id = Uuid::new_v4();

        Ok(Order {
            id: order_id.to_string(),
            checkout_session_id: session.id.clone(),
            permalink_url: format!("https://merchant.example.com/orders/{}", order_id),
        })
    }

    async fn save_session(&self, session: &CheckoutSession) -> Result<(), ServiceError> {
        let cache_key = format!("checkout_session:{}", session.id);
        let data = serde_json::to_string(session)
            .map_err(|e| ServiceError::SerializationError(e.to_string()))?;

        self.cache
            .set(&cache_key, &data, Some(Duration::from_secs(3600)))
            .await
            .map_err(|e| ServiceError::CacheError(e.to_string()))?;

        Ok(())
    }
}

// Data models matching OpenAPI spec

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
