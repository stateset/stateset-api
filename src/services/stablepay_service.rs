use crate::{
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        stablepay_payment_method, stablepay_provider, stablepay_refund, stablepay_transaction,
    },
};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;
use validator::{Validate, ValidationError};

fn validate_positive_decimal(value: &Decimal) -> Result<(), ValidationError> {
    if *value > Decimal::ZERO {
        Ok(())
    } else {
        let mut err = ValidationError::new("range");
        err.message = Some("Amount must be greater than 0".into());
        Err(err)
    }
}

fn validate_currency(currency: &str) -> Result<(), ValidationError> {
    if currency.len() == 3 && currency.chars().all(|c| c.is_ascii_alphabetic()) {
        Ok(())
    } else {
        let mut err = ValidationError::new("currency");
        err.message = Some("Currency must be a 3-letter ISO code".into());
        Err(err)
    }
}

/// Request to create a payment
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreatePaymentRequest {
    pub order_id: Option<Uuid>,
    pub customer_id: Uuid,
    pub payment_method_id: Option<Uuid>,
    #[validate(custom = "validate_positive_decimal")]
    pub amount: Decimal,
    #[validate(length(equal = 3), custom = "validate_currency")]
    pub currency: String,
    pub description: Option<String>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    pub idempotency_key: Option<String>,
}

/// Response for a payment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentResponse {
    pub id: Uuid,
    pub transaction_number: String,
    pub order_id: Option<Uuid>,
    pub customer_id: Uuid,
    pub amount: Decimal,
    pub currency: String,
    pub status: String,
    pub provider_name: String,
    pub provider_fee: Decimal,
    pub platform_fee: Decimal,
    pub total_fees: Decimal,
    pub net_amount: Decimal,
    pub initiated_at: chrono::DateTime<Utc>,
    pub processed_at: Option<chrono::DateTime<Utc>>,
    pub estimated_settlement_date: Option<chrono::NaiveDate>,
}

/// Request to refund a payment
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateRefundRequest {
    pub transaction_id: Uuid,
    #[validate(custom = "validate_positive_decimal")]
    pub amount: Decimal,
    pub reason: Option<String>,
    pub reason_detail: Option<String>,
}

/// Response for a refund
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundResponse {
    pub id: Uuid,
    pub refund_number: String,
    pub transaction_id: Uuid,
    pub amount: Decimal,
    pub currency: String,
    pub status: String,
    pub refunded_fees: Decimal,
    pub net_refund: Decimal,
    pub requested_at: chrono::DateTime<Utc>,
}

/// Currency conversion request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencyConversion {
    pub from_currency: String,
    pub to_currency: String,
    pub amount: Decimal,
    pub exchange_rate: Decimal,
}

/// StablePay Service - Enterprise payment processing
pub struct StablePayService {
    db: Arc<DatabaseConnection>,
    event_sender: Arc<EventSender>,
}

impl StablePayService {
    pub fn new(db: Arc<DatabaseConnection>, event_sender: Arc<EventSender>) -> Self {
        Self { db, event_sender }
    }

