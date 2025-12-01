use crate::{
    errors::ServiceError,
    events::{Event, EventSender},
    models::payment,
};
use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, instrument};
use utoipa::ToSchema;
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

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum PaymentStatus {
    Pending,
    Processing,
    Succeeded,
    Failed,
    Cancelled,
    Refunded,
}

impl std::fmt::Display for PaymentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PaymentStatus::Pending => write!(f, "pending"),
            PaymentStatus::Processing => write!(f, "processing"),
            PaymentStatus::Succeeded => write!(f, "succeeded"),
            PaymentStatus::Failed => write!(f, "failed"),
            PaymentStatus::Cancelled => write!(f, "cancelled"),
            PaymentStatus::Refunded => write!(f, "refunded"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum PaymentMethod {
    CreditCard,
    DebitCard,
    PayPal,
    BankTransfer,
    Cash,
    Check,
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct ProcessPaymentRequest {
    pub order_id: Uuid,
    #[validate(custom = "validate_positive_decimal")]
    pub amount: Decimal,
    pub payment_method: PaymentMethod,
    pub payment_method_id: Option<String>,
    pub currency: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaymentResponse {
    pub id: Uuid,
    pub order_id: Uuid,
    pub amount: Decimal,
    pub currency: String,
    pub status: String,
    pub payment_method: String,
    pub payment_method_id: Option<String>,
    pub description: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
    pub processed_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RefundPaymentRequest {
    pub payment_id: Uuid,
    pub amount: Option<Decimal>,
    pub reason: Option<String>,
}

/// Enhanced Payment Service
pub struct PaymentService {
    db: Arc<DatabaseConnection>,
    event_sender: Arc<EventSender>,
}

impl PaymentService {
    pub fn new(db: Arc<DatabaseConnection>, event_sender: Arc<EventSender>) -> Self {
        Self { db, event_sender }
    }

    /// Process a payment for an order
    #[instrument(skip(self, request))]
    pub async fn process_payment(
        &self,
        request: ProcessPaymentRequest,
    ) -> Result<PaymentResponse, ServiceError> {
        validator::Validate::validate(&request)?;

        info!(
            order_id = %request.order_id,
            amount = %request.amount,
            method = ?request.payment_method,
            "Processing payment"
        );

        // Simulate payment processing (in real implementation, this would call payment gateway)
        let status = self.simulate_payment_processing(&request).await?;

        // Create payment record aligned with entity
        let payment_id = Uuid::new_v4();
        let payment_model = payment::ActiveModel {
            id: Set(payment_id),
            order_id: Set(request.order_id),
            amount: Set(request.amount),
            currency: Set(request.currency.clone()),
            payment_method: Set(format!("{:?}", request.payment_method)),
            payment_method_id: Set(request.payment_method_id.clone()),
            status: Set(status.to_string()),
            description: Set(request.description.clone()),
            transaction_id: Set(None),
            gateway_response: Set(None),
            refunded_amount: Set(Decimal::ZERO),
            refund_reason: Set(None),
            created_at: Set(Utc::now()),
            updated_at: Set(Some(Utc::now())),
            processed_at: Set(Some(Utc::now())),
        };

        let payment = payment_model
            .insert(&*self.db)
            .await
            .map_err(ServiceError::db_error)?;

        // Send event
        let event = if matches!(status, PaymentStatus::Succeeded) {
            Event::PaymentCaptured(payment.id)
        } else if matches!(status, PaymentStatus::Failed) {
            Event::PaymentFailed(payment.id)
        } else {
            Event::PaymentAuthorized(payment.id)
        };
        self.event_sender
            .send(event)
            .await
            .map_err(|e| ServiceError::EventError(e.to_string()))?;

        // Outbox enqueue (best-effort)
        let _ = crate::events::outbox::enqueue(
            &*self.db,
            "payment",
            Some(payment.id),
            match status {
                PaymentStatus::Succeeded => "PaymentSucceeded",
                PaymentStatus::Failed => "PaymentFailed",
                _ => "PaymentAuthorized",
            },
            &serde_json::json!({
                "payment_id": payment.id.to_string(),
                "order_id": payment.order_id.to_string(),
                "amount": payment.amount,
                "status": payment.status,
            }),
        )
        .await;

        info!(
            payment_id = %payment.id,
            order_id = %request.order_id,
            status = %status,
            "Payment processed successfully"
        );

        Ok(PaymentResponse {
            id: payment.id,
            order_id: payment.order_id,
            amount: payment.amount,
            currency: payment.currency,
            status: payment.status,
            payment_method: payment.payment_method,
            payment_method_id: payment.payment_method_id,
            description: payment.description,
            created_at: payment.created_at,
            processed_at: payment.processed_at,
        })
    }

    /// Get payment by ID
    #[instrument(skip(self))]
    pub async fn get_payment(&self, payment_id: Uuid) -> Result<PaymentResponse, ServiceError> {
        let payment = payment::Entity::find_by_id(payment_id)
            .one(&*self.db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| ServiceError::NotFound(format!("Payment {} not found", payment_id)))?;

        Ok(PaymentResponse {
            id: payment.id,
            order_id: payment.order_id,
            amount: payment.amount,
            currency: payment.currency,
            status: payment.status,
            payment_method: payment.payment_method,
            payment_method_id: payment.payment_method_id,
            description: payment.description,
            created_at: payment.created_at,
            processed_at: payment.processed_at,
        })
    }

    /// Get payments for an order
    #[instrument(skip(self))]
    pub async fn get_order_payments(
        &self,
        order_id: Uuid,
    ) -> Result<Vec<PaymentResponse>, ServiceError> {
        let payments = payment::Entity::find()
            .filter(payment::Column::OrderId.eq(order_id))
            .order_by_desc(payment::Column::CreatedAt)
            .all(&*self.db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let responses = payments
            .into_iter()
            .map(|payment| PaymentResponse {
                id: payment.id,
                order_id: payment.order_id,
                amount: payment.amount,
                currency: payment.currency,
                status: payment.status,
                payment_method: payment.payment_method,
                payment_method_id: payment.payment_method_id,
                description: payment.description,
                created_at: payment.created_at,
                processed_at: payment.processed_at,
            })
            .collect();

        Ok(responses)
    }

    /// List payments with pagination
    #[instrument(skip(self))]
    pub async fn list_payments(
        &self,
        page: u64,
        limit: u64,
        status_filter: Option<PaymentStatus>,
    ) -> Result<(Vec<PaymentResponse>, u64), ServiceError> {
        if page == 0 {
            return Err(ServiceError::ValidationError(
                "Page number must be greater than 0".to_string(),
            ));
        }

        if limit == 0 || limit > 1000 {
            return Err(ServiceError::ValidationError(
                "Limit must be between 1 and 1000".to_string(),
            ));
        }

        let mut query = payment::Entity::find();

        if let Some(status) = status_filter {
            query = query.filter(payment::Column::Status.eq(status.to_string()));
        }

        let paginator = query
            .order_by_desc(payment::Column::CreatedAt)
            .paginate(&*self.db, limit);

        let total = paginator
            .num_items()
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let payments = paginator
            .fetch_page(page - 1)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let responses = payments
            .into_iter()
            .map(|payment| PaymentResponse {
                id: payment.id,
                order_id: payment.order_id,
                amount: payment.amount,
                currency: payment.currency,
                status: payment.status,
                payment_method: payment.payment_method,
                payment_method_id: payment.payment_method_id,
                description: payment.description,
                created_at: payment.created_at,
                processed_at: payment.processed_at,
            })
            .collect();

        Ok((responses, total))
    }

    /// Refund a payment
    #[instrument(skip(self, request))]
    pub async fn refund_payment(
        &self,
        request: RefundPaymentRequest,
    ) -> Result<PaymentResponse, ServiceError> {
        // Get original payment
        let original_payment = self.get_payment(request.payment_id).await?;

        if original_payment.status != "succeeded" {
            return Err(ServiceError::ValidationError(
                "Only successful payments can be refunded".to_string(),
            ));
        }

        let refund_amount = request.amount.unwrap_or(original_payment.amount);

        if refund_amount <= Decimal::ZERO {
            return Err(ServiceError::ValidationError(
                "Refund amount must be greater than zero".to_string(),
            ));
        }

        if refund_amount > original_payment.amount {
            return Err(ServiceError::ValidationError(
                "Refund amount cannot exceed original payment amount".to_string(),
            ));
        }

        info!(
            payment_id = %request.payment_id,
            amount = %refund_amount,
            "Processing refund"
        );

        // In real implementation, this would call payment gateway for refund
        let refund_payment = payment::ActiveModel {
            id: Set(Uuid::new_v4()),
            order_id: Set(original_payment.order_id),
            amount: Set(-refund_amount),
            currency: Set(original_payment.currency),
            payment_method: Set(original_payment.payment_method),
            payment_method_id: Set(original_payment.payment_method_id),
            status: Set(PaymentStatus::Refunded.to_string()),
            description: Set(request.reason.clone()),
            transaction_id: Set(None),
            gateway_response: Set(None),
            refunded_amount: Set(-refund_amount),
            refund_reason: Set(request.reason.clone()),
            created_at: Set(Utc::now()),
            updated_at: Set(Some(Utc::now())),
            processed_at: Set(Some(Utc::now())),
        };

        let refund = refund_payment
            .insert(&*self.db)
            .await
            .map_err(ServiceError::db_error)?;

        // Send refund event
        let event = Event::PaymentRefunded(refund.id);
        self.event_sender
            .send(event)
            .await
            .map_err(|e| ServiceError::EventError(e.to_string()))?;

        info!(
            refund_id = %refund.id,
            original_payment_id = %request.payment_id,
            amount = %refund_amount,
            "Refund processed successfully"
        );

        // Outbox: PaymentRefunded
        let _ = crate::events::outbox::enqueue(
            &*self.db,
            "payment",
            Some(refund.id),
            "PaymentRefunded",
            &serde_json::json!({"payment_id": refund.id.to_string(), "order_id": refund.order_id.to_string(), "amount": refund.amount}),
        )
        .await;

        Ok(PaymentResponse {
            id: refund.id,
            order_id: refund.order_id,
            amount: refund.amount,
            currency: refund.currency,
            status: refund.status,
            payment_method: refund.payment_method,
            payment_method_id: refund.payment_method_id,
            description: refund.description,
            created_at: refund.created_at,
            processed_at: refund.processed_at,
        })
    }

    /// Simulate payment processing (replace with real payment gateway integration)
    async fn simulate_payment_processing(
        &self,
        _request: &ProcessPaymentRequest,
    ) -> Result<PaymentStatus, ServiceError> {
        // Simulate payment processing delay
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Simulate 95% success rate
        if rand::random::<f32>() < 0.95 {
            Ok(PaymentStatus::Succeeded)
        } else {
            Ok(PaymentStatus::Failed)
        }
    }

    /// Calculate total payments for an order
    pub async fn get_order_total_payments(&self, order_id: Uuid) -> Result<Decimal, ServiceError> {
        let payments = payment::Entity::find()
            .filter(payment::Column::OrderId.eq(order_id))
            .filter(payment::Column::Status.eq(PaymentStatus::Succeeded.to_string()))
            .all(&*self.db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let mut total = Decimal::ZERO;
        for p in payments {
            total += p.amount;
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    // ==================== PaymentStatus Tests ====================

    #[test]
    fn test_payment_status_display() {
        assert_eq!(PaymentStatus::Pending.to_string(), "pending");
        assert_eq!(PaymentStatus::Processing.to_string(), "processing");
        assert_eq!(PaymentStatus::Succeeded.to_string(), "succeeded");
        assert_eq!(PaymentStatus::Failed.to_string(), "failed");
        assert_eq!(PaymentStatus::Cancelled.to_string(), "cancelled");
        assert_eq!(PaymentStatus::Refunded.to_string(), "refunded");
    }

    // ==================== Validation Tests ====================

    #[test]
    fn test_validate_positive_decimal_with_positive_value() {
        let amount = dec!(100.50);
        assert!(validate_positive_decimal(&amount).is_ok());
    }

    #[test]
    fn test_validate_positive_decimal_with_zero() {
        let amount = Decimal::ZERO;
        let result = validate_positive_decimal(&amount);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, std::borrow::Cow::Borrowed("range"));
    }

    #[test]
    fn test_validate_positive_decimal_with_negative() {
        let amount = dec!(-50.00);
        let result = validate_positive_decimal(&amount);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_positive_decimal_with_small_positive() {
        let amount = dec!(0.01);
        assert!(validate_positive_decimal(&amount).is_ok());
    }

    // ==================== ProcessPaymentRequest Validation Tests ====================

    #[test]
    fn test_process_payment_request_valid() {
        let request = ProcessPaymentRequest {
            order_id: Uuid::new_v4(),
            amount: dec!(99.99),
            payment_method: PaymentMethod::CreditCard,
            payment_method_id: Some("pm_123".to_string()),
            currency: "USD".to_string(),
            description: Some("Test payment".to_string()),
        };
        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_process_payment_request_zero_amount_fails() {
        let request = ProcessPaymentRequest {
            order_id: Uuid::new_v4(),
            amount: Decimal::ZERO,
            payment_method: PaymentMethod::CreditCard,
            payment_method_id: None,
            currency: "USD".to_string(),
            description: None,
        };
        assert!(request.validate().is_err());
    }

    #[test]
    fn test_process_payment_request_negative_amount_fails() {
        let request = ProcessPaymentRequest {
            order_id: Uuid::new_v4(),
            amount: dec!(-100.00),
            payment_method: PaymentMethod::PayPal,
            payment_method_id: None,
            currency: "EUR".to_string(),
            description: None,
        };
        assert!(request.validate().is_err());
    }

    #[test]
    fn test_process_payment_request_large_amount_valid() {
        let request = ProcessPaymentRequest {
            order_id: Uuid::new_v4(),
            amount: dec!(999999.99),
            payment_method: PaymentMethod::BankTransfer,
            payment_method_id: None,
            currency: "USD".to_string(),
            description: Some("Large payment".to_string()),
        };
        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_process_payment_request_minimal_amount_valid() {
        let request = ProcessPaymentRequest {
            order_id: Uuid::new_v4(),
            amount: dec!(0.01),
            payment_method: PaymentMethod::Cash,
            payment_method_id: None,
            currency: "USD".to_string(),
            description: None,
        };
        assert!(request.validate().is_ok());
    }

    // ==================== PaymentMethod Tests ====================

    #[test]
    fn test_payment_method_debug_format() {
        assert_eq!(format!("{:?}", PaymentMethod::CreditCard), "CreditCard");
        assert_eq!(format!("{:?}", PaymentMethod::DebitCard), "DebitCard");
        assert_eq!(format!("{:?}", PaymentMethod::PayPal), "PayPal");
        assert_eq!(format!("{:?}", PaymentMethod::BankTransfer), "BankTransfer");
        assert_eq!(format!("{:?}", PaymentMethod::Cash), "Cash");
        assert_eq!(format!("{:?}", PaymentMethod::Check), "Check");
    }

    // ==================== PaymentResponse Tests ====================

    #[test]
    fn test_payment_response_serialization() {
        let response = PaymentResponse {
            id: Uuid::new_v4(),
            order_id: Uuid::new_v4(),
            amount: dec!(150.00),
            currency: "USD".to_string(),
            status: "succeeded".to_string(),
            payment_method: "CreditCard".to_string(),
            payment_method_id: Some("pm_test_123".to_string()),
            description: Some("Order payment".to_string()),
            created_at: Utc::now(),
            processed_at: Some(Utc::now()),
        };

        let json = serde_json::to_string(&response).expect("serialization should succeed");
        assert!(json.contains("\"amount\":\"150.00\"") || json.contains("\"amount\":150"));
        assert!(json.contains("\"currency\":\"USD\""));
        assert!(json.contains("\"status\":\"succeeded\""));
    }

    #[test]
    fn test_payment_response_deserialization() {
        let json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "order_id": "550e8400-e29b-41d4-a716-446655440001",
            "amount": "100.50",
            "currency": "EUR",
            "status": "pending",
            "payment_method": "PayPal",
            "payment_method_id": null,
            "description": null,
            "created_at": "2024-01-15T10:30:00Z",
            "processed_at": null
        }"#;

        let response: PaymentResponse = serde_json::from_str(json).expect("deserialization should succeed");
        assert_eq!(response.currency, "EUR");
        assert_eq!(response.status, "pending");
        assert_eq!(response.payment_method, "PayPal");
        assert!(response.payment_method_id.is_none());
        assert!(response.processed_at.is_none());
    }

    // ==================== RefundPaymentRequest Tests ====================

    #[test]
    fn test_refund_request_full_refund() {
        let request = RefundPaymentRequest {
            payment_id: Uuid::new_v4(),
            amount: None, // Full refund
            reason: Some("Customer requested refund".to_string()),
        };

        let json = serde_json::to_string(&request).expect("serialization should succeed");
        assert!(json.contains("\"amount\":null"));
        assert!(json.contains("Customer requested refund"));
    }

    #[test]
    fn test_refund_request_partial_refund() {
        let request = RefundPaymentRequest {
            payment_id: Uuid::new_v4(),
            amount: Some(dec!(25.00)),
            reason: Some("Partial item return".to_string()),
        };

        assert!(request.amount.is_some());
        assert_eq!(request.amount.unwrap(), dec!(25.00));
    }

    #[test]
    fn test_refund_request_no_reason() {
        let request = RefundPaymentRequest {
            payment_id: Uuid::new_v4(),
            amount: Some(dec!(10.00)),
            reason: None,
        };

        assert!(request.reason.is_none());
    }

    // ==================== Edge Case Tests ====================

    #[test]
    fn test_decimal_precision() {
        // Test that decimal arithmetic maintains precision
        let amount1 = dec!(33.33);
        let amount2 = dec!(33.33);
        let amount3 = dec!(33.34);
        let total = amount1 + amount2 + amount3;
        assert_eq!(total, dec!(100.00));
    }

    #[test]
    fn test_currency_codes() {
        // Common currency codes should be accepted
        let currencies = vec!["USD", "EUR", "GBP", "JPY", "CAD", "AUD"];
        for currency in currencies {
            let request = ProcessPaymentRequest {
                order_id: Uuid::new_v4(),
                amount: dec!(100.00),
                payment_method: PaymentMethod::CreditCard,
                payment_method_id: None,
                currency: currency.to_string(),
                description: None,
            };
            assert!(request.validate().is_ok(), "Currency {} should be valid", currency);
        }
    }

    #[test]
    fn test_payment_method_serialization_roundtrip() {
        let methods = vec![
            PaymentMethod::CreditCard,
            PaymentMethod::DebitCard,
            PaymentMethod::PayPal,
            PaymentMethod::BankTransfer,
            PaymentMethod::Cash,
            PaymentMethod::Check,
        ];

        for method in methods {
            let json = serde_json::to_string(&method).expect("serialize payment method");
            let deserialized: PaymentMethod = serde_json::from_str(&json).expect("deserialize payment method");
            assert_eq!(format!("{:?}", method), format!("{:?}", deserialized));
        }
    }

    #[test]
    fn test_payment_status_serialization_roundtrip() {
        let statuses = vec![
            PaymentStatus::Pending,
            PaymentStatus::Processing,
            PaymentStatus::Succeeded,
            PaymentStatus::Failed,
            PaymentStatus::Cancelled,
            PaymentStatus::Refunded,
        ];

        for status in statuses {
            let json = serde_json::to_string(&status).expect("serialize payment status");
            let deserialized: PaymentStatus = serde_json::from_str(&json).expect("deserialize payment status");
            assert_eq!(status.to_string(), deserialized.to_string());
        }
    }

    #[test]
    fn test_uuid_uniqueness() {
        // Ensure UUIDs are unique for each payment
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_process_payment_request_with_all_payment_methods() {
        let methods = vec![
            PaymentMethod::CreditCard,
            PaymentMethod::DebitCard,
            PaymentMethod::PayPal,
            PaymentMethod::BankTransfer,
            PaymentMethod::Cash,
            PaymentMethod::Check,
        ];

        for method in methods {
            let request = ProcessPaymentRequest {
                order_id: Uuid::new_v4(),
                amount: dec!(50.00),
                payment_method: method,
                payment_method_id: None,
                currency: "USD".to_string(),
                description: None,
            };
            assert!(request.validate().is_ok());
        }
    }
}
