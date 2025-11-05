use crate::{
    cache::InMemoryCache,
    errors::ServiceError,
    events::{Event, EventSender},
    metrics::{
        self, CHECKOUT_CANCELLATIONS, CHECKOUT_COMPLETIONS, CHECKOUT_SESSIONS_CREATED,
        CHECKOUT_SESSIONS_UPDATED, ORDERS_CREATED, PAYMENT_PROCESSING_FAILURE,
        PAYMENT_PROCESSING_SUCCESS, VAULT_TOKENS_CONSUMED,
    },
    models::{
        Address, CheckoutSession, CheckoutSessionCompleteRequest, CheckoutSessionCreateRequest,
        CheckoutSessionStatus, CheckoutSessionUpdateRequest, CheckoutSessionWithOrder, Customer,
        EstimatedDelivery, FulfillmentChoice, FulfillmentState, LineItem, Links, Message,
        MessageType, Money, Order, OrderStatus, PaymentRequest, RequestItem, Totals,
    },
    product_catalog::ProductCatalogService,
    shopify_integration::ShopifyClient,
    stripe_integration::StripePaymentProcessor,
    tax_service::TaxService,
    validation::{validate_country_code, validate_email, validate_phone, validate_quantity},
};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, instrument, warn};
use uuid::Uuid;

const SESSION_TTL_SECS: u64 = 3600;
const DEFAULT_CURRENCY: &str = "usd";
const FALLBACK_TAX_BPS: i64 = 875; // 8.75%

/// Agentic checkout service for ChatGPT-driven checkout flow
#[derive(Clone)]
pub struct AgenticCheckoutService {
    cache: Arc<InMemoryCache>,
    event_sender: Arc<EventSender>,
    product_catalog: Arc<ProductCatalogService>,
    tax_service: Arc<TaxService>,
    stripe_processor: Option<Arc<StripePaymentProcessor>>,
    shopify_client: Option<Arc<ShopifyClient>>,
}

impl AgenticCheckoutService {
    pub fn new(
        cache: Arc<InMemoryCache>,
        event_sender: Arc<EventSender>,
        product_catalog: Arc<ProductCatalogService>,
        tax_service: Arc<TaxService>,
        stripe_processor: Option<Arc<StripePaymentProcessor>>,
        shopify_client: Option<Arc<ShopifyClient>>,
    ) -> Self {
        Self {
            cache,
            event_sender,
            product_catalog,
            tax_service,
            stripe_processor,
            shopify_client,
        }
    }

    /// Create checkout session
    #[instrument(skip(self))]
    pub async fn create_session(
        &self,
        request: CheckoutSessionCreateRequest,
    ) -> Result<CheckoutSession, ServiceError> {
        if request.items.is_empty() {
            return Err(ServiceError::InvalidInput(
                "At least one line item is required".to_string(),
            ));
        }

        if let Some(customer) = request.customer.as_ref() {
            self.validate_customer(customer)?;
        }

        if let Some(shopify) = &self.shopify_client {
            let session = shopify.create_session(&request).await?;
            CHECKOUT_SESSIONS_CREATED.inc();
            metrics::ACTIVE_SESSIONS.inc();
            if let Ok(uuid) = Uuid::parse_str(&session.id) {
                self.event_sender
                    .send(Event::CheckoutStarted { session_id: uuid })
                    .await;
            } else {
                warn!(
                    "Shopify session ID {} is not a valid UUID; skipping checkout started event",
                    session.id
                );
            }
            info!("Created Shopify-backed checkout session {}", session.id);
            return Ok(session);
        }

        let line_items = self.build_line_items(&request.items).await?;

        let requested_selection = request
            .fulfillment
            .as_ref()
            .and_then(|f| f.selected_id.as_deref());
        let fulfillment_state =
            self.resolve_fulfillment_state(request.customer.as_ref(), requested_selection)?;

        let totals = self.calculate_totals(
            &line_items,
            request.customer.as_ref(),
            fulfillment_state.as_ref(),
        )?;
        let status = self.determine_status(
            request.customer.as_ref(),
            fulfillment_state.as_ref(),
            &line_items,
        );

        let session_id = Uuid::new_v4().to_string();
        self.reserve_line_items(&session_id, &line_items)?;
        let created_at = Utc::now().to_rfc3339();

        let session = CheckoutSession {
            id: session_id.clone(),
            status,
            items: line_items,
            totals,
            fulfillment: fulfillment_state,
            customer: request.customer,
            links: Some(Links {
                terms: Some("https://merchant.example.com/terms".to_string()),
                privacy: Some("https://merchant.example.com/privacy".to_string()),
                order_permalink: None,
            }),
            messages: None,
            created_at: created_at.clone(),
            updated_at: created_at,
        };

        if let Err(err) = self.save_session(&session).await {
            self.product_catalog.release_reservation(&session.id);
            return Err(err);
        }
        CHECKOUT_SESSIONS_CREATED.inc();
        metrics::ACTIVE_SESSIONS.inc();

        if let Ok(uuid) = Uuid::parse_str(&session.id) {
            self.event_sender
                .send(Event::CheckoutStarted { session_id: uuid })
                .await;
        } else {
            warn!(
                "Checkout session ID {} is not a valid UUID; skipping checkout started event",
                session.id
            );
        }

        info!("Created checkout session: {}", session.id);
        Ok(session)
    }

