#[cfg(test)]
mod tests {
    use serde_json::json;
    use stateset_api::handlers::customers::RegisterCustomerRequest;
    use validator::Validate;

    #[tokio::test]
    #[ignore = "requires SQLite and Redis integration environment"]
    async fn test_customer_registration_flow() {
        // This is a basic integration test structure
        // In a real scenario, you'd set up a test database

        let request_body = json!({
            "email": "test@example.com",
            "first_name": "John",
            "last_name": "Doe",
            "password": "securepassword123",
            "phone": "+1234567890",
            "accepts_marketing": true
        });

        // Parse the request
        let register_request: RegisterCustomerRequest =
            serde_json::from_value(request_body).unwrap();

        // Validate the request
        assert!(register_request.validate().is_ok());
        assert_eq!(register_request.email, "test@example.com");
        assert_eq!(register_request.first_name, "John");
        assert_eq!(register_request.last_name, "Doe");
    }

    #[tokio::test]
    #[ignore = "requires SQLite and Redis integration environment"]
    async fn test_payment_request_validation() {
        use stateset_api::services::payments::*;

        let payment_request = ProcessPaymentRequest {
            order_id: uuid::Uuid::new_v4(),
            amount: rust_decimal::Decimal::new(10000, 2), // $100.00
            payment_method: PaymentMethod::CreditCard,
            payment_method_id: Some("pm_1234567890".to_string()),
            currency: "USD".to_string(),
            description: Some("Test payment".to_string()),
        };

        // Validate the payment request
        assert!(payment_request.validate().is_ok());
        assert_eq!(payment_request.currency, "USD");
        assert_eq!(payment_request.amount, rust_decimal::Decimal::new(10000, 2));
    }

    #[tokio::test]
    #[ignore = "requires SQLite and Redis integration environment"]
    async fn test_order_creation_request() {
        use stateset_api::handlers::orders::*;

        let request_body = json!({
            "customer_id": "550e8400-e29b-41d4-a716-446655440000",
            "items": [
                {
                    "product_id": "550e8400-e29b-41d4-a716-446655440001",
                    "quantity": 2
                }
            ],
            "shipping_address": {
                "street": "123 Main St",
                "city": "Anytown",
                "state": "CA",
                "postal_code": "12345",
                "country": "US"
            }
        });

        // Parse the request
        let create_request: CreateOrderRequest = serde_json::from_value(request_body).unwrap();

        // Validate the request
        assert!(create_request.validate().is_ok());
        assert_eq!(create_request.items.len(), 1);
        assert_eq!(create_request.items[0].quantity, 2);
    }
}
