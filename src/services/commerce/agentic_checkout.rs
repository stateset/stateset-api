use crate::{
    cache::InMemoryCache,
    commands::shipments::create_shipment_command::CreateShipmentCommand,
    errors::ServiceError,
    events::{Event, EventSender},
    models::shipment,
    services::{
        cash_sale::CashSaleService,
        commerce::product_catalog_service::ProductCatalogService,
        invoicing::InvoicingService,
        orders::{
            CreateOrderWithItemsInput, NewOrderItemInput, OrderResponse, OrderService,
            UpdateOrderStatusRequest,
        },
        payments::{PaymentMethod, PaymentService, ProcessPaymentRequest},
        promotions::PromotionService,
        shipments::ShipmentService,
    },
};
use chrono::{DateTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex as AsyncMutex;
use tracing::{info, instrument, warn};
use uuid::Uuid;
use validator::validate_email;

const SESSION_TTL_SECS: u64 = 3600;
const MAX_LINE_ITEMS: usize = 50;
const MAX_ITEM_QUANTITY: i32 = 99;
const MAX_ITEM_ID_LENGTH: usize = 128;
const MAX_NAME_LENGTH: usize = 120;
const MAX_ADDRESS_FIELD_LENGTH: usize = 120;
const MAX_EMAIL_LENGTH: usize = 254;
const MIN_IDEMPOTENCY_KEY_LENGTH: usize = 8;
const MAX_IDEMPOTENCY_KEY_LENGTH: usize = 255;

/// Finite State Machine for checkout session status transitions
/// Defines which status transitions are allowed
pub mod checkout_fsm {
    use lazy_static::lazy_static;
    use std::collections::HashMap;

    pub const NOT_READY_FOR_PAYMENT: &str = "not_ready_for_payment";
    pub const READY_FOR_PAYMENT: &str = "ready_for_payment";
    pub const COMPLETED: &str = "completed";
    pub const CANCELED: &str = "canceled";

    lazy_static! {
        /// Maps current status to allowed next statuses
        pub static ref ALLOWED_TRANSITIONS: HashMap<&'static str, Vec<&'static str>> = {
            let mut m = HashMap::new();
            m.insert(NOT_READY_FOR_PAYMENT, vec![READY_FOR_PAYMENT, CANCELED]);
            m.insert(READY_FOR_PAYMENT, vec![COMPLETED, CANCELED]);
            m.insert(COMPLETED, vec![]); // Terminal state
            m.insert(CANCELED, vec![]); // Terminal state
            m
        };
    }

    /// Check if a status transition is allowed
    pub fn can_transition(from: &str, to: &str) -> bool {
        ALLOWED_TRANSITIONS
            .get(from)
            .map(|allowed| allowed.contains(&to))
            .unwrap_or(false)
    }

    /// Check if a status is terminal (no further transitions allowed)
    pub fn is_terminal(status: &str) -> bool {
        matches!(status, COMPLETED | CANCELED)
    }
}

fn default_session_timestamp() -> DateTime<Utc> {
    Utc::now()
}

fn default_session_expiry() -> DateTime<Utc> {
    Utc::now() + chrono::Duration::seconds(SESSION_TTL_SECS as i64)
}

/// Configuration for session behavior
#[derive(Clone, Debug)]
pub struct SessionConfig {
    pub ttl_secs: u64,
    pub auto_extend_threshold: Option<f64>, // Extend when this fraction of TTL remains (0.0-1.0)
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            ttl_secs: SESSION_TTL_SECS,
            auto_extend_threshold: None, // Disabled by default
        }
    }
}

/// Agentic checkout service for ChatGPT-driven checkout flow
#[derive(Clone)]
pub struct AgenticCheckoutService {
    #[allow(dead_code)] // Reserved for direct database operations
    db: Arc<DatabaseConnection>,
    cache: Arc<InMemoryCache>,
    event_sender: Arc<EventSender>,
    product_catalog: Arc<ProductCatalogService>,
    order_service: Arc<OrderService>,
    payment_service: Arc<PaymentService>,
    promotion_service: Arc<PromotionService>,
    shipment_service: Arc<ShipmentService>,
    invoicing_service: Arc<InvoicingService>,
    cash_sale_service: Arc<CashSaleService>,
    config: SessionConfig,
    session_locks: Arc<AsyncMutex<HashMap<String, Arc<AsyncMutex<()>>>>>,
    idempotency_locks: Arc<AsyncMutex<HashMap<String, Arc<AsyncMutex<()>>>>>,
}

impl AgenticCheckoutService {
    pub fn new(
        db: Arc<DatabaseConnection>,
        cache: Arc<InMemoryCache>,
        event_sender: Arc<EventSender>,
        product_catalog: Arc<ProductCatalogService>,
        order_service: Arc<OrderService>,
        payment_service: Arc<PaymentService>,
        promotion_service: Arc<PromotionService>,
        shipment_service: Arc<ShipmentService>,
        invoicing_service: Arc<InvoicingService>,
        cash_sale_service: Arc<CashSaleService>,
    ) -> Self {
        Self::with_config(
            db,
            cache,
            event_sender,
            product_catalog,
            order_service,
            payment_service,
            promotion_service,
            shipment_service,
            invoicing_service,
            cash_sale_service,
            SessionConfig::default(),
        )
    }

    pub fn with_config(
        db: Arc<DatabaseConnection>,
        cache: Arc<InMemoryCache>,
        event_sender: Arc<EventSender>,
        product_catalog: Arc<ProductCatalogService>,
        order_service: Arc<OrderService>,
        payment_service: Arc<PaymentService>,
        promotion_service: Arc<PromotionService>,
        shipment_service: Arc<ShipmentService>,
        invoicing_service: Arc<InvoicingService>,
        cash_sale_service: Arc<CashSaleService>,
        config: SessionConfig,
    ) -> Self {
        Self {
            db,
            cache,
            event_sender,
            product_catalog,
            order_service,
            payment_service,
            promotion_service,
            shipment_service,
            invoicing_service,
            cash_sale_service,
            config,
            session_locks: Arc::new(AsyncMutex::new(HashMap::new())),
            idempotency_locks: Arc::new(AsyncMutex::new(HashMap::new())),
        }
    }

    fn session_ttl(&self) -> Duration {
        Duration::from_secs(self.config.ttl_secs)
    }

    /// Create checkout session
    #[instrument(skip(self, request), fields(items_count = request.items.len(), has_buyer = request.buyer.is_some(), idempotency_key = idempotency_key))]
    pub async fn create_session(
        &self,
        request: CheckoutSessionCreateRequest,
        idempotency_key: Option<&str>,
    ) -> Result<CreateSessionResult, ServiceError> {
        self.validate_create_request(&request)?;

        let hashed_idempotency = match idempotency_key {
            Some(key) => Some(self.hash_idempotency_key(key)?),
            None => None,
        };

        if let Some(ref hash) = hashed_idempotency {
            let idempotency_lock = self.acquire_idempotency_lock(hash).await;
            let guard = idempotency_lock.lock().await;
            let result = self
                .create_session_inner(request, hashed_idempotency.as_deref())
                .await;
            drop(guard);
            self.release_idempotency_lock(hash, idempotency_lock).await;
            result
        } else {
            self.create_session_inner(request, None).await
        }
    }