    /// Get checkout session
    #[instrument(skip(self))]
    pub async fn get_session(&self, session_id: &str) -> Result<CheckoutSession, ServiceError> {
        if let Some(shopify) = &self.shopify_client {
            return shopify.get_session(session_id).await;
        }

        let cache_key = format!("checkout_session:{}", session_id);
        let cached = self
            .cache
            .get(&cache_key)
            .await
            .map_err(|e| ServiceError::CacheError(e.to_string()))?;

        match cached {
            Some(raw) => {
                serde_json::from_str(&raw).map_err(|e| ServiceError::ParseError(e.to_string()))
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
        if let Some(customer) = request.customer.as_ref() {
            self.validate_customer(customer)?;
        }

        if let Some(shopify) = &self.shopify_client {
            let session = shopify.update_session(session_id, &request).await?;
            CHECKOUT_SESSIONS_UPDATED.inc();
            info!("Updated Shopify checkout session: {}", session.id);
            return Ok(session);
        }

        let mut session = self.get_session(session_id).await?;

        if matches!(
            session.status,
            CheckoutSessionStatus::Completed | CheckoutSessionStatus::Canceled
        ) {
            return Err(ServiceError::InvalidOperation(
                "Cannot update completed or canceled session".to_string(),
            ));
        }

        let mut previous_items: Option<Vec<LineItem>> = None;

        if let Some(items) = request.items.as_ref() {
            let new_line_items = self.build_line_items(items).await?;
            previous_items = Some(session.items.clone());
            self.product_catalog.release_reservation(&session.id);
            if let Err(err) = self.reserve_line_items(&session.id, &new_line_items) {
                // best-effort attempt to re-reserve original items
                let _ = self.reserve_line_items(&session.id, &session.items);
                return Err(err);
            }
            session.items = new_line_items;
        }

        if let Some(customer) = request.customer.as_ref() {
            session.customer = Some(customer.clone());
        }

        let requested_selection = request
            .fulfillment
            .as_ref()
            .and_then(|f| f.selected_id.as_deref());

        let selection_to_apply = requested_selection.or_else(|| {
            session
                .fulfillment
                .as_ref()
                .and_then(|f| f.selected_id.as_deref())
        });

        session.fulfillment =
            self.resolve_fulfillment_state(session.customer.as_ref(), selection_to_apply)?;
        session.totals = self.calculate_totals(
            &session.items,
            session.customer.as_ref(),
            session.fulfillment.as_ref(),
        )?;
        session.status = self.determine_status(
            session.customer.as_ref(),
            session.fulfillment.as_ref(),
            &session.items,
        );
        session.updated_at = Utc::now().to_rfc3339();

        if let Err(err) = self.save_session(&session).await {
            self.product_catalog.release_reservation(&session.id);
            if let Some(original) = previous_items {
                let _ = self.reserve_line_items(&session.id, &original);
            }
            return Err(err);
        }
        CHECKOUT_SESSIONS_UPDATED.inc();
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
        if let Some(customer) = request.customer.as_ref() {
            self.validate_customer(customer)?;
        }

        if let Some(shopify) = &self.shopify_client {
            let result = shopify.complete_session(session_id, &request).await?;
            CHECKOUT_COMPLETIONS.inc();
            ORDERS_CREATED.inc();
            metrics::ACTIVE_SESSIONS.dec();

            if let (Ok(session_uuid), Ok(order_uuid)) = (
                Uuid::parse_str(&result.session.id),
                Uuid::parse_str(&result.order.id),
            ) {
                self.event_sender
                    .send(Event::CheckoutCompleted {
                        session_id: session_uuid,
                        order_id: order_uuid,
                    })
                    .await;
            } else {
                warn!(
                    "Shopify completion produced non-UUID identifiers (session={}, order={}); skipping event emission",
                    result.session.id, result.order.id
                );
            }

            info!("Completed Shopify checkout session: {}", result.session.id);
            return Ok(result);
        }

        let mut session = self.get_session(session_id).await?;

        if matches!(session.status, CheckoutSessionStatus::Completed) {
            return Err(ServiceError::InvalidOperation(
                "Session already completed".to_string(),
            ));
        }
        if matches!(session.status, CheckoutSessionStatus::Canceled) {
            return Err(ServiceError::InvalidOperation(
                "Session is canceled".to_string(),
            ));
        }
        if !matches!(session.status, CheckoutSessionStatus::ReadyForPayment) {
            return Err(ServiceError::InvalidOperation(
                "Cannot complete session that is not ready for payment".to_string(),
            ));
        }

        if let Some(customer) = request.customer.as_ref() {
            session.customer = Some(customer.clone());
        }

        if let Some(fulfillment) = request.fulfillment.as_ref() {
            if let Some(selection) = fulfillment.selected_id.as_ref() {
                if !session
                    .fulfillment
                    .as_ref()
                    .and_then(|f| f.options.as_ref())
                    .map(|opts| opts.iter().any(|opt| &opt.id == selection))
                    .unwrap_or(false)
                {
                    return Err(ServiceError::InvalidInput(
                        "Invalid fulfillment option".to_string(),
                    ));
                }
                if let Some(ref mut existing) = session.fulfillment {
                    existing.selected_id = Some(selection.clone());
                }
            }
        }

        session.totals = self.calculate_totals(
            &session.items,
            session.customer.as_ref(),
            session.fulfillment.as_ref(),
        )?;

        let payment_result = match self.process_payment(&request.payment, &session).await {
            Ok(result) => {
                PAYMENT_PROCESSING_SUCCESS.inc();
                info!(
                    "Payment completed for session {} (payment_id={})",
                    session.id, result.payment_id
                );
                result
            }
            Err(err) => {
                PAYMENT_PROCESSING_FAILURE.inc();
                return Err(err);
            }
        };

        let message = Message {
            message_type: MessageType::Info,
            code: None,
            message: format!("Payment {} succeeded", payment_result.payment_id),
            param: None,
        };

        session.messages.get_or_insert_with(Vec::new).push(message);

        let order = self.create_order_from_session(&session).await?;

        if let Some(ref mut links) = session.links {
            links.order_permalink = Some(order.permalink_url.clone().unwrap_or_default());
        }

        self.product_catalog
            .commit_inventory(&session.id)
            .map_err(|err| {
                warn!(
                    "Failed to commit inventory for session {}: {}",
                    session.id, err
                );
                err
            })?;

        session.status = CheckoutSessionStatus::Completed;
        session.updated_at = Utc::now().to_rfc3339();

        self.save_session(&session).await?;
        CHECKOUT_COMPLETIONS.inc();
        ORDERS_CREATED.inc();

        if let (Ok(session_uuid), Ok(order_uuid)) =
            (Uuid::parse_str(&session.id), Uuid::parse_str(&order.id))
        {
            self.event_sender
                .send(Event::CheckoutCompleted {
                    session_id: session_uuid,
                    order_id: order_uuid,
                })
                .await;
        } else {
            warn!(
                "Completion produced non-UUID identifiers (session={}, order={}); skipping event emission",
                session.id, order.id
            );
        }

        metrics::ACTIVE_SESSIONS.dec();

        info!("Completed checkout session: {}", session.id);

        Ok(CheckoutSessionWithOrder { session, order })
    }

    /// Cancel checkout session
    #[instrument(skip(self))]
    pub async fn cancel_session(&self, session_id: &str) -> Result<CheckoutSession, ServiceError> {
        if self.shopify_client.is_some() {
            return Err(ServiceError::InvalidOperation(
                "Canceling Shopify-backed checkouts is not supported".to_string(),
            ));
        }

        let mut session = self.get_session(session_id).await?;

        if matches!(session.status, CheckoutSessionStatus::Completed) {
            return Err(ServiceError::InvalidOperation(
                "Cannot cancel completed session".to_string(),
            ));
        }
        if matches!(session.status, CheckoutSessionStatus::Canceled) {
            return Err(ServiceError::InvalidOperation(
                "Session already canceled".to_string(),
            ));
        }

        session.status = CheckoutSessionStatus::Canceled;
        session.updated_at = Utc::now().to_rfc3339();

        self.product_catalog.release_reservation(&session.id);
        self.save_session(&session).await?;

        CHECKOUT_CANCELLATIONS.inc();
        metrics::ACTIVE_SESSIONS.dec();
        info!("Canceled checkout session: {}", session.id);
        Ok(session)
    }

    // ---------------------------------------------------------------------
    // Private helper methods
    // ---------------------------------------------------------------------

    async fn build_line_items(&self, items: &[RequestItem]) -> Result<Vec<LineItem>, ServiceError> {
        let mut line_items = Vec::with_capacity(items.len());

        for request_item in items {
            validate_quantity(request_item.quantity)?;

            let product = self.product_catalog.get_product(&request_item.id)?;

            if !self
                .product_catalog
                .check_inventory(&product.id, request_item.quantity)?
            {
                return Err(ServiceError::InsufficientStock(format!(
                    "Insufficient stock for product: {}",
                    product.name
                )));
            }

            line_items.push(LineItem {
                id: Uuid::new_v4().to_string(),
                title: product.name.clone(),
                quantity: request_item.quantity,
                unit_price: Money {
                    amount: product.price,
                    currency: product.currency.clone(),
                },
                variant_id: Some(product.id.clone()),
                sku: product.metadata.get("sku").cloned(),
                image_url: product.image_url.clone(),
            });
        }

        Ok(line_items)
    }

    fn calculate_totals(
        &self,
        items: &[LineItem],
        customer: Option<&Customer>,
        fulfillment: Option<&FulfillmentState>,
    ) -> Result<Totals, ServiceError> {
        let subtotal_amount = items
            .iter()
            .map(|item| item.unit_price.amount * item.quantity as i64)
            .sum::<i64>();

        let currency = items
            .first()
            .map(|item| item.unit_price.currency.clone())
            .unwrap_or_else(|| DEFAULT_CURRENCY.to_string());

        let shipping_choice = fulfillment.and_then(|state| {
            let selected = state.selected_id.as_ref()?;
            state
                .options
                .as_ref()
                .and_then(|opts| opts.iter().find(|opt| &opt.id == selected).cloned())
        });

        let shipping_amount = shipping_choice
            .as_ref()
            .map(|choice| choice.price.amount)
            .unwrap_or(0);
        let shipping_money = shipping_choice.map(|choice| Money {
            amount: choice.price.amount,
            currency: currency.clone(),
        });

        let tax_amount = if let Some(address) = customer.and_then(|c| c.shipping_address.as_ref()) {
            let include_shipping = shipping_amount > 0;
            self.tax_service
                .calculate_tax(subtotal_amount, address, include_shipping, shipping_amount)?
                .tax_amount
        } else {
            (subtotal_amount * FALLBACK_TAX_BPS) / 10_000
        };

        let grand_total_amount = subtotal_amount + shipping_amount + tax_amount;

        Ok(Totals {
            subtotal: Money {
                amount: subtotal_amount,
                currency: currency.clone(),
            },
            tax: if tax_amount > 0 {
                Some(Money {
                    amount: tax_amount,
                    currency: currency.clone(),
                })
            } else {
                None
            },
            shipping: shipping_money.map(|money| Money {
                amount: money.amount,
                currency: currency.clone(),
            }),
            discount: None,
            grand_total: Money {
                amount: grand_total_amount,
                currency,
            },
        })
    }

    fn determine_status(
        &self,
        customer: Option<&Customer>,
        fulfillment: Option<&FulfillmentState>,
        items: &[LineItem],
    ) -> CheckoutSessionStatus {
        if items.is_empty() {
            return CheckoutSessionStatus::NotReadyForPayment;
        }

        let shipping_ready = customer.and_then(|c| c.shipping_address.as_ref()).is_some();
        let selection_ready = fulfillment.and_then(|f| f.selected_id.as_ref()).is_some();

        if shipping_ready && selection_ready {
            CheckoutSessionStatus::ReadyForPayment
        } else {
            CheckoutSessionStatus::NotReadyForPayment
        }
    }

    fn resolve_fulfillment_state(
        &self,
        customer: Option<&Customer>,
        selected_id: Option<&str>,
    ) -> Result<Option<FulfillmentState>, ServiceError> {
        let options = self.get_fulfillment_options(customer)?;

        if options.is_empty() {
            return Ok(None);
        }

        let validated_selected = if let Some(id) = selected_id {
            if options.iter().any(|opt| opt.id == id) {
                Some(id.to_string())
            } else {
                return Err(ServiceError::InvalidInput(
                    "Invalid fulfillment option".to_string(),
                ));
            }
        } else {
            None
        };

        Ok(Some(FulfillmentState {
            selected_id: validated_selected,
            options: Some(options),
        }))
    }

    fn get_fulfillment_options(
        &self,
        customer: Option<&Customer>,
    ) -> Result<Vec<FulfillmentChoice>, ServiceError> {
        let shipping_address = customer.and_then(|c| c.shipping_address.as_ref());
        if shipping_address.is_none() {
            return Ok(vec![]);
        }

        Ok(vec![
            FulfillmentChoice {
                id: "standard_shipping".to_string(),
                label: "Standard Shipping".to_string(),
                price: Money {
                    amount: 1000,
                    currency: DEFAULT_CURRENCY.to_string(),
                },
                est_delivery: Some(EstimatedDelivery {
                    earliest: Some((Utc::now() + ChronoDuration::days(5)).to_rfc3339()),
                    latest: Some((Utc::now() + ChronoDuration::days(7)).to_rfc3339()),
                }),
            },
            FulfillmentChoice {
                id: "express_shipping".to_string(),
                label: "Express Shipping".to_string(),
                price: Money {
                    amount: 2500,
                    currency: DEFAULT_CURRENCY.to_string(),
                },
                est_delivery: Some(EstimatedDelivery {
                    earliest: Some((Utc::now() + ChronoDuration::days(2)).to_rfc3339()),
                    latest: Some((Utc::now() + ChronoDuration::days(3)).to_rfc3339()),
                }),
            },
        ])
    }

    fn validate_customer(&self, customer: &Customer) -> Result<(), ServiceError> {
        if let Some(address) = customer.billing_address.as_ref() {
            self.validate_address(address)?;
        }
        if let Some(address) = customer.shipping_address.as_ref() {
            self.validate_address(address)?;
        }
        Ok(())
    }

    fn validate_address(&self, address: &Address) -> Result<(), ServiceError> {
        if address.line1.trim().is_empty() {
            return Err(ServiceError::InvalidInput(
                "Address line1 is required".to_string(),
            ));
        }
        if address.city.trim().is_empty() {
            return Err(ServiceError::InvalidInput("City is required".to_string()));
        }
        if address.postal_code.trim().is_empty() {
            return Err(ServiceError::InvalidInput(
                "Postal code is required".to_string(),
            ));
        }

        validate_country_code(&address.country)?;
        if let Some(phone) = address.phone.as_ref() {
            validate_phone(phone)?;
        }
        if let Some(email) = address.email.as_ref() {
            validate_email(email)?;
        }

        Ok(())
    }

    async fn process_payment(
        &self,
        payment: &PaymentRequest,
        session: &CheckoutSession,
    ) -> Result<PaymentResult, ServiceError> {
        let total_amount = session.totals.grand_total.amount;

        if let Some(token) = payment.delegated_token.as_deref() {
            if token.starts_with("vt_") {
                return self.process_vault_token(token, session, total_amount).await;
            }
            if token.starts_with("spt_") {
                return self
                    .process_stripe_shared_token(token, session, total_amount)
                    .await;
            }
            return self
                .process_stripe_regular(token, session, total_amount)
                .await;
        }

        if let Some(method) = payment.method.as_deref() {
            return self
                .process_stripe_regular(method, session, total_amount)
                .await;
        }

        Err(ServiceError::InvalidInput(
            "payment method or delegated_token required".to_string(),
        ))
    }

    async fn process_vault_token(
        &self,
        token: &str,
        session: &CheckoutSession,
        total_amount: i64,
    ) -> Result<PaymentResult, ServiceError> {
        info!("Processing vault token");
        let cache_key = format!("vault_token:{}", token);
        let cached = self
            .cache
            .get(&cache_key)
            .await
            .map_err(|e| ServiceError::CacheError(e.to_string()))?;

        let data = cached.ok_or_else(|| {
            ServiceError::InvalidOperation("Vault token not found or expired".to_string())
        })?;

        let token_data: serde_json::Value =
            serde_json::from_str(&data).map_err(|e| ServiceError::ParseError(e.to_string()))?;

        let allowance = token_data.get("allowance").ok_or_else(|| {
            ServiceError::InvalidInput("Vault token missing allowance".to_string())
        })?;

        let allowed_session = allowance
            .get("checkout_session_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ServiceError::InvalidInput("Vault token missing checkout_session_id".to_string())
            })?;

        if allowed_session != session.id {
            return Err(ServiceError::InvalidOperation(
                "Vault token does not match checkout session".to_string(),
            ));
        }

        let max_amount = allowance
            .get("max_amount")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| {
                ServiceError::InvalidInput("Vault token missing max_amount".to_string())
            })?;

        let currency = allowance
            .get("currency")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ServiceError::InvalidInput("Vault token missing currency".to_string())
            })?;

        if total_amount > max_amount {
            return Err(ServiceError::InvalidOperation(
                "Vault token allowance exceeded".to_string(),
            ));
        }

        if currency.to_lowercase() != session.totals.grand_total.currency {
            return Err(ServiceError::InvalidOperation(
                "Vault token currency mismatch".to_string(),
            ));
        }

        let expires_at = allowance
            .get("expires_at")
            .and_then(|v| v.as_str())
            .and_then(|v| DateTime::parse_from_rfc3339(v).ok());

        if let Some(expiration) = expires_at {
            if expiration < Utc::now() {
                return Err(ServiceError::InvalidOperation(
                    "Vault token allowance expired".to_string(),
                ));
            }
        }

        self.cache
            .delete(&cache_key)
            .await
            .map_err(|e| ServiceError::CacheError(e.to_string()))?;
        VAULT_TOKENS_CONSUMED.inc();

        Ok(PaymentResult {
            payment_id: format!("pay_vt_{}", &token[token.len().saturating_sub(6)..]),
            status: "authorized".to_string(),
            amount: total_amount,
        })
    }

    async fn process_stripe_shared_token(
        &self,
        token: &str,
        session: &CheckoutSession,
        total_amount: i64,
    ) -> Result<PaymentResult, ServiceError> {
        info!("Processing Stripe SharedPaymentToken");

        if let Some(processor) = &self.stripe_processor {
            let granted_token = processor.get_granted_token(token).await?;
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

            let intent = processor
                .process_shared_payment_token(
                    token,
                    total_amount,
                    session.totals.grand_total.currency.as_str(),
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

        Ok(PaymentResult {
            payment_id: format!("pi_spt_{}", Uuid::new_v4()),
            status: "succeeded".to_string(),
            amount: total_amount,
        })
    }

    async fn process_stripe_regular(
        &self,
        token: &str,
        _session: &CheckoutSession,
        total_amount: i64,
    ) -> Result<PaymentResult, ServiceError> {
        info!("Processing payment method {}", mask_payment_token(token));
        Ok(PaymentResult {
            payment_id: format!("pi_{}", Uuid::new_v4()),
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
            status: OrderStatus::Placed,
            permalink_url: Some(format!("https://merchant.example.com/orders/{}", order_id)),
        })
    }

    async fn save_session(&self, session: &CheckoutSession) -> Result<(), ServiceError> {
        let cache_key = format!("checkout_session:{}", session.id);
        let data = serde_json::to_string(session)
            .map_err(|e| ServiceError::SerializationError(e.to_string()))?;

        self.cache
            .set(
                &cache_key,
                &data,
                Some(Duration::from_secs(SESSION_TTL_SECS)),
            )
            .await
            .map_err(|e| ServiceError::CacheError(e.to_string()))
    }

    fn reserve_line_items(
        &self,
        session_id: &str,
        line_items: &[LineItem],
    ) -> Result<(), ServiceError> {
        for item in line_items {
            let product_id = item
                .variant_id
                .as_ref()
                .ok_or_else(|| ServiceError::InternalError("Missing variant id".to_string()))?;
            self.product_catalog
                .reserve_inventory(product_id, item.quantity, session_id)?;
        }
        Ok(())
    }
}

fn mask_payment_token(token: &str) -> String {
    if token.len() <= 4 {
        return "***".to_string();
    }
    let prefix: String = token.chars().take(4).collect();
    let suffix: String = token
        .chars()
        .rev()
        .take(2)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{}***{}", prefix, suffix)
}

#[derive(Debug, Serialize)]
pub struct PaymentResult {
    pub payment_id: String,
    pub status: String,
    pub amount: i64,
}
