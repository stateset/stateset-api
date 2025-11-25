mod common;

use chrono::Utc;
use common::TestApp;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use stateset_api::{
    models::{stablepay_payment_method, stablepay_provider, stablepay_transaction},
    errors::ServiceError,
    services::stablepay_service::{CreatePaymentRequest, CreateRefundRequest, StablePayService},
};
use std::sync::Arc;
use uuid::Uuid;

// Helper to create a test provider
async fn setup_test_provider(app: &TestApp) -> Uuid {
    let provider_id = Uuid::new_v4();
    let provider = stablepay_provider::ActiveModel {
        id: Set(provider_id),
        name: Set("TestProvider".to_string()),
        provider_type: Set("bank".to_string()),
        is_active: Set(true),
        base_fee: Set(dec!(0.50)),
        percentage_fee: Set(dec!(0.029)),
        min_fee: Set(Some(dec!(0.10))),
        max_fee: Set(Some(dec!(10.00))),
        supported_currencies: Set(Some(vec!["USD".to_string(), "EUR".to_string()])),
        priority: Set(1),
        routing_rules: Set(None),
        config: Set(None),
        created_at: Set(Utc::now()),
        updated_at: Set(Some(Utc::now())),
        created_by: Set(None),
    };

    provider.insert(&*app.state.db).await.expect("Failed to create test provider");
    provider_id
}

// Helper to create a test customer
async fn setup_test_customer(_app: &TestApp) -> Uuid {
    Uuid::new_v4() // In real tests, you'd insert a customer record
}

#[tokio::test]
async fn test_create_payment_success() {
    let app = TestApp::new().await;
    let service = StablePayService::new(app.state.db.clone(), app.state.event_sender.clone());

    let _provider_id = setup_test_provider(&app).await;
    let customer_id = setup_test_customer(&app).await;

    let request = CreatePaymentRequest {
        order_id: None,
        customer_id,
        payment_method_id: None,
        amount: dec!(100.00),
        currency: "USD".to_string(),
        description: Some("Test payment".to_string()),
        metadata: None,
        idempotency_key: Some("test-key-001".to_string()),
    };

    let result = service.create_payment(request).await;

    assert!(result.is_ok(), "Payment creation should succeed");
    let payment = result.unwrap();

    assert_eq!(payment.amount, dec!(100.00));
    assert_eq!(payment.currency, "USD");
    assert_eq!(payment.customer_id, customer_id);
    assert!(!payment.transaction_number.is_empty());
    assert!(payment.provider_fee >= Decimal::ZERO);
    assert!(payment.platform_fee >= Decimal::ZERO);
    assert_eq!(payment.total_fees, payment.provider_fee + payment.platform_fee);
    assert_eq!(payment.net_amount, payment.amount - payment.total_fees);
}

#[tokio::test]
async fn test_create_payment_validation_negative_amount() {
    let app = TestApp::new().await;
    let service = StablePayService::new(app.state.db.clone(), app.state.event_sender.clone());

    let customer_id = setup_test_customer(&app).await;

    let request = CreatePaymentRequest {
        order_id: None,
        customer_id,
        payment_method_id: None,
        amount: dec!(-10.00), // Negative amount
        currency: "USD".to_string(),
        description: None,
        metadata: None,
        idempotency_key: None,
    };

    let result = service.create_payment(request).await;

    assert!(result.is_err(), "Should reject negative amount");
    match result.unwrap_err() {
        ServiceError::ValidationError(_) => {}, // Expected
        e => panic!("Expected ValidationError, got: {:?}", e),
    }
}

#[tokio::test]
async fn test_create_payment_validation_zero_amount() {
    let app = TestApp::new().await;
    let service = StablePayService::new(app.state.db.clone(), app.state.event_sender.clone());

    let customer_id = setup_test_customer(&app).await;

    let request = CreatePaymentRequest {
        order_id: None,
        customer_id,
        payment_method_id: None,
        amount: Decimal::ZERO, // Zero amount
        currency: "USD".to_string(),
        description: None,
        metadata: None,
        idempotency_key: None,
    };

    let result = service.create_payment(request).await;

    assert!(result.is_err(), "Should reject zero amount");
    match result.unwrap_err() {
        ServiceError::ValidationError(_) => {}, // Expected
        e => panic!("Expected ValidationError, got: {:?}", e),
    }
}

#[tokio::test]
async fn test_create_payment_validation_invalid_currency() {
    let app = TestApp::new().await;
    let service = StablePayService::new(app.state.db.clone(), app.state.event_sender.clone());

    let customer_id = setup_test_customer(&app).await;

    let request = CreatePaymentRequest {
        order_id: None,
        customer_id,
        payment_method_id: None,
        amount: dec!(100.00),
        currency: "INVALID".to_string(), // Invalid currency code (not 3 chars)
        description: None,
        metadata: None,
        idempotency_key: None,
    };

    let result = service.create_payment(request).await;

    assert!(result.is_err(), "Should reject invalid currency");
}