    async fn create_session_inner(
        &self,
        request: CheckoutSessionCreateRequest,
        hashed_idempotency: Option<&str>,
    ) -> Result<CreateSessionResult, ServiceError> {
        if let Some(hash) = hashed_idempotency {
            let cache_key = self.idempotency_cache_key(hash);
            if let Some(existing_session_id) = self
                .cache
                .get(&cache_key)
                .await
                .map_err(|e| ServiceError::CacheError(e.to_string()))?
            {
                match self.get_session(&existing_session_id).await {
                    Ok(session) => {
                        return Ok(CreateSessionResult {
                            session,
                            was_created: false,
                        });
                    }
                    Err(ServiceError::NotFound(_)) => {
                        self.cache
                            .delete(&cache_key)
                            .await
                            .map_err(|e| ServiceError::CacheError(e.to_string()))?;
                    }
                    Err(err) => return Err(err),
                }
            }
        }

        let line_items = self.build_line_items(&request.items).await?;

        // Validate and load promotion if provided
        let promotion = if let Some(ref promo_code) = request.promotion_code {
            match self
                .promotion_service
                .find_active_promotion(promo_code)
                .await
            {
                Ok(Some(promo)) => {
                    info!("Applied promotion: {} ({})", promo.name, promo_code);
                    Some(promo)
                }
                Ok(None) => {
                    warn!("Invalid or expired promotion code: {}", promo_code);
                    None
                }
                Err(e) => {
                    warn!("Error loading promotion {}: {}", promo_code, e);
                    None
                }
            }
        } else {
            None
        };

        let session_id = Uuid::new_v4();
        let currency = "USD".to_string();
        let now = Utc::now();
        let expires_at = now
            + chrono::Duration::from_std(self.session_ttl())
                .unwrap_or_else(|_| chrono::Duration::seconds(SESSION_TTL_SECS as i64));

        // Calculate totals with promotion applied
        let totals = self.calculate_totals_with_promotion(
            &line_items,
            request.fulfillment_address.as_ref(),
            None,
            promotion.as_ref(),
        )?;

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
            order_id: None,
            payment_id: None,
            invoice_id: None,
            cash_sale_id: None,
            shipment_id: None,
            status,
            currency,
            line_items,
            fulfillment_address: request.fulfillment_address,
            fulfillment_options,
            fulfillment_option_id: None,
            promotion_code: request.promotion_code,
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
            created_at: now,
            updated_at: None,
            expires_at,
            completed_at: None,
            canceled_at: None,
            payment_intent_id: None,
        };

        // Store session in cache with configured TTL
        self.save_session(&session).await?;
        if let Some(hash) = hashed_idempotency {
            self.cache
                .set(
                    &self.idempotency_cache_key(hash),
                    &session.id,
                    Some(self.session_ttl()),
                )
                .await
                .map_err(|e| ServiceError::CacheError(e.to_string()))?;
        }

        if let Ok(session_uuid) = Uuid::parse_str(&session.id) {
            self.event_sender
                .send_or_log(Event::CheckoutStarted {
                    cart_id: Uuid::nil(), // Not cart-based
                    session_id: session_uuid,
                })
                .await;
        }

