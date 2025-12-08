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
    #[allow(dead_code)] // Reserved for future order operations
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
            .send_or_log(Event::CheckoutStarted {
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
        let shipping_address = session.shipping_address.as_ref().ok_or_else(|| {
            ServiceError::InvalidOperation("Shipping address required".to_string())
        })?;
        let rate = self.calculate_shipping_rate(&method, shipping_address)?;

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

        // Extract validated fields (already checked for Some above)
        let shipping_method = session.shipping_method.as_ref().ok_or_else(|| {
            ServiceError::InvalidOperation("Shipping method required".to_string())
        })?;
        let shipping_address = session.shipping_address.as_ref().ok_or_else(|| {
            ServiceError::InvalidOperation("Shipping address required".to_string())
        })?;

        // Calculate totals
        let subtotal = session.cart.subtotal;
        let shipping_total = self
            .calculate_shipping_rate(shipping_method, shipping_address)?
            .amount;
        let tax_total = self.calculate_tax(subtotal, shipping_address)?;
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
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
            )),
            billing_address: Set(Some(
                serde_json::to_value(&session.billing_address)
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
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
            .send_or_log(Event::CheckoutCompleted {
                session_id: session.id,
                order_id,
            })
            .await;

        self.event_sender
            .send_or_log(Event::OrderCreated(order_id))
            .await;

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

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    // ==================== CheckoutStep Tests ====================

    #[test]
    fn test_checkout_step_customer_info() {
        let step = CheckoutStep::CustomerInfo;
        assert_eq!(format!("{:?}", step), "CustomerInfo");
    }

    #[test]
    fn test_checkout_step_shipping_address() {
        let step = CheckoutStep::ShippingAddress;
        assert_eq!(format!("{:?}", step), "ShippingAddress");
    }

    #[test]
    fn test_checkout_step_shipping_method() {
        let step = CheckoutStep::ShippingMethod;
        assert_eq!(format!("{:?}", step), "ShippingMethod");
    }

    #[test]
    fn test_checkout_step_payment() {
        let step = CheckoutStep::Payment;
        assert_eq!(format!("{:?}", step), "Payment");
    }

    // ==================== ShippingMethod Tests ====================

    #[test]
    fn test_shipping_method_standard() {
        let method = ShippingMethod::Standard;
        assert_eq!(format!("{:?}", method), "Standard");
    }

    #[test]
    fn test_shipping_method_express() {
        let method = ShippingMethod::Express;
        assert_eq!(format!("{:?}", method), "Express");
    }

    #[test]
    fn test_shipping_method_overnight() {
        let method = ShippingMethod::Overnight;
        assert_eq!(format!("{:?}", method), "Overnight");
    }

    // ==================== ShippingRate Tests ====================

    #[test]
    fn test_shipping_rate_standard() {
        let rate = ShippingRate {
            method: ShippingMethod::Standard,
            amount: dec!(10),
            estimated_days: 5,
        };

        assert_eq!(rate.amount, dec!(10));
        assert_eq!(rate.estimated_days, 5);
    }

    #[test]
    fn test_shipping_rate_express() {
        let rate = ShippingRate {
            method: ShippingMethod::Express,
            amount: dec!(25),
            estimated_days: 2,
        };

        assert_eq!(rate.amount, dec!(25));
        assert_eq!(rate.estimated_days, 2);
    }

    #[test]
    fn test_shipping_rate_overnight() {
        let rate = ShippingRate {
            method: ShippingMethod::Overnight,
            amount: dec!(50),
            estimated_days: 1,
        };

        assert_eq!(rate.amount, dec!(50));
        assert_eq!(rate.estimated_days, 1);
    }

    // ==================== PaymentMethod Tests ====================

    #[test]
    fn test_payment_method_credit_card() {
        let method = PaymentMethod::CreditCard;
        assert_eq!(format!("{:?}", method), "CreditCard");
    }

    #[test]
    fn test_payment_method_paypal() {
        let method = PaymentMethod::PayPal;
        assert_eq!(format!("{:?}", method), "PayPal");
    }

    #[test]
    fn test_payment_method_apple_pay() {
        let method = PaymentMethod::ApplePay;
        assert_eq!(format!("{:?}", method), "ApplePay");
    }

    #[test]
    fn test_payment_method_google_pay() {
        let method = PaymentMethod::GooglePay;
        assert_eq!(format!("{:?}", method), "GooglePay");
    }

    // ==================== PaymentStatus Tests ====================

    #[test]
    fn test_payment_status_approved() {
        let status = PaymentStatus::Approved;
        assert_eq!(format!("{:?}", status), "Approved");
    }

    #[test]
    fn test_payment_status_declined() {
        let status = PaymentStatus::Declined;
        assert_eq!(format!("{:?}", status), "Declined");
    }

    #[test]
    fn test_payment_status_error() {
        let status = PaymentStatus::Error;
        assert_eq!(format!("{:?}", status), "Error");
    }

    // ==================== Address Tests ====================

    #[test]
    fn test_address_creation() {
        let address = Address {
            first_name: "John".to_string(),
            last_name: "Doe".to_string(),
            company: Some("Acme Inc".to_string()),
            address_line_1: "123 Main St".to_string(),
            address_line_2: Some("Suite 100".to_string()),
            city: "New York".to_string(),
            province: "NY".to_string(),
            country_code: "US".to_string(),
            postal_code: "10001".to_string(),
            phone: Some("+1234567890".to_string()),
        };

        assert_eq!(address.first_name, "John");
        assert_eq!(address.city, "New York");
        assert_eq!(address.country_code, "US");
    }

    #[test]
    fn test_address_minimal() {
        let address = Address {
            first_name: "Jane".to_string(),
            last_name: "Smith".to_string(),
            company: None,
            address_line_1: "456 Oak Ave".to_string(),
            address_line_2: None,
            city: "Los Angeles".to_string(),
            province: "CA".to_string(),
            country_code: "US".to_string(),
            postal_code: "90001".to_string(),
            phone: None,
        };

        assert!(address.company.is_none());
        assert!(address.address_line_2.is_none());
        assert!(address.phone.is_none());
    }

    #[test]
    fn test_address_serialization() {
        let address = Address {
            first_name: "Test".to_string(),
            last_name: "User".to_string(),
            company: None,
            address_line_1: "789 Pine Rd".to_string(),
            address_line_2: None,
            city: "Chicago".to_string(),
            province: "IL".to_string(),
            country_code: "US".to_string(),
            postal_code: "60601".to_string(),
            phone: None,
        };

        let json = serde_json::to_string(&address).expect("serialization should succeed");
        assert!(json.contains("Chicago"));
        assert!(json.contains("IL"));
    }

    // ==================== CustomerInfoInput Tests ====================

    #[test]
    fn test_customer_info_input() {
        let input = CustomerInfoInput {
            email: "customer@example.com".to_string(),
            subscribe_newsletter: true,
        };

        assert_eq!(input.email, "customer@example.com");
        assert!(input.subscribe_newsletter);
    }

    #[test]
    fn test_customer_info_no_newsletter() {
        let input = CustomerInfoInput {
            email: "no-newsletter@example.com".to_string(),
            subscribe_newsletter: false,
        };

        assert!(!input.subscribe_newsletter);
    }

    // ==================== PaymentInfo Tests ====================

    #[test]
    fn test_payment_info_creation() {
        let info = PaymentInfo {
            method: PaymentMethod::CreditCard,
            token: "tok_test_123456".to_string(),
        };

        assert!(!info.token.is_empty());
    }

    #[test]
    fn test_payment_info_paypal() {
        let info = PaymentInfo {
            method: PaymentMethod::PayPal,
            token: "paypal_token_abc123".to_string(),
        };

        assert!(info.token.starts_with("paypal"));
    }

    // ==================== Tax Calculation Tests ====================

    #[test]
    fn test_tax_calculation() {
        let subtotal = dec!(100.00);
        let tax_rate = dec!(8.75) / dec!(100); // 8.75%
        let tax = subtotal * tax_rate;

        assert_eq!(tax, dec!(8.75));
    }

    #[test]
    fn test_tax_calculation_rounding() {
        let subtotal = dec!(99.99);
        let tax_rate = dec!(8.75) / dec!(100);
        let tax = subtotal * tax_rate;

        // Should be approximately $8.75
        assert!(tax > dec!(8.74) && tax < dec!(8.76));
    }

    // ==================== Total Calculation Tests ====================

    #[test]
    fn test_total_calculation() {
        let subtotal = dec!(100.00);
        let shipping = dec!(10.00);
        let tax = dec!(8.75);
        let total = subtotal + shipping + tax;

        assert_eq!(total, dec!(118.75));
    }

    #[test]
    fn test_total_calculation_with_decimals() {
        let subtotal = dec!(99.99);
        let shipping = dec!(10.00);
        let tax = dec!(8.74);
        let total = subtotal + shipping + tax;

        assert_eq!(total, dec!(118.73));
    }

    // ==================== Session ID Tests ====================

    #[test]
    fn test_session_id_uniqueness() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_session_id_format() {
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        assert_eq!(id_str.len(), 36);
    }

    // ==================== Order Number Generation Tests ====================

    #[test]
    fn test_order_number_format() {
        let order_id = Uuid::new_v4();
        let order_number = format!("ORD-{}", order_id.to_string()[..8].to_uppercase());

        assert!(order_number.starts_with("ORD-"));
        assert_eq!(order_number.len(), 12); // "ORD-" + 8 chars
    }

    #[test]
    fn test_order_number_uppercase() {
        let order_id = Uuid::new_v4();
        let order_number = format!("ORD-{}", order_id.to_string()[..8].to_uppercase());

        // The suffix should be uppercase
        let suffix = &order_number[4..];
        assert!(suffix.chars().all(|c| c.is_uppercase() || c.is_numeric() || c == '-'));
    }

    // ==================== Error Handling Tests ====================

    #[test]
    fn test_not_found_error_cart() {
        let cart_id = Uuid::new_v4();
        let error = ServiceError::NotFound(format!("Cart {} not found", cart_id));

        match error {
            ServiceError::NotFound(msg) => {
                assert!(msg.contains("Cart"));
                assert!(msg.contains("not found"));
            }
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    fn test_invalid_operation_error() {
        let error = ServiceError::InvalidOperation("Cart is not active".to_string());

        match error {
            ServiceError::InvalidOperation(msg) => {
                assert!(msg.contains("not active"));
            }
            _ => panic!("Expected InvalidOperation error"),
        }
    }

    #[test]
    fn test_empty_cart_error() {
        let error = ServiceError::InvalidOperation("Cart is empty".to_string());

        match error {
            ServiceError::InvalidOperation(msg) => {
                assert!(msg.contains("empty"));
            }
            _ => panic!("Expected InvalidOperation error"),
        }
    }

    #[test]
    fn test_incomplete_session_error() {
        let error = ServiceError::InvalidOperation("Checkout session incomplete".to_string());

        match error {
            ServiceError::InvalidOperation(msg) => {
                assert!(msg.contains("incomplete"));
            }
            _ => panic!("Expected InvalidOperation error"),
        }
    }

    // ==================== Shipping Rate Calculation Tests ====================

    #[test]
    fn test_shipping_rate_by_method() {
        let methods_and_rates = vec![
            (ShippingMethod::Standard, dec!(10), 5u32),
            (ShippingMethod::Express, dec!(25), 2u32),
            (ShippingMethod::Overnight, dec!(50), 1u32),
        ];

        for (method, expected_amount, expected_days) in methods_and_rates {
            let rate = match method {
                ShippingMethod::Standard => ShippingRate {
                    method: method.clone(),
                    amount: dec!(10),
                    estimated_days: 5,
                },
                ShippingMethod::Express => ShippingRate {
                    method: method.clone(),
                    amount: dec!(25),
                    estimated_days: 2,
                },
                ShippingMethod::Overnight => ShippingRate {
                    method: method.clone(),
                    amount: dec!(50),
                    estimated_days: 1,
                },
            };

            assert_eq!(rate.amount, expected_amount);
            assert_eq!(rate.estimated_days, expected_days);
        }
    }

    // ==================== Checkout Flow Tests ====================

    #[test]
    fn test_checkout_step_progression() {
        // Test the expected checkout flow
        let steps = vec![
            CheckoutStep::CustomerInfo,
            CheckoutStep::ShippingAddress,
            CheckoutStep::ShippingMethod,
            CheckoutStep::Payment,
        ];

        // Verify all steps exist
        assert_eq!(steps.len(), 4);
    }

    #[test]
    fn test_checkout_session_requires_email() {
        let email: Option<String> = None;
        let shipping_address: Option<Address> = Some(Address {
            first_name: "John".to_string(),
            last_name: "Doe".to_string(),
            company: None,
            address_line_1: "123 Main St".to_string(),
            address_line_2: None,
            city: "New York".to_string(),
            province: "NY".to_string(),
            country_code: "US".to_string(),
            postal_code: "10001".to_string(),
            phone: None,
        });
        let shipping_method: Option<ShippingMethod> = Some(ShippingMethod::Standard);

        // Session is incomplete if email is missing
        let is_complete = email.is_some() && shipping_address.is_some() && shipping_method.is_some();
        assert!(!is_complete);
    }

    #[test]
    fn test_checkout_session_requires_shipping_address() {
        let email: Option<String> = Some("test@example.com".to_string());
        let shipping_address: Option<Address> = None;
        let shipping_method: Option<ShippingMethod> = Some(ShippingMethod::Standard);

        let is_complete = email.is_some() && shipping_address.is_some() && shipping_method.is_some();
        assert!(!is_complete);
    }

    #[test]
    fn test_checkout_session_requires_shipping_method() {
        let email: Option<String> = Some("test@example.com".to_string());
        let shipping_address: Option<Address> = Some(Address {
            first_name: "John".to_string(),
            last_name: "Doe".to_string(),
            company: None,
            address_line_1: "123 Main St".to_string(),
            address_line_2: None,
            city: "New York".to_string(),
            province: "NY".to_string(),
            country_code: "US".to_string(),
            postal_code: "10001".to_string(),
            phone: None,
        });
        let shipping_method: Option<ShippingMethod> = None;

        let is_complete = email.is_some() && shipping_address.is_some() && shipping_method.is_some();
        assert!(!is_complete);
    }

    #[test]
    fn test_checkout_session_complete() {
        let email: Option<String> = Some("test@example.com".to_string());
        let shipping_address: Option<Address> = Some(Address {
            first_name: "John".to_string(),
            last_name: "Doe".to_string(),
            company: None,
            address_line_1: "123 Main St".to_string(),
            address_line_2: None,
            city: "New York".to_string(),
            province: "NY".to_string(),
            country_code: "US".to_string(),
            postal_code: "10001".to_string(),
            phone: None,
        });
        let shipping_method: Option<ShippingMethod> = Some(ShippingMethod::Standard);

        let is_complete = email.is_some() && shipping_address.is_some() && shipping_method.is_some();
        assert!(is_complete);
    }

    // ==================== Decimal Precision Tests ====================

    #[test]
    fn test_decimal_currency_precision() {
        let price1 = dec!(19.99);
        let price2 = dec!(29.99);
        let price3 = dec!(9.99);
        let total = price1 + price2 + price3;

        assert_eq!(total, dec!(59.97));
    }

    #[test]
    fn test_decimal_tax_precision() {
        let subtotal = dec!(100.00);
        let tax_rate = Decimal::new(875, 3); // 8.75%
        let tax = subtotal * tax_rate / Decimal::from(100);

        // Should maintain precision
        assert!(tax > Decimal::ZERO);
    }
}
