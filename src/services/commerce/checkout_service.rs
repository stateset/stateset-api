use crate::{
    entities::{
        commerce::{cart, cart_item, Cart, CartItem},
        order::{self},
        order_item::{self},
    },
    errors::ServiceError,
    events::{Event, EventSender},
    services::orders::OrderService,
};
use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, DatabaseConnection, EntityTrait, ModelTrait, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, instrument};
use uuid::Uuid;

/// Checkout service for converting carts to orders
#[derive(Clone)]
pub struct CheckoutService {
    db: Arc<DatabaseConnection>,
    event_sender: Arc<EventSender>,
    order_service: Arc<OrderService>,
}

impl CheckoutService {
    pub fn new(
        db: Arc<DatabaseConnection>,
        event_sender: Arc<EventSender>,
        order_service: Arc<OrderService>,
    ) -> Self {
        Self {
            db,
            event_sender,
            order_service,
        }
    }

    /// Start checkout session
    #[instrument(skip(self))]
    pub async fn start_checkout(&self, cart_id: Uuid) -> Result<CheckoutSession, ServiceError> {
        let cart = Cart::find_by_id(cart_id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| ServiceError::NotFound(format!("Cart {} not found", cart_id)))?;

        if cart.status != cart::CartStatus::Active {
            return Err(ServiceError::InvalidOperation(
                "Cart is not active".to_string(),
            ));
        }

        // Verify cart has items
        let items = cart.find_related(CartItem).all(&*self.db).await?;
        if items.is_empty() {
            return Err(ServiceError::InvalidOperation("Cart is empty".to_string()));
        }

        let session_id = Uuid::new_v4();

        // Update cart status to converting
        let mut cart_update: cart::ActiveModel = cart.clone().into();
        cart_update.status = Set(cart::CartStatus::Converting);
        cart_update.updated_at = Set(Utc::now());
        cart_update.update(&*self.db).await?;

        self.event_sender
            .send(Event::CheckoutStarted {
                cart_id,
                session_id,
            })
            .await;

        Ok(CheckoutSession {
            id: session_id,
            cart_id,
            cart,
            items,
            step: CheckoutStep::CustomerInfo,
            customer_email: None,
            shipping_address: None,
            billing_address: None,
            shipping_method: None,
            payment_method: None,
        })
    }

    /// Update checkout session with customer info
    #[instrument(skip(self))]
    pub async fn set_customer_info(
        &self,
        session: &mut CheckoutSession,
        input: CustomerInfoInput,
    ) -> Result<(), ServiceError> {
        session.customer_email = Some(input.email);
        session.step = CheckoutStep::ShippingAddress;
        Ok(())
    }

    /// Set shipping address
    #[instrument(skip(self))]
    pub async fn set_shipping_address(
        &self,
        session: &mut CheckoutSession,
        address: Address,
    ) -> Result<(), ServiceError> {
        session.shipping_address = Some(address.clone());
        session.billing_address = Some(address); // Default to same as shipping
        session.step = CheckoutStep::ShippingMethod;
        Ok(())
    }

    /// Set shipping method and calculate shipping
    #[instrument(skip(self))]
    pub async fn set_shipping_method(
        &self,
        session: &mut CheckoutSession,
        method: ShippingMethod,
    ) -> Result<ShippingRate, ServiceError> {
        // Calculate shipping rate based on method and address
        let rate =
            self.calculate_shipping_rate(&method, session.shipping_address.as_ref().unwrap())?;

        session.shipping_method = Some(method);
        session.step = CheckoutStep::Payment;

        Ok(rate)
    }