#[tokio::test]
async fn test_create_payment_idempotency() {
    let app = TestApp::new().await;
    let service = StablePayService::new(app.state.db.clone(), app.state.event_sender.clone());

    let _provider_id = setup_test_provider(&app).await;
    let customer_id = setup_test_customer(&app).await;

    let idempotency_key = "idempotent-key-123";

    let request1 = CreatePaymentRequest {
        order_id: None,
        customer_id,
        payment_method_id: None,
        amount: dec!(100.00),
        currency: "USD".to_string(),
        description: Some("First request".to_string()),
        metadata: None,
        idempotency_key: Some(idempotency_key.to_string()),
    };

    let request2 = CreatePaymentRequest {
        order_id: None,
        customer_id,
        payment_method_id: None,
        amount: dec!(200.00), // Different amount
        currency: "USD".to_string(),
        description: Some("Second request".to_string()),
        metadata: None,
        idempotency_key: Some(idempotency_key.to_string()), // Same key
    };

    // First request
    let result1 = service.create_payment(request1).await;
    assert!(result1.is_ok(), "First payment should succeed");
    let payment1 = result1.unwrap();

    // Second request with same idempotency key should return first payment
    let result2 = service.create_payment(request2).await;
    assert!(result2.is_ok(), "Second payment should succeed (idempotent)");
    let payment2 = result2.unwrap();

    // Should return the same payment
    assert_eq!(payment1.id, payment2.id, "Should return same payment ID");
    assert_eq!(payment1.amount, payment2.amount, "Should return original amount");
    assert_eq!(payment1.transaction_number, payment2.transaction_number);
}

#[tokio::test]
async fn test_create_payment_fee_calculation() {
    let app = TestApp::new().await;
    let service = StablePayService::new(app.state.db.clone(), app.state.event_sender.clone());

    let _provider_id = setup_test_provider(&app).await;
    let customer_id = setup_test_customer(&app).await;

    let request = CreatePaymentRequest {
        order_id: None,
        customer_id,
        payment_method_id: None,
        amount: dec!(1000.00),
        currency: "USD".to_string(),
        description: None,
        metadata: None,
        idempotency_key: Some("fee-test-001".to_string()),
    };

    let result = service.create_payment(request).await;
    assert!(result.is_ok());
    let payment = result.unwrap();

    // Verify fee calculations
    assert!(payment.provider_fee > Decimal::ZERO, "Provider fee should be positive");
    assert!(payment.platform_fee >= Decimal::ZERO, "Platform fee should be non-negative");
    assert_eq!(payment.total_fees, payment.provider_fee + payment.platform_fee);
    assert_eq!(payment.net_amount, payment.amount - payment.total_fees);

    // Net amount should always be less than gross amount
    assert!(payment.net_amount < payment.amount, "Net amount should be less than gross");
}

#[tokio::test]
async fn test_create_refund_success() {
    let app = TestApp::new().await;
    let service = StablePayService::new(app.state.db.clone(), app.state.event_sender.clone());

    let _provider_id = setup_test_provider(&app).await;
    let customer_id = setup_test_customer(&app).await;

    // First create a payment
    let payment_request = CreatePaymentRequest {
        order_id: None,
        customer_id,
        payment_method_id: None,
        amount: dec!(100.00),
        currency: "USD".to_string(),
        description: None,
        metadata: None,
        idempotency_key: Some("refund-test-payment".to_string()),
    };

    let payment = service.create_payment(payment_request).await.unwrap();

    // Now create a refund
    let refund_request = CreateRefundRequest {
        transaction_id: payment.id,
        amount: dec!(50.00), // Partial refund
        reason: Some("Customer request".to_string()),
        reason_detail: Some("Changed mind".to_string()),
    };

    let result = service.create_refund(refund_request).await;

    assert!(result.is_ok(), "Refund creation should succeed");
    let refund = result.unwrap();

    assert_eq!(refund.amount, dec!(50.00));
    assert_eq!(refund.transaction_id, payment.id);
    assert!(!refund.refund_number.is_empty());
    assert!(refund.refunded_fees >= Decimal::ZERO);
    assert_eq!(refund.net_refund, refund.amount - refund.refunded_fees);
}