    /// Create and process a payment with intelligent routing
    #[instrument(skip(self, request))]
    pub async fn create_payment(
        &self,
        request: CreatePaymentRequest,
    ) -> Result<PaymentResponse, ServiceError> {
        request.validate()?;

        info!(
            customer_id = %request.customer_id,
            amount = %request.amount,
            currency = %request.currency,
            "Creating StablePay payment"
        );

        // Check for idempotency
        if let Some(ref key) = request.idempotency_key {
            if let Some(existing) = self.find_by_idempotency_key(key).await? {
                info!("Returning existing payment for idempotency key");
                return Ok(self.payment_to_response(existing).await?);
            }
        }

        // Select optimal provider based on routing rules
        let provider = self
            .select_optimal_provider(&request.currency, &request.amount)
            .await?;

        info!(provider_name = %provider.name, "Selected payment provider");

        // Calculate fees
        let provider_fee = provider.calculate_fee(request.amount);
        let platform_fee = self.calculate_platform_fee(request.amount);
        let total_fees = provider_fee + platform_fee;
        let net_amount = request.amount - total_fees;

        // Generate transaction number
        let transaction_number = self.generate_transaction_number().await?;

        // Create transaction record
        let transaction_id = Uuid::new_v4();
        let now = Utc::now();
        let estimated_settlement = now.date_naive() + Duration::days(2);

        let transaction_model = stablepay_transaction::ActiveModel {
            id: Set(transaction_id),
            transaction_number: Set(transaction_number.clone()),
            order_id: Set(request.order_id),
            customer_id: Set(request.customer_id),
            payment_method_id: Set(request.payment_method_id),
            provider_id: Set(provider.id),
            amount: Set(request.amount),
            currency: Set(request.currency.clone()),
            original_amount: Set(None),
            original_currency: Set(None),
            exchange_rate: Set(None),
            provider_fee: Set(provider_fee),
            platform_fee: Set(platform_fee),
            total_fees: Set(total_fees),
            net_amount: Set(net_amount),
            status: Set("processing".to_string()),
            payment_intent_id: Set(None),
            charge_id: Set(None),
            initiated_at: Set(now),
            processed_at: Set(None),
            settled_at: Set(None),
            estimated_settlement_date: Set(Some(estimated_settlement)),
            failure_code: Set(None),
            failure_message: Set(None),
            retry_count: Set(0),
            is_reconciled: Set(false),
            reconciled_at: Set(None),
            reconciliation_id: Set(None),
            risk_score: Set(self.calculate_risk_score(&request).await?),
            is_flagged_for_review: Set(false),
            fraud_indicators: Set(None),
            description: Set(request.description.clone()),
            metadata: Set(request.metadata.clone()),
            gateway_response: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
            created_by: Set(None),
            idempotency_key: Set(request.idempotency_key),
        };

        let transaction = transaction_model
            .insert(&*self.db)
            .await
            .map_err(ServiceError::db_error)?;

        // Process payment with provider
        let processing_result = self.process_with_provider(&transaction, &provider).await;

        // Update transaction based on result
        let updated_transaction = match processing_result {
            Ok(charge_id) => {
                info!(
                    transaction_id = %transaction_id,
                    charge_id = %charge_id,
                    "Payment processed successfully"
                );

                stablepay_transaction::ActiveModel {
                    id: Set(transaction_id),
                    status: Set("succeeded".to_string()),
                    charge_id: Set(Some(charge_id)),
                    processed_at: Set(Some(Utc::now())),
                    updated_at: Set(Some(Utc::now())),
                    ..Default::default()
                }
                .update(&*self.db)
                .await
                .map_err(ServiceError::db_error)?
            }
            Err(e) => {
                error!(
                    transaction_id = %transaction_id,
                    error = ?e,
                    "Payment processing failed"
                );

                stablepay_transaction::ActiveModel {
                    id: Set(transaction_id),
                    status: Set("failed".to_string()),
                    failure_message: Set(Some(e.to_string())),
                    updated_at: Set(Some(Utc::now())),
                    ..Default::default()
                }
                .update(&*self.db)
                .await
                .map_err(ServiceError::db_error)?
            }
        };

        // Send event
        let event = Event::PaymentProcessed {
            transaction_id,
            order_id: request.order_id,
            customer_id: request.customer_id,
            amount: request.amount,
            currency: request.currency.clone(),
            status: updated_transaction.status.clone(),
        };

        if let Err(e) = self.event_sender.send(event).await {
            warn!(error = ?e, "Failed to send payment event");
        }

        Ok(PaymentResponse {
            id: updated_transaction.id,
            transaction_number: updated_transaction.transaction_number,
            order_id: updated_transaction.order_id,
            customer_id: updated_transaction.customer_id,
            amount: updated_transaction.amount,
            currency: updated_transaction.currency,
            status: updated_transaction.status,
            provider_name: provider.name,
            provider_fee: updated_transaction.provider_fee,
            platform_fee: updated_transaction.platform_fee,
            total_fees: updated_transaction.total_fees,
            net_amount: updated_transaction.net_amount,
            initiated_at: updated_transaction.initiated_at,
            processed_at: updated_transaction.processed_at,
            estimated_settlement_date: updated_transaction.estimated_settlement_date,
        })
    }