    /// Complete checkout and create order
    #[instrument(skip(self))]
    pub async fn complete_checkout(
        &self,
        session: CheckoutSession,
        payment_info: PaymentInfo,
    ) -> Result<order::Model, ServiceError> {
        let txn = self.db.begin().await?;

        // Validate session is ready
        if session.customer_email.is_none()
            || session.shipping_address.is_none()
            || session.shipping_method.is_none()
        {
            return Err(ServiceError::InvalidOperation(
                "Checkout session incomplete".to_string(),
            ));
        }

        // Calculate totals
        let subtotal = session.cart.subtotal;
        let shipping_total = self
            .calculate_shipping_rate(
                session.shipping_method.as_ref().unwrap(),
                session.shipping_address.as_ref().unwrap(),
            )?
            .amount;
        let tax_total = self.calculate_tax(subtotal, session.shipping_address.as_ref().unwrap())?;
        let total = subtotal + shipping_total + tax_total;

        // Create order
        let order_id = Uuid::new_v4();
        let order = order::ActiveModel {
            id: Set(order_id),
            order_number: Set(format!("ORD-{}", order_id.to_string()[..8].to_uppercase())),
            customer_id: Set(session.cart.customer_id.unwrap_or_else(|| Uuid::new_v4())),
            status: Set("pending".to_string()),
            order_date: Set(Utc::now()),
            total_amount: Set(total),
            currency: Set(session.cart.currency.clone()),
            payment_status: Set("pending".to_string()),
            fulfillment_status: Set("unfulfilled".to_string()),
            payment_method: Set(Some("card".to_string())),
            shipping_method: Set(session.shipping_method.as_ref().map(|m| format!("{:?}", m))),
            tracking_number: Set(None),
            shipping_address: Set(Some(
                serde_json::to_value(&session.shipping_address)
                    .unwrap()
                    .to_string(),
            )),
            billing_address: Set(Some(
                serde_json::to_value(&session.billing_address)
                    .unwrap()
                    .to_string(),
            )),
            is_archived: Set(false),
            created_at: Set(Utc::now()),
            updated_at: Set(Some(Utc::now())),
            notes: Set(None),
            version: Set(1),
        };

        let order = order.insert(&txn).await?;

        // Create order items from cart items
        for cart_item in &session.items {
            let order_item = order_item::ActiveModel {
                id: Set(Uuid::new_v4()),
                order_id: Set(order_id),
                product_id: Set(cart_item.variant_id), // Using variant_id as product_id
                sku: Set(format!(
                    "SKU-{}",
                    cart_item.variant_id.to_string()[..8].to_uppercase()
                )),
                name: Set(format!(
                    "Product {}",
                    cart_item
                        .variant_id
                        .to_string()
                        .chars()
                        .take(8)
                        .collect::<String>()
                )),
                quantity: Set(cart_item.quantity),
                unit_price: Set(cart_item.unit_price),
                total_price: Set(cart_item.line_total),
                discount: Set(cart_item.discount_amount),
                tax_rate: Set(Decimal::from(0)),
                tax_amount: Set(Decimal::from(0)),
                status: Set("confirmed".to_string()),
                notes: Set(None),
                created_at: Set(Utc::now()),
                updated_at: Set(Some(Utc::now())),
            };
            order_item.insert(&txn).await?;
        }

        // Process payment
        self.process_payment(&payment_info, total).await?;

        // Update cart status to converted
        let mut cart_update: cart::ActiveModel = session.cart.into();
        cart_update.status = Set(cart::CartStatus::Converted);
        cart_update.updated_at = Set(Utc::now());
        cart_update.update(&txn).await?;

        txn.commit().await?;

        self.event_sender
            .send(Event::CheckoutCompleted {
                session_id: session.id,
                order_id,
            })
            .await;

        self.event_sender.send(Event::OrderCreated(order_id)).await;

        info!(
            "Checkout completed: order {} created from cart {}",
            order_id, session.cart_id
        );
        Ok(order)
    }

    /// Calculate shipping rate
    fn calculate_shipping_rate(
        &self,
        method: &ShippingMethod,
        address: &Address,
    ) -> Result<ShippingRate, ServiceError> {
        // Simplified shipping calculation
        let base_rate = match method {
            ShippingMethod::Standard => Decimal::from(10),
            ShippingMethod::Express => Decimal::from(25),
            ShippingMethod::Overnight => Decimal::from(50),
        };

        Ok(ShippingRate {
            method: method.clone(),
            amount: base_rate,
            estimated_days: match method {
                ShippingMethod::Standard => 5,
                ShippingMethod::Express => 2,
                ShippingMethod::Overnight => 1,
            },
        })
    }

    /// Calculate tax
    fn calculate_tax(&self, subtotal: Decimal, address: &Address) -> Result<Decimal, ServiceError> {
        // Simplified tax calculation - would integrate with tax provider
        let tax_rate = Decimal::new(875, 3); // 8.75%
        Ok(subtotal * tax_rate / Decimal::from(100))
    }

    /// Process payment
    async fn process_payment(
        &self,
        payment_info: &PaymentInfo,
        amount: Decimal,
    ) -> Result<PaymentResult, ServiceError> {
        // This would integrate with payment gateway
        info!(
            "Processing payment of {} via {:?}",
            amount, payment_info.method
        );

        // Simulate payment processing
        Ok(PaymentResult {
            transaction_id: Uuid::new_v4().to_string(),
            status: PaymentStatus::Approved,
            amount,
        })
    }
}

/// Checkout session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSession {
    pub id: Uuid,
    pub cart_id: Uuid,
    pub cart: cart::Model,
    pub items: Vec<cart_item::Model>,
    pub step: CheckoutStep,
    pub customer_email: Option<String>,
    pub shipping_address: Option<Address>,
    pub billing_address: Option<Address>,
    pub shipping_method: Option<ShippingMethod>,
    pub payment_method: Option<PaymentMethod>,
}

/// Checkout steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CheckoutStep {
    CustomerInfo,
    ShippingAddress,
    ShippingMethod,
    Payment,
}

/// Customer info input
#[derive(Debug, Deserialize)]
pub struct CustomerInfoInput {
    pub email: String,
    pub subscribe_newsletter: bool,
}

/// Address structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    pub first_name: String,
    pub last_name: String,
    pub company: Option<String>,
    pub address_line_1: String,
    pub address_line_2: Option<String>,
    pub city: String,
    pub province: String,
    pub country_code: String,
    pub postal_code: String,
    pub phone: Option<String>,
}

/// Shipping methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShippingMethod {
    Standard,
    Express,
    Overnight,
}

/// Shipping rate
#[derive(Debug, Serialize)]
pub struct ShippingRate {
    pub method: ShippingMethod,
    pub amount: Decimal,
    pub estimated_days: u32,
}

/// Payment method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaymentMethod {
    CreditCard,
    PayPal,
    ApplePay,
    GooglePay,
}

/// Payment info
#[derive(Debug, Deserialize)]
pub struct PaymentInfo {
    pub method: PaymentMethod,
    pub token: String, // Payment token from frontend
}

/// Payment result
#[derive(Debug)]
pub struct PaymentResult {
    pub transaction_id: String,
    pub status: PaymentStatus,
    pub amount: Decimal,
}

#[derive(Debug)]
pub enum PaymentStatus {
    Approved,
    Declined,
    Error,
}