#[tokio::test]
async fn test_create_refund_full_amount() {
    let app = TestApp::new().await;
    let service = StablePayService::new(app.state.db.clone(), app.state.event_sender.clone());

    let _provider_id = setup_test_provider(&app).await;
    let customer_id = setup_test_customer(&app).await;

    // Create a payment
    let payment_request = CreatePaymentRequest {
        order_id: None,
        customer_id,
        payment_method_id: None,
        amount: dec!(100.00),
        currency: "USD".to_string(),
        description: None,
        metadata: None,
        idempotency_key: Some("full-refund-test".to_string()),
    };

    let payment = service.create_payment(payment_request).await.unwrap();

    // Full refund
    let refund_request = CreateRefundRequest {
        transaction_id: payment.id,
        amount: dec!(100.00), // Full amount
        reason: Some("Order cancelled".to_string()),
        reason_detail: None,
    };

    let result = service.create_refund(refund_request).await;

    assert!(result.is_ok(), "Full refund should succeed");
    let refund = result.unwrap();

    assert_eq!(refund.amount, dec!(100.00));
}

#[tokio::test]
async fn test_create_refund_exceeds_amount() {
    let app = TestApp::new().await;
    let service = StablePayService::new(app.state.db.clone(), app.state.event_sender.clone());

    let _provider_id = setup_test_provider(&app).await;
    let customer_id = setup_test_customer(&app).await;

    // Create a payment
    let payment_request = CreatePaymentRequest {
        order_id: None,
        customer_id,
        payment_method_id: None,
        amount: dec!(100.00),
        currency: "USD".to_string(),
        description: None,
        metadata: None,
        idempotency_key: Some("exceed-refund-test".to_string()),
    };

    let payment = service.create_payment(payment_request).await.unwrap();

    // Try to refund more than payment amount
    let refund_request = CreateRefundRequest {
        transaction_id: payment.id,
        amount: dec!(150.00), // More than payment
        reason: Some("Invalid refund".to_string()),
        reason_detail: None,
    };

    let result = service.create_refund(refund_request).await;

    assert!(result.is_err(), "Should reject refund exceeding payment amount");
    match result.unwrap_err() {
        ServiceError::ValidationError(msg) => {
            assert!(msg.contains("exceeds"), "Error should mention exceeding amount");
        }
        e => panic!("Expected ValidationError, got: {:?}", e),
    }
}

#[tokio::test]
async fn test_create_refund_validation_negative_amount() {
    let app = TestApp::new().await;
    let service = StablePayService::new(app.state.db.clone(), app.state.event_sender.clone());

    let transaction_id = Uuid::new_v4(); // Non-existent transaction

    let refund_request = CreateRefundRequest {
        transaction_id,
        amount: dec!(-10.00), // Negative amount
        reason: None,
        reason_detail: None,
    };

    let result = service.create_refund(refund_request).await;

    assert!(result.is_err(), "Should reject negative refund amount");
}

#[tokio::test]
async fn test_create_refund_nonexistent_transaction() {
    let app = TestApp::new().await;
    let service = StablePayService::new(app.state.db.clone(), app.state.event_sender.clone());

    let transaction_id = Uuid::new_v4(); // Non-existent transaction

    let refund_request = CreateRefundRequest {
        transaction_id,
        amount: dec!(50.00),
        reason: Some("Test".to_string()),
        reason_detail: None,
    };

    let result = service.create_refund(refund_request).await;

    assert!(result.is_err(), "Should reject refund for non-existent transaction");
    match result.unwrap_err() {
        ServiceError::NotFound(_) => {}, // Expected
        e => panic!("Expected NotFound error, got: {:?}", e),
    }
}

#[tokio::test]
async fn test_get_payment_success() {
    let app = TestApp::new().await;
    let service = StablePayService::new(app.state.db.clone(), app.state.event_sender.clone());

    let _provider_id = setup_test_provider(&app).await;
    let customer_id = setup_test_customer(&app).await;

    // Create a payment
    let payment_request = CreatePaymentRequest {
        order_id: None,
        customer_id,
        payment_method_id: None,
        amount: dec!(100.00),
        currency: "USD".to_string(),
        description: Some("Get test".to_string()),
        metadata: None,
        idempotency_key: Some("get-payment-test".to_string()),
    };

    let created_payment = service.create_payment(payment_request).await.unwrap();

    // Retrieve the payment
    let result = service.get_payment(created_payment.id).await;

    assert!(result.is_ok(), "Should retrieve payment successfully");
    let retrieved_payment = result.unwrap();

    assert_eq!(retrieved_payment.id, created_payment.id);
    assert_eq!(retrieved_payment.amount, created_payment.amount);
    assert_eq!(retrieved_payment.currency, created_payment.currency);
    assert_eq!(retrieved_payment.transaction_number, created_payment.transaction_number);
}