    /// Create a refund for a transaction
    #[instrument(skip(self, request))]
    pub async fn create_refund(
        &self,
        request: CreateRefundRequest,
    ) -> Result<RefundResponse, ServiceError> {
        request.validate()?;

        info!(
            transaction_id = %request.transaction_id,
            amount = %request.amount,
            "Creating refund"
        );

        // Get original transaction
        let transaction = stablepay_transaction::Entity::find_by_id(request.transaction_id)
            .one(&*self.db)
            .await
            .map_err(ServiceError::db_error)?
            .ok_or_else(|| ServiceError::NotFound("Transaction not found".to_string()))?;

        // Validate refund amount
        if request.amount > transaction.amount {
            return Err(ServiceError::ValidationError(
                "Refund amount exceeds transaction amount".to_string(),
            ));
        }

        // Calculate refunded fees (proportional)
        let refund_percentage = request.amount / transaction.amount;
        let refunded_fees = transaction.total_fees * refund_percentage;
        let net_refund = request.amount - refunded_fees;

        // Generate refund number
        let refund_number = self.generate_refund_number().await?;

        // Create refund record
        let refund_id = Uuid::new_v4();
        let now = Utc::now();

        let refund_model = stablepay_refund::ActiveModel {
            id: Set(refund_id),
            refund_number: Set(refund_number.clone()),
            transaction_id: Set(request.transaction_id),
            amount: Set(request.amount),
            currency: Set(transaction.currency.clone()),
            refunded_fees: Set(refunded_fees),
            net_refund: Set(net_refund),
            status: Set("processing".to_string()),
            refund_id_external: Set(None),
            reason: Set(request.reason),
            reason_detail: Set(request.reason_detail),
            requested_at: Set(now),
            processed_at: Set(None),
            failure_code: Set(None),
            failure_message: Set(None),
            metadata: Set(None),
            gateway_response: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
            created_by: Set(None),
        };

        let refund = refund_model
            .insert(&*self.db)
            .await
            .map_err(ServiceError::db_error)?;

        // Process refund with provider
        let refund_result = self
            .process_refund_with_provider(&transaction, &refund)
            .await;

        // Update refund based on result
        let updated_refund = match refund_result {
            Ok(external_id) => {
                info!(
                    refund_id = %refund_id,
                    external_id = %external_id,
                    "Refund processed successfully"
                );

                stablepay_refund::ActiveModel {
                    id: Set(refund_id),
                    status: Set("succeeded".to_string()),
                    refund_id_external: Set(Some(external_id)),
                    processed_at: Set(Some(Utc::now())),
                    updated_at: Set(Some(Utc::now())),
                    ..Default::default()
                }
                .update(&*self.db)
                .await
                .map_err(ServiceError::db_error)?
            }
            Err(e) => {
                error!(refund_id = %refund_id, error = ?e, "Refund processing failed");

                stablepay_refund::ActiveModel {
                    id: Set(refund_id),
                    status: Set("failed".to_string()),
                    failure_message: Set(Some(e.to_string())),
                    updated_at: Set(Some(Utc::now())),
                    ..Default::default()
                }
                .update(&*self.db)
                .await
                .map_err(ServiceError::db_error)?
            }
        };

        Ok(RefundResponse {
            id: updated_refund.id,
            refund_number: updated_refund.refund_number,
            transaction_id: updated_refund.transaction_id,
            amount: updated_refund.amount,
            currency: updated_refund.currency,
            status: updated_refund.status,
            refunded_fees: updated_refund.refunded_fees,
            net_refund: updated_refund.net_refund,
            requested_at: updated_refund.requested_at,
        })
    }

    /// Get payment by ID
    pub async fn get_payment(&self, id: Uuid) -> Result<PaymentResponse, ServiceError> {
        let transaction = stablepay_transaction::Entity::find_by_id(id)
            .one(&*self.db)
            .await
            .map_err(ServiceError::db_error)?
            .ok_or_else(|| ServiceError::NotFound("Payment not found".to_string()))?;

        self.payment_to_response(transaction).await
    }

