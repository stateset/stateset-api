use crate::{
    cache::InMemoryCache,
    errors::ServiceError,
    events::{Event, EventSender},
};
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex as AsyncMutex;
use tracing::{info, instrument};
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

fn default_session_timestamp() -> DateTime<Utc> {
    Utc::now()
}

fn default_session_expiry() -> DateTime<Utc> {
    Utc::now() + chrono::Duration::seconds(SESSION_TTL_SECS as i64)
}

/// Agentic checkout service for ChatGPT-driven checkout flow
#[derive(Clone)]
pub struct AgenticCheckoutService {
    db: Arc<DatabaseConnection>,
    cache: Arc<InMemoryCache>,
    event_sender: Arc<EventSender>,
    session_ttl: Duration,
    session_locks: Arc<AsyncMutex<HashMap<String, Arc<AsyncMutex<()>>>>>,
    idempotency_locks: Arc<AsyncMutex<HashMap<String, Arc<AsyncMutex<()>>>>>,
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
            session_ttl: Duration::from_secs(SESSION_TTL_SECS),
            session_locks: Arc::new(AsyncMutex::new(HashMap::new())),
            idempotency_locks: Arc::new(AsyncMutex::new(HashMap::new())),
        }
    }

    /// Create checkout session
    #[instrument(skip(self))]
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

        let session_id = Uuid::new_v4();
        let currency = "USD".to_string();
        let now = Utc::now();
        let expires_at = now
            + chrono::Duration::from_std(self.session_ttl)
                .unwrap_or_else(|_| chrono::Duration::seconds(SESSION_TTL_SECS as i64));

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
            created_at: now,
            updated_at: None,
            expires_at,
            completed_at: None,
            canceled_at: None,
        };

        // Store session in cache with 1 hour TTL
        self.save_session(&session).await?;
        if let Some(hash) = hashed_idempotency {
            self.cache
                .set(
                    &self.idempotency_cache_key(hash),
                    &session.id,
                    Some(self.session_ttl),
                )
                .await
                .map_err(|e| ServiceError::CacheError(e.to_string()))?;
        }

        self.event_sender
            .send_or_log(Event::CheckoutStarted {
                cart_id: Uuid::nil(), // Not cart-based
                session_id: Uuid::parse_str(&session.id).unwrap(),
            })
            .await;

        info!("Created checkout session: {}", session.id);
        Ok(CreateSessionResult {
            session,
            was_created: true,
        })
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
                let mut session: CheckoutSession = serde_json::from_str(&data)
                    .map_err(|e| ServiceError::SerializationError(e.to_string()))?;

                if session.status != "completed" && session.status != "canceled" {
                    if let Ok(ttl_duration) = chrono::Duration::from_std(self.session_ttl) {
                        let now = Utc::now();
                        if session.expires_at > now {
                            let remaining = session.expires_at - now;
                            if remaining < ttl_duration / 2 {
                                session.expires_at = now + ttl_duration;
                                session.updated_at = Some(now);
                                self.save_session(&session).await?;
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
    #[instrument(skip(self))]
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

                if let Some(buyer) = request.buyer {
                    Self::validate_buyer(&buyer)?;
                    session.buyer = Some(buyer);
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
                    + chrono::Duration::from_std(self.session_ttl)
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
    #[instrument(skip(self))]
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

                Self::ensure_session_open(&session)?;
                self.validate_complete_request(&request)?;

                if let Some(buyer) = request.buyer {
                    Self::validate_buyer(&buyer)?;
                    session.buyer = Some(buyer);
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

                self.process_payment(&request.payment_data).await?;

                let order = self.create_order_from_session(&session).await?;

                session.status = "completed".to_string();
                let now = Utc::now();
                session.completed_at = Some(now);
                session.updated_at = Some(now);
                session.expires_at = now
                    + chrono::Duration::from_std(self.session_ttl)
                        .unwrap_or_else(|_| chrono::Duration::seconds(SESSION_TTL_SECS as i64));
                self.save_session(&session).await?;

                self.event_sender
                    .send_or_log(Event::CheckoutCompleted {
                        session_id: Uuid::parse_str(&session.id).unwrap(),
                        order_id: Uuid::parse_str(&order.id).unwrap(),
                    })
                    .await;

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
    #[instrument(skip(self))]
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
            .set(&cache_key, &data, Some(self.session_ttl))
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
    use sea_orm::Database;
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
        AgenticCheckoutService::new(db, cache, event_sender)
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