#[tokio::test]
async fn test_get_payment_not_found() {
    let app = TestApp::new().await;
    let service = StablePayService::new(app.state.db.clone(), app.state.event_sender.clone());

    let non_existent_id = Uuid::new_v4();
    let result = service.get_payment(non_existent_id).await;

    assert!(result.is_err(), "Should return error for non-existent payment");
    match result.unwrap_err() {
        ServiceError::NotFound(_) => {}, // Expected
        e => panic!("Expected NotFound error, got: {:?}", e),
    }
}

#[tokio::test]
async fn test_list_customer_payments() {
    let app = TestApp::new().await;
    let service = StablePayService::new(app.state.db.clone(), app.state.event_sender.clone());

    let _provider_id = setup_test_provider(&app).await;
    let customer_id = setup_test_customer(&app).await;

    // Create multiple payments for the customer
    for i in 0..5 {
        let request = CreatePaymentRequest {
            order_id: None,
            customer_id,
            payment_method_id: None,
            amount: dec!(100.00) + Decimal::from(i),
            currency: "USD".to_string(),
            description: Some(format!("Payment {}", i)),
            metadata: None,
            idempotency_key: Some(format!("list-test-{}", i)),
        };

        service.create_payment(request).await.unwrap();
    }

    // List payments
    let result = service.list_customer_payments(customer_id, 10, 0).await;

    assert!(result.is_ok(), "Should list payments successfully");
    let payments = result.unwrap();

    assert_eq!(payments.len(), 5, "Should return 5 payments");

    // Verify all payments belong to the customer
    for payment in payments {
        assert_eq!(payment.customer_id, customer_id);
    }
}

#[tokio::test]
async fn test_list_customer_payments_with_pagination() {
    let app = TestApp::new().await;
    let service = StablePayService::new(app.state.db.clone(), app.state.event_sender.clone());

    let _provider_id = setup_test_provider(&app).await;
    let customer_id = setup_test_customer(&app).await;

    // Create 10 payments
    for i in 0..10 {
        let request = CreatePaymentRequest {
            order_id: None,
            customer_id,
            payment_method_id: None,
            amount: dec!(100.00),
            currency: "USD".to_string(),
            description: None,
            metadata: None,
            idempotency_key: Some(format!("pagination-test-{}", i)),
        };

        service.create_payment(request).await.unwrap();
    }

    // Get first page (limit 5)
    let page1 = service.list_customer_payments(customer_id, 5, 0).await.unwrap();
    assert_eq!(page1.len(), 5, "First page should have 5 payments");

    // Get second page (limit 5, offset 5)
    let page2 = service.list_customer_payments(customer_id, 5, 5).await.unwrap();
    assert_eq!(page2.len(), 5, "Second page should have 5 payments");

    // Verify no duplicate IDs
    let page1_ids: Vec<Uuid> = page1.iter().map(|p| p.id).collect();
    let page2_ids: Vec<Uuid> = page2.iter().map(|p| p.id).collect();

    for id in &page1_ids {
        assert!(!page2_ids.contains(id), "Pages should not have duplicate payments");
    }
}

#[tokio::test]
async fn test_list_customer_payments_empty() {
    let app = TestApp::new().await;
    let service = StablePayService::new(app.state.db.clone(), app.state.event_sender.clone());

    let customer_id = Uuid::new_v4(); // Customer with no payments

    let result = service.list_customer_payments(customer_id, 10, 0).await;

    assert!(result.is_ok(), "Should succeed even with no payments");
    let payments = result.unwrap();

    assert_eq!(payments.len(), 0, "Should return empty list");
}

#[tokio::test]
async fn test_concurrent_payment_creation() {
    let app = TestApp::new().await;
    let service = Arc::new(StablePayService::new(
        app.state.db.clone(),
        app.state.event_sender.clone(),
    ));

    let _provider_id = setup_test_provider(&app).await;
    let customer_id = setup_test_customer(&app).await;

    // Create 10 payments concurrently
    let mut handles: Vec<tokio::task::JoinHandle<Result<_, ServiceError>>> = vec![];

    for i in 0..10 {
        let service_clone = service.clone();
        let handle = tokio::spawn(async move {
            let request = CreatePaymentRequest {
                order_id: None,
                customer_id,
                payment_method_id: None,
                amount: dec!(100.00),
                currency: "USD".to_string(),
                description: Some(format!("Concurrent payment {}", i)),
                metadata: None,
                idempotency_key: Some(format!("concurrent-{}", i)),
            };

            service_clone.create_payment(request).await
        });

        handles.push(handle);
    }

    // Wait for all to complete
    let results = futures::future::join_all(handles).await;

    // All should succeed
    for (i, result) in results.iter().enumerate() {
        assert!(result.is_ok(), "Handle {} should succeed", i);
        let payment_result = result.as_ref().unwrap();
        assert!(payment_result.is_ok(), "Payment {} should succeed", i);
    }
}