        info!("Created checkout session: {}", session.id);
        Ok(CreateSessionResult {
            session,
            was_created: true,
        })
    }

    /// Get checkout session
    #[instrument(skip(self), fields(session_id = %session_id))]
    pub async fn get_session(&self, session_id: &str) -> Result<CheckoutSession, ServiceError> {
        let cache_key = format!("checkout_session:{}", session_id);

        let cached = self
            .cache
            .get(&cache_key)
            .await
            .map_err(|e| ServiceError::CacheError(e.to_string()))?;

        match cached {
            Some(data) => {
                let mut session: CheckoutSession = serde_json::from_str(&data)
                    .map_err(|e| ServiceError::SerializationError(e.to_string()))?;

                // Auto-extend session if configured
                if session.status != "completed" && session.status != "canceled" {
                    if let Some(threshold) = self.config.auto_extend_threshold {
                        if let Ok(ttl_duration) = chrono::Duration::from_std(self.session_ttl()) {
                            let now = Utc::now();
                            if session.expires_at > now {
                                let elapsed = now - session.created_at;
                                let total_ttl =
                                    (session.expires_at - session.created_at).num_seconds() as f64;
                                let elapsed_ratio = elapsed.num_seconds() as f64 / total_ttl;

                                // Extend if we've passed the threshold
                                if elapsed_ratio >= threshold {
                                    session.expires_at = now + ttl_duration;
                                    session.updated_at = Some(now);
                                    self.save_session(&session).await?;
                                }
                            }
                        }
                    }
                }

                Ok(session)
            }
            None => Err(ServiceError::NotFound(format!(
                "Checkout session {} not found",
                session_id
            ))),
        }
    }

    /// Update checkout session
    #[instrument(skip(self, request), fields(session_id = %session_id, has_buyer = request.buyer.is_some(), has_items = request.items.is_some()))]
    pub async fn update_session(
        &self,
        session_id: &str,
        request: CheckoutSessionUpdateRequest,
    ) -> Result<CheckoutSession, ServiceError> {
        let session_lock = self.acquire_session_lock(session_id).await;
        let guard = session_lock.lock().await;

        let result = {
            let request = request;
            async move {
                let mut session = self.get_session(session_id).await?;

                Self::ensure_session_open(&session)?;
                self.validate_update_request(&request)?;

                if let Some(ref buyer) = request.buyer {
                    Self::validate_buyer(buyer)?;
                    session.buyer = Some(buyer.clone());
                }

                if let Some(items) = request.items {
                    Self::validate_items(&items)?;
                    session.line_items = self.build_line_items(&items).await?;
                }

                if let Some(address) = request.fulfillment_address {
                    Self::validate_address(&address)?;
                    let address_changed = session
                        .fulfillment_address
                        .as_ref()
                        .map(|existing| existing != &address)
                        .unwrap_or(true);
                    session.fulfillment_address = Some(address);
                    session.fulfillment_options =
                        self.get_fulfillment_options(session.fulfillment_address.as_ref())?;
                    if address_changed {
                        session.fulfillment_option_id = None;
                    }
                }

                if let Some(option_id) = request.fulfillment_option_id {
                    Self::validate_fulfillment_selection(&session, &option_id)?;
                    session.fulfillment_option_id = Some(option_id);
                }

                session.totals = self.calculate_totals(
                    &session.line_items,
                    session.fulfillment_address.as_ref(),
                    session.fulfillment_option_id.as_deref(),
                )?;

                session.status = self.determine_status_from_session(&session);
                let now = Utc::now();
                session.updated_at = Some(now);
                session.expires_at = now
                    + chrono::Duration::from_std(self.session_ttl())
                        .unwrap_or_else(|_| chrono::Duration::seconds(SESSION_TTL_SECS as i64));

                self.save_session(&session).await?;

                Ok(session)
            }
        }
        .await;

        drop(guard);
        self.release_session_lock(session_id, session_lock).await;

        if let Ok(ref session) = result {
            info!("Updated checkout session: {}", session.id);
        }

        result
    }

    /// Complete checkout session
    #[instrument(skip(self, request), fields(session_id = %session_id, has_buyer = request.buyer.is_some()))]
    pub async fn complete_session(
        &self,
        session_id: &str,
        request: CheckoutSessionCompleteRequest,
    ) -> Result<CheckoutSessionWithOrder, ServiceError> {
        let session_lock = self.acquire_session_lock(session_id).await;
        let guard = session_lock.lock().await;

        let result = {
            let request = request;
            async move {
                let mut session = self.get_session(session_id).await?;

                if session.status == checkout_fsm::COMPLETED {
                    let order = self.load_existing_order_summary(&session).await?;
                    return Ok(CheckoutSessionWithOrder { session, order });
                }

                Self::ensure_session_open(&session)?;
                self.validate_complete_request(&request)?;

                if let Some(ref buyer) = request.buyer {
                    Self::validate_buyer(buyer)?;
                    session.buyer = Some(buyer.clone());
                }

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
                if let Some(option_id) = session.fulfillment_option_id.clone() {
                    Self::validate_fulfillment_selection(&session, &option_id)?;
                }

                Self::validate_payment_provider(&session, &request.payment_data)?;

                // Two-phase payment: Authorize then Capture
                // Phase 1: Authorize (reserve funds)
                let payment_intent = self
                    .authorize_payment(session_id, &request.payment_data)
                    .await?;

                // Store payment intent ID in session
                session.payment_intent_id = Some(payment_intent.id.clone());
                self.save_session(&session).await?;

                // Phase 2: Capture (charge customer)
                // If capture fails, we can retry or handle gracefully since authorization succeeded
                if let Err(e) = self.capture_payment(&payment_intent.id).await {
                    tracing::error!(
                        "Payment capture failed for session {} intent {}: {}",
                        session_id,
                        payment_intent.id,
                        e
                    );
                    return Err(ServiceError::PaymentFailed(format!(
                        "Payment authorized but capture failed: {}",
                        e
                    )));
                }
                info!("Payment captured successfully for session {}", session_id);

                let order = self
                    .orchestrate_checkout_completion(&mut session, &request)
                    .await?;

                session.status = "completed".to_string();
                let now = Utc::now();
                session.completed_at = Some(now);
                session.updated_at = Some(now);
                session.expires_at = now
                    + chrono::Duration::from_std(self.session_ttl())
                        .unwrap_or_else(|_| chrono::Duration::seconds(SESSION_TTL_SECS as i64));
                self.save_session(&session).await?;

                if let (Ok(session_uuid), Some(order_uuid)) = (
                    Uuid::parse_str(&session.id),
                    session
                        .order_id
                        .as_deref()
                        .and_then(|id| Uuid::parse_str(id).ok()),
                ) {
                    self.event_sender
                        .send_or_log(Event::CheckoutCompleted {
                            session_id: session_uuid,
                            order_id: order_uuid,
                        })
                        .await;
                }

                Ok(CheckoutSessionWithOrder { session, order })
            }
        }
        .await;

        drop(guard);
        self.release_session_lock(session_id, session_lock).await;

        if let Ok(ref result) = result {
            info!("Completed checkout session: {}", result.session.id);
        }

        result
    }

    /// Cancel checkout session
    #[instrument(skip(self), fields(session_id = %session_id))]
    pub async fn cancel_session(&self, session_id: &str) -> Result<CheckoutSession, ServiceError> {
        let session_lock = self.acquire_session_lock(session_id).await;
        let guard = session_lock.lock().await;

        let result = async move {
            let mut session = self.get_session(session_id).await?;

            Self::ensure_session_open(&session)?;

            session.status = "canceled".to_string();
            let now = Utc::now();
            session.canceled_at = Some(now);
            session.updated_at = Some(now);
            self.save_session(&session).await?;

            Ok(session)
        }
        .await;

        drop(guard);
        self.release_session_lock(session_id, session_lock).await;

        if let Ok(ref session) = result {
            info!("Canceled checkout session: {}", session.id);
        }

        result
    }

    // Private helper methods

    async fn acquire_session_lock(&self, session_id: &str) -> Arc<AsyncMutex<()>> {
        let mut locks = self.session_locks.lock().await;
        if let Some(lock) = locks.get(session_id) {
            lock.clone()
        } else {
            let new_lock = Arc::new(AsyncMutex::new(()));
            locks.insert(session_id.to_string(), new_lock.clone());
            new_lock
        }
    }

    async fn release_session_lock(&self, session_id: &str, lock: Arc<AsyncMutex<()>>) {
        if Arc::strong_count(&lock) == 1 {
            let mut locks = self.session_locks.lock().await;
            if let Some(existing) = locks.get(session_id) {
                if Arc::ptr_eq(existing, &lock) {
                    locks.remove(session_id);
                }
            }
        }
    }

    async fn acquire_idempotency_lock(&self, hash: &str) -> Arc<AsyncMutex<()>> {
        let mut locks = self.idempotency_locks.lock().await;
        if let Some(lock) = locks.get(hash) {
            lock.clone()
        } else {
            let new_lock = Arc::new(AsyncMutex::new(()));
            locks.insert(hash.to_string(), new_lock.clone());
            new_lock
        }
    }

    async fn release_idempotency_lock(&self, hash: &str, lock: Arc<AsyncMutex<()>>) {
        if Arc::strong_count(&lock) == 1 {
            let mut locks = self.idempotency_locks.lock().await;
            if let Some(existing) = locks.get(hash) {
                if Arc::ptr_eq(existing, &lock) {
                    locks.remove(hash);
                }
            }
        }
    }

    fn idempotency_cache_key(&self, hash: &str) -> String {
        format!("checkout_idem:{}", hash)
    }

    fn hash_idempotency_key(&self, key: &str) -> Result<String, ServiceError> {
        let key = key.trim();
        if key.len() < MIN_IDEMPOTENCY_KEY_LENGTH {
            return Err(ServiceError::ValidationError(format!(
                "Idempotency key must be at least {} characters long",
                MIN_IDEMPOTENCY_KEY_LENGTH
            )));
        }
        if key.len() > MAX_IDEMPOTENCY_KEY_LENGTH {
            return Err(ServiceError::ValidationError(format!(
                "Idempotency key must be {} characters or fewer",
                MAX_IDEMPOTENCY_KEY_LENGTH
            )));
        }
        if !key.chars().all(|c| c.is_ascii_graphic()) {
            return Err(ServiceError::ValidationError(
                "Idempotency key must contain visible ASCII characters only".to_string(),
            ));
        }

        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        Ok(format!("{:x}", hasher.finalize()))
    }

    fn validate_create_request(
        &self,
        request: &CheckoutSessionCreateRequest,
    ) -> Result<(), ServiceError> {
        Self::validate_items(&request.items)?;
        if let Some(buyer) = &request.buyer {
            Self::validate_buyer(buyer)?;
        }
        if let Some(address) = &request.fulfillment_address {
            Self::validate_address(address)?;
        }
        Ok(())
    }

    fn validate_update_request(
        &self,
        request: &CheckoutSessionUpdateRequest,
    ) -> Result<(), ServiceError> {
        if request.buyer.is_none()
            && request.items.is_none()
            && request.fulfillment_address.is_none()
            && request.fulfillment_option_id.is_none()
        {
            return Err(ServiceError::ValidationError(
                "At least one field must be supplied to update a checkout session".to_string(),
            ));
        }
        if let Some(items) = &request.items {
            Self::validate_items(items)?;
        }
        if let Some(buyer) = &request.buyer {
            Self::validate_buyer(buyer)?;
        }
        if let Some(address) = &request.fulfillment_address {
            Self::validate_address(address)?;
        }
        if let Some(option_id) = &request.fulfillment_option_id {
            if option_id.trim().is_empty() {
                return Err(ServiceError::ValidationError(
                    "Fulfillment option id cannot be blank".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn validate_complete_request(
        &self,
        request: &CheckoutSessionCompleteRequest,
    ) -> Result<(), ServiceError> {
        if let Some(buyer) = &request.buyer {
            Self::validate_buyer(buyer)?;
        }
        Self::validate_payment_data(&request.payment_data)?;
        Ok(())
    }

    fn validate_payment_provider(
        session: &CheckoutSession,
        payment_data: &PaymentData,
    ) -> Result<(), ServiceError> {
        if let Some(provider) = &session.payment_provider {
            if provider.provider != payment_data.provider {
                return Err(ServiceError::PaymentFailed(format!(
                    "Payment provider {} is not supported for this session",
                    payment_data.provider
                )));
            }

            if !provider
                .supported_payment_methods
                .iter()
                .any(|method| method.eq_ignore_ascii_case("card"))
            {
                return Err(ServiceError::PaymentFailed(
                    "No supported payment methods are available for this session".to_string(),
                ));
            }
        } else {
            return Err(ServiceError::PaymentFailed(
                "Checkout session does not have an associated payment provider".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_items(items: &[Item]) -> Result<(), ServiceError> {
        if items.is_empty() {
            return Err(ServiceError::ValidationError(
                "At least one item is required".to_string(),
            ));
        }
        if items.len() > MAX_LINE_ITEMS {
            return Err(ServiceError::ValidationError(format!(
                "A maximum of {} line items are supported",
                MAX_LINE_ITEMS
            )));
        }
        for item in items {
            Self::validate_item(item)?;
        }
        Ok(())
    }

    fn validate_item(item: &Item) -> Result<(), ServiceError> {
        if item.quantity < 1 || item.quantity > MAX_ITEM_QUANTITY {
            return Err(ServiceError::ValidationError(format!(
                "Quantity for item {} must be between 1 and {}",
                item.id, MAX_ITEM_QUANTITY
            )));
        }
        Self::ensure_ascii_identifier("item id", &item.id, MAX_ITEM_ID_LENGTH)?;
        Ok(())
    }

    fn validate_buyer(buyer: &Buyer) -> Result<(), ServiceError> {
        Self::ensure_non_empty("buyer.first_name", &buyer.first_name, MAX_NAME_LENGTH)?;
        Self::ensure_non_empty("buyer.last_name", &buyer.last_name, MAX_NAME_LENGTH)?;
        Self::ensure_non_empty("buyer.email", &buyer.email, MAX_EMAIL_LENGTH)?;
        if !validate_email(&buyer.email) {
            return Err(ServiceError::ValidationError(
                "buyer.email is not a valid email address".to_string(),
            ));
        }
        if let Some(phone) = &buyer.phone_number {
            if !Self::is_valid_phone(phone) {
                return Err(ServiceError::ValidationError(
                    "buyer.phone_number is not a valid phone number".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn validate_address(address: &Address) -> Result<(), ServiceError> {
        Self::ensure_non_empty("fulfillment_address.name", &address.name, MAX_NAME_LENGTH)?;
        Self::ensure_non_empty(
            "fulfillment_address.line_one",
            &address.line_one,
            MAX_ADDRESS_FIELD_LENGTH,
        )?;
        if let Some(line_two) = &address.line_two {
            Self::ensure_length(
                "fulfillment_address.line_two",
                line_two,
                MAX_ADDRESS_FIELD_LENGTH,
            )?;
        }
        Self::ensure_non_empty(
            "fulfillment_address.city",
            &address.city,
            MAX_ADDRESS_FIELD_LENGTH,
        )?;
        Self::ensure_non_empty(
            "fulfillment_address.state",
            &address.state,
            MAX_ADDRESS_FIELD_LENGTH,
        )?;
        Self::ensure_non_empty(
            "fulfillment_address.postal_code",
            &address.postal_code,
            MAX_ADDRESS_FIELD_LENGTH,
        )?;
        let country = address.country.trim();
        if country.len() != 2 || !country.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(ServiceError::ValidationError(
                "fulfillment_address.country must be a two-character ISO country code".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_payment_data(payment_data: &PaymentData) -> Result<(), ServiceError> {
        Self::ensure_non_empty("payment_data.token", &payment_data.token, 256)?;
        Self::ensure_ascii_identifier("payment_data.provider", &payment_data.provider, 64)?;
        if let Some(address) = &payment_data.billing_address {
            Self::validate_address(address)?;
        }
        Ok(())
    }

    fn validate_fulfillment_selection(
        session: &CheckoutSession,
        option_id: &str,
    ) -> Result<(), ServiceError> {
        if option_id.trim().is_empty() {
            return Err(ServiceError::ValidationError(
                "Fulfillment option id cannot be blank".to_string(),
            ));
        }
        let exists = session.fulfillment_options.iter().any(|opt| match opt {
            FulfillmentOption::Shipping(s) => s.id == option_id,
            FulfillmentOption::Digital(d) => d.id == option_id,
        });
        if !exists {
            return Err(ServiceError::InvalidInput(
                "Invalid fulfillment option".to_string(),
            ));
        }
        if session.fulfillment_address.is_none() {
            return Err(ServiceError::InvalidOperation(
                "Fulfillment address must be provided before selecting an option".to_string(),
            ));
        }
        Ok(())
    }

    fn ensure_session_open(session: &CheckoutSession) -> Result<(), ServiceError> {
        if checkout_fsm::is_terminal(&session.status) {
            return Err(ServiceError::InvalidOperation(format!(
                "Session is in terminal state '{}' and cannot be modified",
                session.status
            )));
        }
        if session.expires_at <= Utc::now() {
            return Err(ServiceError::InvalidOperation(
                "Checkout session has expired".to_string(),
            ));
        }
        Ok(())
    }

    fn ensure_non_empty(field: &str, value: &str, max_len: usize) -> Result<(), ServiceError> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(ServiceError::ValidationError(format!(
                "{} cannot be empty",
                field
            )));
        }
        Self::ensure_length(field, trimmed, max_len)
    }

    fn ensure_length(field: &str, value: &str, max_len: usize) -> Result<(), ServiceError> {
        if value.chars().count() > max_len {
            return Err(ServiceError::ValidationError(format!(
                "{} must be {} characters or fewer",
                field, max_len
            )));
        }
        Ok(())
    }

    fn ensure_ascii_identifier(
        field: &str,
        value: &str,
        max_len: usize,
    ) -> Result<(), ServiceError> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(ServiceError::ValidationError(format!(
                "{} cannot be empty",
                field
            )));
        }
        if !trimmed.is_ascii() {
            return Err(ServiceError::ValidationError(format!(
                "{} must use ASCII characters",
                field
            )));
        }
        if !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '/' | '#'))
        {
            return Err(ServiceError::ValidationError(format!(
                "{} contains unsupported characters",
                field
            )));
        }
        Self::ensure_length(field, trimmed, max_len)
    }

    fn is_valid_phone(phone: &str) -> bool {
        if phone.trim().is_empty() {
            return false;
        }
        if !phone
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, ' ' | '+' | '-' | '(' | ')' | '.'))
        {
            return false;
        }
        let digit_count = phone.chars().filter(|c| c.is_ascii_digit()).count();
        digit_count >= 7 && digit_count <= 16
    }

    async fn build_line_items(&self, items: &[Item]) -> Result<Vec<LineItem>, ServiceError> {
        Self::validate_items(items)?;
        let mut line_items = Vec::new();

        for item in items {
            // Try to parse item.id as UUID for database lookup
            let product = if let Ok(product_id) = Uuid::parse_str(&item.id) {
                // Look up by product UUID
                self.product_catalog.get_product(product_id).await.ok()
            } else {
                // If not a UUID, might be a legacy SKU or product identifier
                // For now, log a warning and skip
                warn!(
                    "Item ID '{}' is not a valid UUID, cannot fetch product details",
                    item.id
                );
                None
            };

            // Build line item from product data or use fallback
            let (unit_price_cents, title, sku, image_url) = if let Some(product) = product {
                let price_cents = product.price.to_i64().ok_or_else(|| {
                    ServiceError::InternalError(format!("Invalid price for product {}", item.id))
                })?;

                (
                    price_cents,
                    product.name.clone(),
                    Some(product.sku.clone()),
                    product.image_url.clone(),
                )
            } else {
                // Fallback for items not found in catalog
                warn!(
                    "Product not found for item ID '{}', using fallback pricing",
                    item.id
                );
                (
                    5000, // $50.00 default
                    format!("Product {}", item.id),
                    Some(item.id.clone()),
                    None,
                )
            };

            let base_amount = unit_price_cents * item.quantity as i64;
            // Item-level discounts (e.g., sale prices) are applied here
            // Promotion/coupon discounts are applied at checkout level in calculate_totals_with_promotion
            let item_discount = 0; // Item-specific discounts would come from product.sale_price or similar
            let subtotal = base_amount - item_discount;
            let tax = 0; // Tax will be calculated later based on address
            let total = subtotal + tax;

            line_items.push(LineItem {
                id: Uuid::new_v4().to_string(),
                title,
                quantity: item.quantity,
                unit_price: Money {
                    amount: unit_price_cents,
                    currency: "usd".to_string(),
                },
                variant_id: None, // Could be extended to support variants
                sku,
                image_url,
                item: Some(item.clone()),
                base_amount,
                discount: item_discount,
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
        self.calculate_totals_with_promotion(line_items, address, fulfillment_option_id, None)
    }

    fn calculate_totals_with_promotion(
        &self,
        line_items: &[LineItem],
        _address: Option<&Address>,
        fulfillment_option_id: Option<&str>,
        promotion: Option<&crate::models::promotion_entity::Model>,
    ) -> Result<Vec<Total>, ServiceError> {
        let mut totals = Vec::new();

        // Items base amount
        let items_base: i64 = line_items.iter().map(|item| item.base_amount).sum();
        totals.push(Total {
            total_type: "items_base_amount".to_string(),
            display_text: "Items".to_string(),
            amount: items_base,
        });

        // Items discount (from line items)
        let line_items_discount: i64 = line_items.iter().map(|item| item.discount).sum();

        // Apply promotion discount
        let promotion_discount = if let Some(promo) = promotion {
            self.promotion_service
                .calculate_discount(promo, items_base)
                .unwrap_or(0)
        } else {
            0
        };

        let total_items_discount = line_items_discount + promotion_discount;

        if total_items_discount > 0 {
            totals.push(Total {
                total_type: "items_discount".to_string(),
                display_text: if let Some(promo) = promotion {
                    format!("Discount ({})", promo.promotion_code)
                } else {
                    "Discount".to_string()
                },
                amount: -total_items_discount,
            });
        }

        // Subtotal after discount
        let subtotal = items_base - total_items_discount;
        totals.push(Total {
            total_type: "subtotal".to_string(),
            display_text: "Subtotal".to_string(),
            amount: subtotal,
        });

        // Fulfillment cost (check if promotion provides free shipping)
        let mut fulfillment_cost = if fulfillment_option_id.is_some() {
            1000 // $10.00 shipping
        } else {
            0
        };

        if fulfillment_cost > 0 {
            if let Some(promo) = promotion {
                if self.promotion_service.provides_free_shipping(promo) {
                    fulfillment_cost = 0;
                    totals.push(Total {
                        total_type: "fulfillment".to_string(),
                        display_text: format!("Shipping (FREE with {})", promo.promotion_code),
                        amount: 0,
                    });
                } else {
                    totals.push(Total {
                        total_type: "fulfillment".to_string(),
                        display_text: "Shipping".to_string(),
                        amount: fulfillment_cost,
                    });
                }
            } else {
                totals.push(Total {
                    total_type: "fulfillment".to_string(),
                    display_text: "Shipping".to_string(),
                    amount: fulfillment_cost,
                });
            }
        }

        // Tax (calculated on subtotal + shipping)
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

    /// Authorize payment (Phase 1: Reserve funds)
    /// Returns a payment intent ID that can be used to capture the payment later
    #[instrument(skip(self, payment_data), fields(session_id = %session_id, provider = %payment_data.provider))]
    pub async fn authorize_payment(
        &self,
        session_id: &str,
        payment_data: &PaymentData,
    ) -> Result<PaymentIntent, ServiceError> {
        let session = self.get_session(session_id).await?;

        // Validate session is ready for payment
        if session.status != "ready_for_payment" {
            return Err(ServiceError::InvalidOperation(format!(
                "Session must be ready_for_payment to authorize. Current status: {}",
                session.status
            )));
        }

        Self::validate_payment_provider(&session, payment_data)?;

        // Calculate total amount to authorize
        let grand_total = session
            .totals
            .iter()
            .find(|t| t.total_type == "total")
            .ok_or_else(|| ServiceError::InternalError("Total not found".to_string()))?;

        // Amount is already in cents
        let amount_cents = grand_total.amount;

        info!(
            "Authorizing payment for session {} with provider: {} amount: {} (token: ***)",
            session_id, payment_data.provider, amount_cents
        );

        // Generate payment intent ID
        let intent_id = format!("pi_{}", Uuid::new_v4().to_string().replace('-', ""));

        // In production, this would call the payment provider's API
        // For example with Stripe:
        // let intent = stripe_client
        //     .payment_intents()
        //     .create(amount_cents, session.currency, payment_data.token)
        //     .await?;

        // Simulate successful authorization
        let payment_intent = PaymentIntent {
            id: intent_id.clone(),
            status: PaymentIntentStatus::Authorized,
            amount: amount_cents,
            currency: session.currency.clone(),
            provider: payment_data.provider.clone(),
            created_at: Utc::now(),
        };

        // Store payment intent ID in cache for later capture
        let cache_key = format!("payment_intent:{}", intent_id);
        let data = serde_json::to_string(&payment_intent)
            .map_err(|e| ServiceError::SerializationError(e.to_string()))?;
        self.cache
            .set(&cache_key, &data, Some(Duration::from_secs(3600)))
            .await
            .map_err(|e| ServiceError::CacheError(e.to_string()))?;

        info!("Payment authorized successfully: {}", intent_id);
        Ok(payment_intent)
    }

    /// Capture payment (Phase 2: Charge the customer)
    /// Uses the payment intent ID from the authorize phase
    #[instrument(skip(self), fields(intent_id = %intent_id))]
    pub async fn capture_payment(&self, intent_id: &str) -> Result<(), ServiceError> {
        // Retrieve payment intent from cache
        let cache_key = format!("payment_intent:{}", intent_id);
        let data = self
            .cache
            .get(&cache_key)
            .await
            .map_err(|e| ServiceError::CacheError(e.to_string()))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Payment intent {} not found", intent_id))
            })?;

        let mut payment_intent: PaymentIntent = serde_json::from_str(&data)
            .map_err(|e| ServiceError::SerializationError(e.to_string()))?;

        // Verify intent is in authorized state
        if payment_intent.status != PaymentIntentStatus::Authorized {
            return Err(ServiceError::PaymentFailed(format!(
                "Payment intent {} is in state {:?}, expected Authorized",
                intent_id, payment_intent.status
            )));
        }

        info!(
            "Capturing payment for intent {} with provider: {} amount: {}",
            intent_id, payment_intent.provider, payment_intent.amount
        );

        // In production, this would call the payment provider's capture API
        // For example with Stripe:
        // stripe_client
        //     .payment_intents()
        //     .capture(intent_id)
        //     .await?;

        // Simulate successful capture
        payment_intent.status = PaymentIntentStatus::Captured;

        // Update payment intent in cache
        let data = serde_json::to_string(&payment_intent)
            .map_err(|e| ServiceError::SerializationError(e.to_string()))?;
        self.cache
            .set(&cache_key, &data, Some(Duration::from_secs(3600)))
            .await
            .map_err(|e| ServiceError::CacheError(e.to_string()))?;

        info!("Payment captured successfully: {}", intent_id);
        Ok(())
    }

    async fn orchestrate_checkout_completion(
        &self,
        session: &mut CheckoutSession,
        request: &CheckoutSessionCompleteRequest,
    ) -> Result<Order, ServiceError> {
        let total_cents = Self::find_total_amount(&session.totals, "total").ok_or_else(|| {
            ServiceError::InvalidOperation(
                "Checkout session totals missing grand total amount".to_string(),
            )
        })?;
        let total_decimal = Self::decimal_from_cents(total_cents);
        let currency = session.currency.to_uppercase();

        let order_items = Self::build_order_item_inputs(&session.line_items)?;

        let shipping_address_json = session
            .fulfillment_address
            .as_ref()
            .map(Self::serialize_address)
            .transpose()?;

        let billing_address_json = request
            .payment_data
            .billing_address
            .as_ref()
            .or(session.fulfillment_address.as_ref())
            .map(Self::serialize_address)
            .transpose()?;

        let selected_option = session
            .fulfillment_option_id
            .as_ref()
            .and_then(|id| Self::find_fulfillment_option(&session.fulfillment_options, id));
        let (shipping_method_enum, shipping_method_label) =
            Self::derive_shipping_details(selected_option);

        let (mut order_response, _) = self
            .order_service
            .create_order_with_items(CreateOrderWithItemsInput {
                customer_id: Uuid::new_v4(),
                total_amount: total_decimal,
                currency: currency.clone(),
                payment_status: "pending".to_string(),
                fulfillment_status: if shipping_method_enum.is_some() {
                    "unfulfilled".to_string()
                } else {
                    "fulfilled".to_string()
                },
                payment_method: Some(request.payment_data.provider.clone()),
                shipping_method: shipping_method_label.clone(),
                shipping_address: shipping_address_json.clone(),
                billing_address: billing_address_json,
                notes: None,
                items: order_items,
            })
            .await?;

        let order_id = order_response.id;

        let payment_method_enum =
            Self::map_payment_method_from_provider(&request.payment_data.provider)?;

        let payment_response = self
            .payment_service
            .process_payment(ProcessPaymentRequest {
                order_id,
                amount: total_decimal,
                payment_method: payment_method_enum,
                payment_method_id: Some(request.payment_data.token.clone()),
                currency: currency.clone(),
                description: Some(format!("Agentic checkout {}", session.id)),
            })
            .await?;

        let normalized_payment_status = Self::normalize_payment_status(&payment_response.status);

        let _ = self
            .order_service
            .update_payment_status(
                order_id,
                normalized_payment_status,
                Some(payment_response.payment_method.clone()),
            )
            .await?;

        order_response = self
            .order_service
            .update_order_status(
                order_id,
                UpdateOrderStatusRequest {
                    status: "confirmed".to_string(),
                    notes: Some("Payment captured via Agentic checkout".to_string()),
                },
            )
            .await?;

        let invoice_id = self
            .invoicing_service
            .generate_invoice(order_id)
            .await
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;

        let cash_sale_id = self
            .cash_sale_service
            .create_cash_sale(
                order_id,
                total_decimal,
                payment_response.payment_method.clone(),
            )
            .await
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;

        let mut shipment_id = None;
        let mut tracking_number: Option<String> = None;

        if let Some(shipping_method_enum) = shipping_method_enum {
            let shipping_payload = shipping_address_json.clone().ok_or_else(|| {
                ServiceError::InvalidOperation(
                    "Shipping address required for fulfillment selection".to_string(),
                )
            })?;
            let recipient_name =
                Self::derive_recipient_name(session).unwrap_or_else(|| "Customer".to_string());
            let tracking = format!("trk_{}", Uuid::new_v4().to_string().replace('-', ""));
            let command = CreateShipmentCommand {
                order_id,
                shipping_address: shipping_payload,
                shipping_method: shipping_method_enum,
                tracking_number: tracking.clone(),
                recipient_name,
            };
            let created_shipment_id = self.shipment_service.create_shipment(command).await?;
            shipment_id = Some(created_shipment_id);
            tracking_number = Some(tracking.clone());
        }

        order_response = self
            .order_service
            .update_fulfillment_details(
                order_id,
                if shipment_id.is_some() {
                    Some("processing".to_string())
                } else {
                    Some(order_response.fulfillment_status.clone())
                },
                shipping_method_label.clone(),
                tracking_number.clone(),
            )
            .await?;

        session.order_id = Some(order_id.to_string());
        session.payment_id = Some(payment_response.id.to_string());
        session.invoice_id = Some(invoice_id.to_string());
        session.cash_sale_id = Some(cash_sale_id.to_string());
        session.shipment_id = shipment_id.map(|id| id.to_string());
        session.payment_provider = Some(PaymentProvider {
            provider: request.payment_data.provider.clone(),
            supported_payment_methods: vec![payment_response.payment_method.to_ascii_lowercase()],
        });

        if let Some(tracking) = tracking_number.clone() {
            Self::upsert_link(
                &mut session.links,
                "shipment_tracking",
                format!("https://carrier.example.com/track/{}", tracking),
            );
        }
        let permalink = Self::order_permalink(&order_response);
        Self::upsert_link(&mut session.links, "order", permalink.clone());

        session.messages.push(Message::Info(MessageInfo {
            message_type: "order_update".to_string(),
            param: Some(order_response.order_number.clone()),
            content_type: "text".to_string(),
            content: format!(
                "Order {} confirmed. Invoice {} and payment {} recorded.",
                order_response.order_number, invoice_id, payment_response.id
            ),
        }));

        Ok(Self::compose_order_summary(
            session,
            &order_response,
            permalink,
        ))
    }

    async fn load_existing_order_summary(
        &self,
        session: &CheckoutSession,
    ) -> Result<Order, ServiceError> {
        let order_id = session.order_id.as_ref().ok_or_else(|| {
            ServiceError::InvalidOperation(
                "Checkout session completed without an order reference".to_string(),
            )
        })?;
        let order_uuid = Uuid::parse_str(order_id).map_err(|_| {
            ServiceError::InvalidOperation("Stored order_id is not a valid UUID".to_string())
        })?;

        let order_response = self
            .order_service
            .get_order(order_uuid)
            .await?
            .ok_or_else(|| ServiceError::NotFound(format!("Order {} not found", order_id)))?;

        let permalink = Self::order_permalink(&order_response);

        Ok(Self::compose_order_summary(
            session,
            &order_response,
            permalink,
        ))
    }

    fn compose_order_summary(
        session: &CheckoutSession,
        order: &OrderResponse,
        permalink_url: String,
    ) -> Order {
        Order {
            id: order.id.to_string(),
            checkout_session_id: session.id.clone(),
            permalink_url,
            order_number: Some(order.order_number.clone()),
            status: Some(order.status.clone()),
            payment_status: Some(order.payment_status.clone()),
            fulfillment_status: Some(order.fulfillment_status.clone()),
            payment_id: session.payment_id.clone(),
            payment_method: order.payment_method.clone(),
            invoice_id: session.invoice_id.clone(),
            cash_sale_id: session.cash_sale_id.clone(),
            shipment_id: session.shipment_id.clone(),
            shipping_method: order.shipping_method.clone(),
            tracking_number: order.tracking_number.clone(),
        }
    }

    fn order_permalink(order: &OrderResponse) -> String {
        let identifier = if order.order_number.trim().is_empty() {
            order.id.to_string()
        } else {
            order.order_number.clone()
        };
        format!("https://merchant.example.com/orders/{}", identifier)
    }

    fn build_order_item_inputs(
        line_items: &[LineItem],
    ) -> Result<Vec<NewOrderItemInput>, ServiceError> {
        let mut inputs = Vec::with_capacity(line_items.len());
        for item in line_items {
            let unit_price = Self::decimal_from_cents(item.unit_price.amount);
            let product_id = item
                .variant_id
                .as_ref()
                .and_then(|id| Uuid::parse_str(id).ok())
                .or_else(|| {
                    item.item
                        .as_ref()
                        .and_then(|raw| Uuid::parse_str(&raw.id).ok())
                });
            let sku = item.sku.clone().unwrap_or_else(|| item.id.clone());
            let tax_rate = if item.subtotal > 0 && item.tax > 0 {
                Some(Self::decimal_from_cents(item.tax) / Self::decimal_from_cents(item.subtotal))
            } else {
                None
            };
            inputs.push(NewOrderItemInput {
                sku,
                product_id,
                name: Some(item.title.clone()),
                quantity: item.quantity,
                unit_price,
                tax_rate,
            });
        }
        Ok(inputs)
    }

    fn find_total_amount(totals: &[Total], total_type: &str) -> Option<i64> {
        totals
            .iter()
            .find(|total| total.total_type == total_type)
            .map(|total| total.amount)
    }

    fn decimal_from_cents(amount: i64) -> Decimal {
        Decimal::new(amount, 2)
    }

    fn map_payment_method_from_provider(provider: &str) -> Result<PaymentMethod, ServiceError> {
        match provider.to_ascii_lowercase().as_str() {
            "stripe" | "card" | "credit_card" | "visa" | "mastercard" | "amex" => {
                Ok(PaymentMethod::CreditCard)
            }
            "debit_card" | "debit" => Ok(PaymentMethod::DebitCard),
            "paypal" => Ok(PaymentMethod::PayPal),
            "bank_transfer" | "ach" => Ok(PaymentMethod::BankTransfer),
            "cash" => Ok(PaymentMethod::Cash),
            "check" | "cheque" => Ok(PaymentMethod::Check),
            other => {
                warn!(
                    "Unsupported payment provider '{}', defaulting to credit card",
                    other
                );
                Ok(PaymentMethod::CreditCard)
            }
        }
    }

    fn normalize_payment_status(status: &str) -> String {
        match status.to_ascii_lowercase().as_str() {
            "succeeded" | "captured" => "paid".to_string(),
            "failed" => "failed".to_string(),
            "cancelled" | "canceled" => "cancelled".to_string(),
            "refunded" => "refunded".to_string(),
            "processing" => "processing".to_string(),
            _ => "pending".to_string(),
        }
    }

    fn serialize_address(address: &Address) -> Result<String, ServiceError> {
        serde_json::to_string(address).map_err(|e| ServiceError::SerializationError(e.to_string()))
    }

    fn find_fulfillment_option<'a>(
        options: &'a [FulfillmentOption],
        id: &str,
    ) -> Option<&'a FulfillmentOption> {
        options.iter().find(|option| match option {
            FulfillmentOption::Shipping(opt) => opt.id == id,
            FulfillmentOption::Digital(opt) => opt.id == id,
        })
    }

    fn derive_shipping_details(
        option: Option<&FulfillmentOption>,
    ) -> (Option<shipment::ShippingMethod>, Option<String>) {
        match option {
            Some(FulfillmentOption::Shipping(shipping)) => {
                let method = match shipping.id.as_str() {
                    "standard_shipping" => shipment::ShippingMethod::Standard,
                    "express_shipping" => shipment::ShippingMethod::Express,
                    "overnight_shipping" => shipment::ShippingMethod::Overnight,
                    "two_day_shipping" | "two_day" => shipment::ShippingMethod::TwoDay,
                    "international_shipping" => shipment::ShippingMethod::International,
                    _ => shipment::ShippingMethod::Standard,
                };
                (Some(method), Some(shipping.title.clone()))
            }
            _ => (None, None),
        }
    }

    fn derive_recipient_name(session: &CheckoutSession) -> Option<String> {
        if let Some(address) = &session.fulfillment_address {
            if !address.name.trim().is_empty() {
                return Some(address.name.clone());
            }
        }
        session.buyer.as_ref().map(|buyer| {
            let first = buyer.first_name.trim();
            let last = buyer.last_name.trim();
            let combined = format!("{} {}", first, last).trim().to_string();
            if combined.is_empty() {
                buyer.email.clone()
            } else {
                combined
            }
        })
    }

    fn upsert_link(links: &mut Vec<Link>, link_type: &str, url: String) {
        if let Some(existing) = links.iter_mut().find(|link| link.link_type == link_type) {
            existing.url = url;
        } else {
            links.push(Link {
                link_type: link_type.to_string(),
                url,
            });
        }
    }

    async fn save_session(&self, session: &CheckoutSession) -> Result<(), ServiceError> {
        let cache_key = format!("checkout_session:{}", session.id);
        let data = serde_json::to_string(session)
            .map_err(|e| ServiceError::SerializationError(e.to_string()))?;

        self.cache
            .set(&cache_key, &data, Some(self.session_ttl()))
            .await
            .map_err(|e| ServiceError::CacheError(e.to_string()))?;

        Ok(())
    }
}

// Data models matching OpenAPI spec

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Address {
    pub name: String,
    pub line_one: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_two: Option<String>,
    pub city: String,
    pub state: String,
    pub country: String,
    pub postal_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    pub title: String,
    pub quantity: i32,
    pub unit_price: Money,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sku: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    // Internal fields for calculation (kept for backwards compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<Item>,
    pub base_amount: i64,
    pub discount: i64,
    pub subtotal: i64,
    pub tax: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Money {
    pub amount: i64,      // Amount in cents
    pub currency: String, // ISO 4217 currency code (e.g., "usd")
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invoice_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cash_sale_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipment_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracking_number: Option<String>,
}

/// Payment intent for two-phase payment flow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentIntent {
    pub id: String,
    pub status: PaymentIntentStatus,
    pub amount: i64, // in cents
    pub currency: String,
    pub provider: String,
    pub created_at: DateTime<Utc>,
}

/// Payment intent status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentIntentStatus {
    /// Funds have been reserved but not captured
    Authorized,
    /// Funds have been charged to customer
    Captured,
    /// Authorization has expired or been cancelled
    Cancelled,
    /// Payment failed
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSession {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buyer: Option<Buyer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_provider: Option<PaymentProvider>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invoice_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cash_sale_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shipment_id: Option<String>,
    pub status: String,
    pub currency: String,
    pub line_items: Vec<LineItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment_address: Option<Address>,
    pub fulfillment_options: Vec<FulfillmentOption>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment_option_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub promotion_code: Option<String>,
    pub totals: Vec<Total>,
    pub messages: Vec<Message>,
    pub links: Vec<Link>,
    #[serde(default = "default_session_timestamp")]
    pub created_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,
    #[serde(default = "default_session_expiry")]
    pub expires_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canceled_at: Option<DateTime<Utc>>,
    /// Payment intent ID from authorize phase (two-phase payment)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_intent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSessionWithOrder {
    #[serde(flatten)]
    pub session: CheckoutSession,
    pub order: Order,
}

#[derive(Debug, Clone)]
pub struct CreateSessionResult {
    pub session: CheckoutSession,
    pub was_created: bool,
}

// Request types

#[derive(Debug, Clone, Deserialize)]
pub struct CheckoutSessionCreateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buyer: Option<Buyer>,
    pub items: Vec<Item>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment_address: Option<Address>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub promotion_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CheckoutSessionUpdateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buyer: Option<Buyer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<Item>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment_address: Option<Address>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment_option_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub promotion_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CheckoutSessionCompleteRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buyer: Option<Buyer>,
    pub payment_data: PaymentData,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        circuit_breaker::{CircuitBreaker, CircuitBreakerRegistry},
        message_queue::{InMemoryMessageQueue, MessageQueue},
        services::{
            cash_sale::CashSaleService, invoicing::InvoicingService, orders::OrderService,
            payments::PaymentService, shipments::ShipmentService,
        },
    };
    use sea_orm::Database;
    use slog::Logger;
    use std::time::Duration;
    use tokio::sync::mpsc;

    async fn build_service() -> AgenticCheckoutService {
        let db = Arc::new(
            Database::connect("sqlite::memory:")
                .await
                .expect("connect in-memory sqlite"),
        );
        let cache = Arc::new(InMemoryCache::new());
        let (tx, _rx) = mpsc::channel(4);
        let event_sender = Arc::new(EventSender::new(tx));
        let product_catalog =
            Arc::new(ProductCatalogService::new(db.clone(), event_sender.clone()));
        let order_service = Arc::new(OrderService::new(db.clone(), Some(event_sender.clone())));
        let payment_service = Arc::new(PaymentService::new(db.clone(), event_sender.clone()));
        let invoicing_service = Arc::new(InvoicingService::new(db.clone()));
        let cash_sale_service = Arc::new(CashSaleService::new(db.clone()));
        let redis_client = Arc::new(redis::Client::open("redis://127.0.0.1/").unwrap());
        let message_queue: Arc<dyn MessageQueue> = Arc::new(InMemoryMessageQueue::new());
        let circuit_breaker = Arc::new(CircuitBreaker::new(5, Duration::from_secs(60), 2));
        let circuit_breaker_registry = Arc::new(CircuitBreakerRegistry::new(None));
        let logger = Logger::root(slog::Discard, slog::o!());
        let shipment_service = Arc::new(ShipmentService::new(
            db.clone(),
            event_sender.clone(),
            redis_client,
            message_queue,
            circuit_breaker,
            circuit_breaker_registry,
            logger,
        ));

        let promotion_service = Arc::new(crate::services::promotions::PromotionService::new(
            (*db).clone(),
        ));

        AgenticCheckoutService::new(
            db,
            cache,
            event_sender,
            product_catalog,
            order_service,
            payment_service,
            promotion_service,
            shipment_service,
            invoicing_service,
            cash_sale_service,
        )
    }

    fn fixture_buyer() -> Buyer {
        Buyer {
            first_name: "Ada".to_string(),
            last_name: "Lovelace".to_string(),
            email: "ada@example.com".to_string(),
            phone_number: Some("+1-555-555-0101".to_string()),
        }
    }

    fn fixture_address() -> Address {
        Address {
            name: "Ada Lovelace".to_string(),
            line_one: "123 Example St".to_string(),
            line_two: None,
            city: "San Francisco".to_string(),
            state: "CA".to_string(),
            country: "US".to_string(),
            postal_code: "94105".to_string(),
            phone: None,
            email: None,
        }
    }

    #[tokio::test]
    async fn create_session_rejects_invalid_email() {
        let service = build_service().await;
        let request = CheckoutSessionCreateRequest {
            buyer: Some(Buyer {
                email: "not-an-email".to_string(),
                ..fixture_buyer()
            }),
            items: vec![Item {
                id: "sku-123".to_string(),
                quantity: 1,
            }],
            fulfillment_address: Some(fixture_address()),
            promotion_code: None,
        };

        let result = service.create_session(request, None).await;
        assert!(matches!(result, Err(ServiceError::ValidationError(_))));
    }

    #[tokio::test]
    async fn create_session_is_idempotent() {
        let service = build_service().await;
        let request = CheckoutSessionCreateRequest {
            buyer: Some(fixture_buyer()),
            items: vec![Item {
                id: "sku-123".to_string(),
                quantity: 2,
            }],
            fulfillment_address: Some(fixture_address()),
            promotion_code: None,
        };

        let first = service
            .create_session(request.clone(), Some("idem-key-1234"))
            .await
            .expect("first create succeeds");
        assert!(first.was_created);

        let second = service
            .create_session(request, Some("idem-key-1234"))
            .await
            .expect("second create succeeds via idempotency");
        assert!(!second.was_created);
        assert_eq!(first.session.id, second.session.id);
    }

    #[tokio::test]
    async fn create_session_recovers_from_stale_idempotency_reference() {
        let service = build_service().await;
        let request = CheckoutSessionCreateRequest {
            buyer: Some(fixture_buyer()),
            items: vec![Item {
                id: "sku-555".to_string(),
                quantity: 1,
            }],
            fulfillment_address: Some(fixture_address()),
            promotion_code: None,
        };

        let first = service
            .create_session(request.clone(), Some("idem-stale-0001"))
            .await
            .expect("initial create succeeds");

        service
            .cache
            .delete(&format!("checkout_session:{}", first.session.id))
            .await
            .expect("delete session cache entry");

        let second = service
            .create_session(request, Some("idem-stale-0001"))
            .await
            .expect("recreate succeeds");

        assert!(second.was_created);
        assert_ne!(first.session.id, second.session.id);
    }

    #[tokio::test]
    async fn complete_session_rejects_payment_provider_mismatch() {
        let service = build_service().await;
        let request = CheckoutSessionCreateRequest {
            buyer: Some(fixture_buyer()),
            items: vec![Item {
                id: "sku-987".to_string(),
                quantity: 1,
            }],
            fulfillment_address: Some(fixture_address()),
            promotion_code: None,
        };

        let created = service
            .create_session(request, None)
            .await
            .expect("session created");
        let session_id = created.session.id.clone();
        let shipping_option = created
            .session
            .fulfillment_options
            .iter()
            .find_map(|opt| match opt {
                FulfillmentOption::Shipping(option) => Some(option.id.clone()),
                _ => None,
            })
            .expect("shipping option");

        service
            .update_session(
                &session_id,
                CheckoutSessionUpdateRequest {
                    buyer: None,
                    items: None,
                    fulfillment_address: None,
                    fulfillment_option_id: Some(shipping_option),
                    promotion_code: None,
                },
            )
            .await
            .expect("update succeeds");

        let result = service
            .complete_session(
                &session_id,
                CheckoutSessionCompleteRequest {
                    buyer: None,
                    payment_data: PaymentData {
                        token: "tok_mismatch".to_string(),
                        provider: "paypal".to_string(),
                        billing_address: None,
                    },
                },
            )
            .await;

        assert!(matches!(result, Err(ServiceError::PaymentFailed(_))));
    }
}
