use crate::{
    errors::ServiceError,
    events::{Event, EventSender},
    models::payment,
};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    PaginatorTrait,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, instrument};
use uuid::Uuid;
use rust_decimal::Decimal;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaymentMethod {
    CreditCard,
    DebitCard,
    PayPal,
    BankTransfer,
    Cash,
    Check,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ProcessPaymentRequest {
    pub order_id: Uuid,
    #[validate(range(min = 0.01, message = "Amount must be greater than 0"))]
    pub amount: Decimal,
    pub payment_method: PaymentMethod,
    pub payment_method_id: Option<String>,
    pub currency: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RefundPaymentRequest {
    pub payment_id: Uuid,
    #[validate(range(min = 0.01, message = "Refund amount must be greater than 0"))]
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

        // Create payment record
        let payment_model = payment::ActiveModel {
            id: Set(Uuid::new_v4()),
            order_id: Set(request.order_id),
            amount: Set(request.amount),
            status: Set(Some(status.to_string())),
            created_at: Set(Utc::now()),
        };

        let payment = payment_model.insert(&*self.db).await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        // Send event
        let event = Event::PaymentAuthorized(payment.id);
        self.event_sender.send(event).await
            .map_err(|e| ServiceError::EventError(e.to_string()))?;

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
            currency: request.currency,
            status: status.to_string(),
            payment_method: format!("{:?}", request.payment_method),
            payment_method_id: request.payment_method_id,
            description: request.description,
            created_at: payment.created_at,
            processed_at: Some(Utc::now()),
        })
    }

    /// Get payment by ID
    #[instrument(skip(self))]
    pub async fn get_payment(&self, payment_id: Uuid) -> Result<PaymentResponse, ServiceError> {
        let payment = payment::Entity::find_by_id(payment_id)
            .one(&*self.db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?
            .ok_or_else(|| ServiceError::NotFound(format!("Payment {} not found", payment_id)))?;

        Ok(PaymentResponse {
            id: payment.id,
            order_id: payment.order_id,
            amount: payment.amount,
            currency: "USD".to_string(), // Default currency
            status: payment.status.unwrap_or_else(|| "unknown".to_string()),
            payment_method: "unknown".to_string(), // Would be stored in separate table
            payment_method_id: None,
            description: None,
            created_at: payment.created_at,
            processed_at: None,
        })
    }

    /// Get payments for an order
    #[instrument(skip(self))]
    pub async fn get_order_payments(&self, order_id: Uuid) -> Result<Vec<PaymentResponse>, ServiceError> {
        let payments = payment::Entity::find()
            .filter(payment::Column::OrderId.eq(order_id))
            .order_by_desc(payment::Column::CreatedAt)
            .all(&*self.db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        let responses = payments.into_iter()
            .map(|payment| PaymentResponse {
                id: payment.id,
                order_id: payment.order_id,
                amount: payment.amount,
                currency: "USD".to_string(),
                status: payment.status.unwrap_or_else(|| "unknown".to_string()),
                payment_method: "unknown".to_string(),
                payment_method_id: None,
                description: None,
                created_at: payment.created_at,
                processed_at: None,
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
            return Err(ServiceError::ValidationError("Page number must be greater than 0".to_string()));
        }

        if limit == 0 || limit > 1000 {
            return Err(ServiceError::ValidationError("Limit must be between 1 and 1000".to_string()));
        }

        let mut query = payment::Entity::find();

        if let Some(status) = status_filter {
            query = query.filter(payment::Column::Status.eq(status.to_string()));
        }

        let paginator = query
            .order_by_desc(payment::Column::CreatedAt)
            .paginate(&*self.db, limit);

        let total = paginator.num_items().await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        let payments = paginator.fetch_page(page - 1).await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        let responses = payments.into_iter()
            .map(|payment| PaymentResponse {
                id: payment.id,
                order_id: payment.order_id,
                amount: payment.amount,
                currency: "USD".to_string(),
                status: payment.status.unwrap_or_else(|| "unknown".to_string()),
                payment_method: "unknown".to_string(),
                payment_method_id: None,
                description: None,
                created_at: payment.created_at,
                processed_at: None,
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
        validator::Validate::validate(&request)?;

        // Get original payment
        let original_payment = self.get_payment(request.payment_id).await?;

        if original_payment.status != "succeeded" {
            return Err(ServiceError::ValidationError("Only successful payments can be refunded".to_string()));
        }

        let refund_amount = request.amount.unwrap_or(original_payment.amount);

        if refund_amount > original_payment.amount {
            return Err(ServiceError::ValidationError("Refund amount cannot exceed original payment amount".to_string()));
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
            amount: Set(-refund_amount), // Negative amount for refund
            status: Set(Some(PaymentStatus::Refunded.to_string())),
            created_at: Set(Utc::now()),
        };

        let refund = refund_payment.insert(&*self.db).await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        // Send refund event
        let event = Event::PaymentRefunded(refund.id);
        self.event_sender.send(event).await
            .map_err(|e| ServiceError::EventError(e.to_string()))?;

        info!(
            refund_id = %refund.id,
            original_payment_id = %request.payment_id,
            amount = %refund_amount,
            "Refund processed successfully"
        );

        Ok(PaymentResponse {
            id: refund.id,
            order_id: refund.order_id,
            amount: refund.amount,
            currency: "USD".to_string(),
            status: PaymentStatus::Refunded.to_string(),
            payment_method: "refund".to_string(),
            payment_method_id: None,
            description: request.reason,
            created_at: refund.created_at,
            processed_at: Some(Utc::now()),
        })
    }

    /// Simulate payment processing (replace with real payment gateway integration)
    async fn simulate_payment_processing(&self, _request: &ProcessPaymentRequest) -> Result<PaymentStatus, ServiceError> {
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
            .map_err(|e| ServiceError::DatabaseError(e))?;

        let total = payments.iter()
            .map(|payment| payment.amount)
            .sum()
            .unwrap_or(Decimal::ZERO);

        Ok(total)
    }
}
