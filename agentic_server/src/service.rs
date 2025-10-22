use crate::{
    cache::InMemoryCache,
    errors::ServiceError,
    events::{Event, EventSender},
    metrics,
    models::*,
    product_catalog::ProductCatalogService,
    stripe_integration::StripePaymentProcessor,
    tax_service::TaxService,
    validation::{
        validate_country_code, validate_currency, validate_email, validate_phone, validate_quantity,
    },
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, instrument, warn};
use uuid::Uuid;

/// Agentic checkout service for ChatGPT-driven checkout flow
#[derive(Clone)]
pub struct AgenticCheckoutService {
    cache: Arc<InMemoryCache>,
    event_sender: Arc<EventSender>,
    product_catalog: Arc<ProductCatalogService>,
    tax_service: Arc<TaxService>,
    stripe_processor: Option<Arc<StripePaymentProcessor>>,
}

impl AgenticCheckoutService {
    pub fn new(
        cache: Arc<InMemoryCache>,
        event_sender: Arc<EventSender>,
        product_catalog: Arc<ProductCatalogService>,
        tax_service: Arc<TaxService>,
        stripe_processor: Option<Arc<StripePaymentProcessor>>,
    ) -> Self {
        Self {
            cache,
            event_sender,
            product_catalog,
            tax_service,
            stripe_processor,
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
        let currency = "usd".to_string();
        validate_currency(&currency)?;

        if let Some(ref buyer) = request.buyer {
            validate_email(&buyer.email)?;
            if let Some(ref phone) = buyer.phone_number {
                validate_phone(phone)?;
            }
        }

        if let Some(ref address) = request.fulfillment_address {
            validate_country_code(&address.country)?;
        }

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

        if let Err(err) = self.reserve_line_items(&session.id, &session.line_items) {
            self.product_catalog.release_reservation(&session.id);
            return Err(err);
        }

        // Store session in cache with 1 hour TTL
        if let Err(err) = self.save_session(&session).await {
            self.product_catalog.release_reservation(&session.id);
            return Err(err);
        }

        self.event_sender
            .send(Event::CheckoutStarted {
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
                    .map_err(|e| ServiceError::ParseError(e.to_string()))?;
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
        let mut previous_line_items: Option<Vec<LineItem>> = None;

        // Check if already completed or canceled
        if session.status == "completed" || session.status == "canceled" {
            return Err(ServiceError::InvalidOperation(
                "Cannot update completed or canceled session".to_string(),
            ));
        }

        let CheckoutSessionUpdateRequest {
            buyer,
            items,
            fulfillment_address,
            fulfillment_option_id,
        } = request;

        // Update fields if provided
        if let Some(buyer) = buyer {
            validate_email(&buyer.email)?;
            if let Some(ref phone) = buyer.phone_number {
                validate_phone(phone)?;
            }
            session.buyer = Some(buyer);
        }

        if let Some(items) = items {
            previous_line_items = Some(session.line_items.clone());
            session.line_items = self.build_line_items(&items).await?;

            self.product_catalog.release_reservation(&session.id);
            if let Err(err) = self.reserve_line_items(&session.id, &session.line_items) {
                if let Some(ref prior) = previous_line_items {
                    if let Err(reapply_err) = self.reserve_line_items(&session.id, prior) {
                        warn!(
                            "Failed to restore reservations for session {}: {}",
                            session.id, reapply_err
                        );
                    }
                }
                return Err(err);
            }
        }

        if let Some(address) = fulfillment_address {
            validate_country_code(&address.country)?;
            session.fulfillment_address = Some(address);
            // Recalculate fulfillment options
            session.fulfillment_options =
                self.get_fulfillment_options(session.fulfillment_address.as_ref())?;
        }

        if let Some(option_id) = fulfillment_option_id {
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
        if let Err(err) = self.save_session(&session).await {
            self.product_catalog.release_reservation(&session.id);
            if let Some(ref prior) = previous_line_items {
                if let Err(reapply_err) = self.reserve_line_items(&session.id, prior) {
                    warn!(
                        "Failed to restore reservations for session {} after save error: {}",
                        session.id, reapply_err
                    );
                }
            }
            return Err(err);
        }

        metrics::CHECKOUT_SESSIONS_UPDATED.inc();
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
        let CheckoutSessionCompleteRequest {
            buyer,
            payment_data,
        } = request;

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
        if let Some(buyer) = buyer {
            validate_email(&buyer.email)?;
            if let Some(ref phone) = buyer.phone_number {
                validate_phone(phone)?;
            }
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
        let payment_result = match self.process_payment(&payment_data, &session).await {
            Ok(result) => {
                metrics::PAYMENT_PROCESSING_SUCCESS.inc();
                info!(
                    "Payment completed for session {} (payment_id={})",
                    session.id, result.payment_id
                );
                result
            }
            Err(err) => {
                metrics::PAYMENT_PROCESSING_FAILURE.inc();
                return Err(err);
            }
        };

        session.messages.push(Message::Info(MessageInfo {
            message_type: "payment".to_string(),
            param: None,
            content_type: "text".to_string(),
            content: format!(
                "Payment {} succeeded via {}",
                payment_result.payment_id,
                payment_data.provider.as_str()
            ),
        }));

        // Create order
        let order = self.create_order_from_session(&session).await?;

        self.product_catalog
            .commit_inventory(&session.id)
            .map_err(|err| {
                warn!(
                    "Failed to commit inventory for session {}: {}",
                    session.id, err
                );
                err
            })?;

        // Update session status
        session.status = "completed".to_string();
        if let Err(err) = self.save_session(&session).await {
            warn!(
                "Failed to persist completed session {}, releasing reservations",
                session.id
            );
            self.product_catalog.release_reservation(&session.id);
            return Err(err);
        }

        self.event_sender
            .send(Event::CheckoutCompleted {
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

        self.product_catalog.release_reservation(&session.id);
        session.status = "canceled".to_string();
        if let Err(err) = self.save_session(&session).await {
            warn!(
                "Failed to persist canceled session {}; reservations already released",
                session.id
            );
            return Err(err);
        }

        metrics::CHECKOUT_CANCELLATIONS.inc();
        info!("Canceled checkout session: {}", session.id);
        Ok(session)
    }

    // Private helper methods

    async fn build_line_items(&self, items: &[Item]) -> Result<Vec<LineItem>, ServiceError> {
        let mut line_items = Vec::new();

        for item in items {
            validate_quantity(item.quantity)?;

            // Fetch real product details from catalog
            let product = self.product_catalog.get_product(&item.id)?;

            // Check inventory
            if !self
                .product_catalog
                .check_inventory(&item.id, item.quantity)?
            {
                return Err(ServiceError::InsufficientStock(format!(
                    "Insufficient stock for product: {}",
                    product.name
                )));
            }

            // Calculate pricing
            let base_amount = product.price * item.quantity as i64;
            let discount = 0; // TODO: Apply discounts/promotions
            let subtotal = base_amount - discount;

            // Tax will be calculated separately at checkout level
            let tax = 0; // Placeholder, calculated in calculate_totals
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
            1000 // $10.00 shipping (TODO: Real-time rates)
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

        // Tax - Calculate using tax service if address available
        let tax = if let Some(addr) = address {
            let tax_calc = self.tax_service.calculate_tax(
                subtotal,
                addr,
                true, // Include shipping in taxable amount
                fulfillment_cost,
            )?;
            tax_calc.tax_amount
        } else {
            // No address, use simple calculation
            subtotal * 875 / 10000 // 8.75% default
        };

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
        fulfillment_option_id: Option<&str>,
    ) -> String {
        // Check if ready for payment
        if request.buyer.is_some()
            && request.fulfillment_address.is_some()
            && !_line_items.is_empty()
            && fulfillment_option_id.is_some()
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

    async fn process_payment(
        &self,
        payment_data: &PaymentData,
        session: &CheckoutSession,
    ) -> Result<PaymentResult, ServiceError> {
        let token_preview = Self::mask_payment_token(&payment_data.token);
        info!(
            "Processing payment with provider: {} (token_preview={})",
            payment_data.provider, token_preview
        );

        // Determine payment method type
        if payment_data.token.starts_with("vt_") {
            // Our delegated payment vault token (mock PSP)
            self.process_vault_token(payment_data, session).await
        } else if payment_data.token.starts_with("spt_") {
            // Stripe SharedPaymentToken
            self.process_stripe_shared_token(payment_data, session)
                .await
        } else {
            // Regular Stripe payment token or PaymentMethod ID
            self.process_stripe_regular(payment_data, session).await
        }
    }

    async fn process_vault_token(
        &self,
        payment_data: &PaymentData,
        session: &CheckoutSession,
    ) -> Result<PaymentResult, ServiceError> {
        info!("Processing vault token (mock PSP)");

        // Validate vault token with allowances
        let cache_key = format!("vault_token:{}", payment_data.token);
        let cached = self
            .cache
            .get(&cache_key)
            .await
            .map_err(|e| ServiceError::CacheError(e.to_string()))?;

        match cached {
            Some(data) => {
                let token_data: serde_json::Value = serde_json::from_str(&data)
                    .map_err(|e| ServiceError::ParseError(e.to_string()))?;

                // Validate checkout session ID
                if let Some(allowance) = token_data.get("allowance") {
                    if allowance
                        .get("checkout_session_id")
                        .and_then(|v| v.as_str())
                        != Some(&session.id)
                    {
                        return Err(ServiceError::InvalidOperation(
                            "Vault token is not valid for this checkout session".to_string(),
                        ));
                    }
                }

                // Consume the token (single-use enforcement)
                self.cache
                    .delete(&cache_key)
                    .await
                    .map_err(|e| ServiceError::CacheError(e.to_string()))?;

                info!("Vault token consumed (single-use enforcement)");
                metrics::VAULT_TOKENS_CONSUMED.inc();

                Ok(PaymentResult {
                    payment_id: format!("pay_mock_{}", uuid::Uuid::new_v4()),
                    status: "succeeded".to_string(),
                    amount: session
                        .totals
                        .iter()
                        .find(|t| t.total_type == "total")
                        .map(|t| t.amount)
                        .unwrap_or(0),
                })
            }
            None => {
                metrics::VAULT_TOKEN_REUSE_BLOCKED.inc();
                Err(ServiceError::InvalidOperation(
                    "Vault token not found or already used".to_string(),
                ))
            }
        }
    }

    async fn process_stripe_shared_token(
        &self,
        payment_data: &PaymentData,
        session: &CheckoutSession,
    ) -> Result<PaymentResult, ServiceError> {
        let total_amount = session
            .totals
            .iter()
            .find(|t| t.total_type == "total")
            .map(|t| t.amount)
            .unwrap_or(0);

        if let Some(processor) = &self.stripe_processor {
            info!("Processing Stripe SharedPaymentToken via Stripe API");

            let granted_token = processor
                .get_granted_token(payment_data.token.as_str())
                .await?;
            let risk = processor.assess_risk(&granted_token);

            if !risk.warnings.is_empty() {
                warn!(
                    "Stripe risk warnings for session {}: {}",
                    session.id,
                    risk.warnings.join(", ")
                );
            }

            if risk.should_block {
                return Err(ServiceError::PaymentFailed(
                    "Stripe risk assessment blocked payment".to_string(),
                ));
            }

            let mut metadata = HashMap::new();
            metadata.insert("checkout_session_id".to_string(), session.id.clone());
            metadata.insert(
                "payment_provider".to_string(),
                payment_data.provider.clone(),
            );
            if let Some(buyer) = &session.buyer {
                metadata.insert("buyer_email".to_string(), buyer.email.clone());
            }

            let intent = processor
                .process_shared_payment_token(
                    payment_data.token.as_str(),
                    total_amount,
                    session.currency.as_str(),
                    metadata,
                )
                .await?;

            let final_intent = if intent.status == "requires_capture" {
                processor.capture_payment(&intent.id).await?
            } else {
                intent
            };

            return Ok(PaymentResult {
                payment_id: final_intent.id,
                status: final_intent.status,
                amount: final_intent.amount,
            });
        }

        info!(
            "Creating PaymentIntent with SharedPaymentToken preview {} for amount: {} (mock)",
            Self::mask_payment_token(&payment_data.token),
            total_amount
        );

        Ok(PaymentResult {
            payment_id: format!("pi_spt_{}", uuid::Uuid::new_v4()),
            status: "succeeded".to_string(),
            amount: total_amount,
        })
    }

    async fn process_stripe_regular(
        &self,
        payment_data: &PaymentData,
        session: &CheckoutSession,
    ) -> Result<PaymentResult, ServiceError> {
        info!(
            "Processing regular Stripe payment method (token_preview={})",
            Self::mask_payment_token(&payment_data.token)
        );

        let total_amount = session
            .totals
            .iter()
            .find(|t| t.total_type == "total")
            .map(|t| t.amount)
            .unwrap_or(0);

        // Mock regular Stripe payment
        Ok(PaymentResult {
            payment_id: format!("pi_{}", uuid::Uuid::new_v4()),
            status: "succeeded".to_string(),
            amount: total_amount,
        })
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

    fn reserve_line_items(
        &self,
        session_id: &str,
        line_items: &[LineItem],
    ) -> Result<(), ServiceError> {
        for line_item in line_items {
            self.product_catalog.reserve_inventory(
                &line_item.item.id,
                line_item.item.quantity,
                session_id,
            )?;
        }

        Ok(())
    }

    fn mask_payment_token(token: &str) -> String {
        if token.is_empty() {
            return "<empty>".to_string();
        }

        let chars: Vec<char> = token.chars().collect();

        if chars.len() <= 4 {
            return "***".to_string();
        }

        let prefix: String = chars.iter().take(4).collect();
        let suffix: String = chars
            .iter()
            .rev()
            .take(2)
            .cloned()
            .collect::<Vec<char>>()
            .into_iter()
            .rev()
            .collect();

        format!("{}***{}", prefix, suffix)
    }
}

#[derive(Debug, Serialize)]
pub struct PaymentResult {
    pub payment_id: String,
    pub status: String,
    pub amount: i64,
}