    /// List payments for a customer
    pub async fn list_customer_payments(
        &self,
        customer_id: Uuid,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<PaymentResponse>, ServiceError> {
        let transactions = stablepay_transaction::Entity::find()
            .filter(stablepay_transaction::Column::CustomerId.eq(customer_id))
            .order_by_desc(stablepay_transaction::Column::CreatedAt)
            .limit(limit)
            .offset(offset)
            .all(&*self.db)
            .await
            .map_err(ServiceError::db_error)?;

        let mut responses = Vec::new();
        for transaction in transactions {
            responses.push(self.payment_to_response(transaction).await?);
        }

        Ok(responses)
    }

    // Private helper methods

    async fn select_optimal_provider(
        &self,
        currency: &str,
        amount: &Decimal,
    ) -> Result<stablepay_provider::Model, ServiceError> {
        // Get active providers that support the currency
        let providers = stablepay_provider::Entity::find()
            .filter(stablepay_provider::Column::IsActive.eq(true))
            .order_by_asc(stablepay_provider::Column::Priority)
            .all(&*self.db)
            .await
            .map_err(ServiceError::db_error)?;

        // Find best provider (lowest fees)
        let mut best_provider: Option<(stablepay_provider::Model, Decimal)> = None;

        for provider in providers {
            if provider.supports_currency(currency) {
                let fee = provider.calculate_fee(*amount);
                match best_provider {
                    None => best_provider = Some((provider, fee)),
                    Some((_, best_fee)) => {
                        if fee < best_fee {
                            best_provider = Some((provider, fee));
                        }
                    }
                }
            }
        }

        best_provider.map(|(p, _)| p).ok_or_else(|| {
            ServiceError::ValidationError(format!(
                "No payment provider available for currency: {}",
                currency
            ))
        })
    }

    fn calculate_platform_fee(&self, amount: Decimal) -> Decimal {
        // 0.5% platform fee
        amount * dec!(0.005)
    }

    async fn calculate_risk_score(
        &self,
        request: &CreatePaymentRequest,
    ) -> Result<Option<Decimal>, ServiceError> {
        // Simple risk scoring (in production, this would be more sophisticated)
        let mut score = Decimal::ZERO;

        // Large amounts are higher risk
        if request.amount > dec!(10000) {
            score += dec!(20);
        } else if request.amount > dec!(1000) {
            score += dec!(10);
        }

        // First-time customers are higher risk (would check transaction history)
        // This is simplified for demo
        score += dec!(15);

        Ok(Some(score))
    }

    async fn process_with_provider(
        &self,
        transaction: &stablepay_transaction::Model,
        provider: &stablepay_provider::Model,
    ) -> Result<String, ServiceError> {
        // Simulate payment processing with provider
        // In production, this would integrate with Stripe, PayPal, etc.

        info!(
            provider = %provider.name,
            transaction_id = %transaction.id,
            "Processing payment with provider"
        );

        // Simulate API call delay
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // 95% success rate for demo
        let success = rand::random::<f64>() > 0.05;

        if success {
            Ok(format!(
                "ch_{}",
                Uuid::new_v4().to_string().replace("-", "")
            ))
        } else {
            Err(ServiceError::ExternalApiError(
                "Payment declined by provider".to_string(),
            ))
        }
    }

    async fn process_refund_with_provider(
        &self,
        transaction: &stablepay_transaction::Model,
        refund: &stablepay_refund::Model,
    ) -> Result<String, ServiceError> {
        // Simulate refund processing
        info!(
            transaction_id = %transaction.id,
            refund_id = %refund.id,
            "Processing refund with provider"
        );

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        Ok(format!(
            "re_{}",
            Uuid::new_v4().to_string().replace("-", "")
        ))
    }

    async fn generate_transaction_number(&self) -> Result<String, ServiceError> {
        let timestamp = Utc::now().format("%Y%m%d");
        let random = Uuid::new_v4()
            .to_string()
            .split('-')
            .next()
            .unwrap()
            .to_uppercase();
        Ok(format!("PAY-{}-{}", timestamp, random))
    }

    async fn generate_refund_number(&self) -> Result<String, ServiceError> {
        let timestamp = Utc::now().format("%Y%m%d");
        let random = Uuid::new_v4()
            .to_string()
            .split('-')
            .next()
            .unwrap()
            .to_uppercase();
        Ok(format!("REF-{}-{}", timestamp, random))
    }

    async fn find_by_idempotency_key(
        &self,
        key: &str,
    ) -> Result<Option<stablepay_transaction::Model>, ServiceError> {
        stablepay_transaction::Entity::find()
            .filter(stablepay_transaction::Column::IdempotencyKey.eq(key))
            .one(&*self.db)
            .await
            .map_err(ServiceError::db_error)
    }

    async fn payment_to_response(
        &self,
        transaction: stablepay_transaction::Model,
    ) -> Result<PaymentResponse, ServiceError> {
        let provider = stablepay_provider::Entity::find_by_id(transaction.provider_id)
            .one(&*self.db)
            .await
            .map_err(ServiceError::db_error)?
            .ok_or_else(|| ServiceError::NotFound("Provider not found".to_string()))?;

        Ok(PaymentResponse {
            id: transaction.id,
            transaction_number: transaction.transaction_number,
            order_id: transaction.order_id,
            customer_id: transaction.customer_id,
            amount: transaction.amount,
            currency: transaction.currency,
            status: transaction.status,
            provider_name: provider.name,
            provider_fee: transaction.provider_fee,
            platform_fee: transaction.platform_fee,
            total_fees: transaction.total_fees,
            net_amount: transaction.net_amount,
            initiated_at: transaction.initiated_at,
            processed_at: transaction.processed_at,
            estimated_settlement_date: transaction.estimated_settlement_date,
        })
    }
}
