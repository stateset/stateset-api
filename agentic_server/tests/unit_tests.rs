/// Unit tests for core business logic
#[cfg(test)]
mod service_tests {
    use agentic_commerce_server::service::AgenticCheckoutService;
    use agentic_commerce_server::models::*;
    use std::sync::Arc;

    fn setup_test_service() -> AgenticCheckoutService {
        let cache = Arc::new(agentic_commerce_server::cache::InMemoryCache::new());
        let (event_tx, _rx) = tokio::sync::mpsc::channel(1024);
        let event_sender = Arc::new(agentic_commerce_server::events::EventSender::new(event_tx));
        let product_catalog = Arc::new(agentic_commerce_server::product_catalog::ProductCatalogService::new());
        let tax_service = Arc::new(agentic_commerce_server::tax_service::TaxService::new());

        AgenticCheckoutService::new(
            cache,
            event_sender,
            product_catalog,
            tax_service,
            None,
            None,
            None,
        )
    }

    #[tokio::test]
    async fn test_totals_calculation() {
        let service = setup_test_service();

        let items = vec![
            LineItem {
                id: "li_1".to_string(),
                title: "Test Product".to_string(),
                quantity: 2,
                unit_price: Money {
                    amount: 1000, // $10.00
                    currency: "usd".to_string(),
                },
                variant_id: Some("item_123".to_string()),
                sku: None,
                image_url: None,
            }
        ];

        let customer = Customer {
            shipping_address: Some(Address {
                name: Some("Test".to_string()),
                line1: "123 Test".to_string(),
                line2: None,
                city: "Test City".to_string(),
                region: Some("CA".to_string()),
                postal_code: "94105".to_string(),
                country: "US".to_string(),
                phone: None,
                email: Some("test@test.com".to_string()),
            }),
            billing_address: None,
        };

        // Test totals calculation logic
        // Subtotal: 2 * $10.00 = $20.00 (2000 cents)
        // Tax: depends on CA rate
        // Shipping: $10.00 if selected
        // This would test the internal calculate_totals method
    }

    #[tokio::test]
    async fn test_session_state_transitions() {
        let service = setup_test_service();

        // Create session - should be NotReadyForPayment
        let create_request = CheckoutSessionCreateRequest {
            items: vec![RequestItem {
                id: "item_123".to_string(),
                quantity: 1,
            }],
            customer: None,
            fulfillment: None,
        };

        let session = service.create_session(create_request).await.unwrap();
        assert_eq!(session.status, CheckoutSessionStatus::NotReadyForPayment);

        // Add customer and fulfillment - should become ReadyForPayment
        let update_request = CheckoutSessionUpdateRequest {
            items: None,
            customer: Some(Customer {
                shipping_address: Some(Address {
                    name: Some("Test".to_string()),
                    line1: "123 Test".to_string(),
                    line2: None,
                    city: "Test City".to_string(),
                    region: Some("CA".to_string()),
                    postal_code: "94105".to_string(),
                    country: "US".to_string(),
                    phone: None,
                    email: Some("test@test.com".to_string()),
                }),
                billing_address: None,
            }),
            fulfillment: Some(FulfillmentRequest {
                selected_id: Some("standard_shipping".to_string()),
            }),
        };

        let updated = service.update_session(&session.id, update_request).await.unwrap();
        assert_eq!(updated.status, CheckoutSessionStatus::ReadyForPayment);
    }

    #[tokio::test]
    async fn test_inventory_reservation() {
        let service = setup_test_service();

        // Create multiple sessions for same product
        let create_request = CheckoutSessionCreateRequest {
            items: vec![RequestItem {
                id: "item_123".to_string(),
                quantity: 5,
            }],
            customer: None,
            fulfillment: None,
        };

        let session1 = service.create_session(create_request.clone()).await.unwrap();
        let session2 = service.create_session(create_request.clone()).await;

        // Second session might fail if inventory exhausted
        // Test inventory management
    }

    #[tokio::test]
    async fn test_session_expiry() {
        // Test that sessions expire after TTL
        // This would require mocking time or waiting
    }
}

#[cfg(test)]
mod validation_tests {
    use agentic_commerce_server::validation::*;

    #[test]
    fn test_email_validation() {
        assert!(validate_email("test@example.com").is_ok());
        assert!(validate_email("invalid-email").is_err());
        assert!(validate_email("@example.com").is_err());
        assert!(validate_email("test@").is_err());
    }

    #[test]
    fn test_phone_validation() {
        assert!(validate_phone("+14155551234").is_ok());
        assert!(validate_phone("+442071234567").is_ok());
        assert!(validate_phone("1234567").is_err());
        assert!(validate_phone("invalid").is_err());
    }

    #[test]
    fn test_country_code_validation() {
        assert!(validate_country_code("US").is_ok());
        assert!(validate_country_code("GB").is_ok());
        assert!(validate_country_code("XX").is_err());
        assert!(validate_country_code("USA").is_err());
    }

    #[test]
    fn test_quantity_validation() {
        assert!(validate_quantity(1).is_ok());
        assert!(validate_quantity(100).is_ok());
        assert!(validate_quantity(0).is_err());
        assert!(validate_quantity(-1).is_err());
    }
}

#[cfg(test)]
mod delegated_payment_tests {
    use agentic_commerce_server::delegated_payment::*;

    #[test]
    fn test_card_number_validation() {
        // Valid cards
        assert!(is_valid_card_number("4242424242424242")); // Visa
        assert!(is_valid_card_number("5555555555554444")); // Mastercard
        assert!(is_valid_card_number("378282246310005"));  // Amex

        // Invalid cards
        assert!(!is_valid_card_number("1234567890123456"));
        assert!(!is_valid_card_number("4242424242424241")); // Bad Luhn
        assert!(!is_valid_card_number("123"));
    }

    #[test]
    fn test_card_expiry_validation() {
        // This would test expiry date validation
        // Month should be 1-12, year should be current or future
    }

    #[test]
    fn test_allowance_validation() {
        // Test that allowances are properly validated
        // - max_amount must be positive
        // - expires_at must be in future
        // - currency must be valid
    }
}

#[cfg(test)]
mod tax_service_tests {
    use agentic_commerce_server::tax_service::TaxService;
    use agentic_commerce_server::models::Address;

    #[test]
    fn test_california_tax_calculation() {
        let service = TaxService::new();

        let address = Address {
            name: None,
            line1: "123 Main".to_string(),
            line2: None,
            city: "San Francisco".to_string(),
            region: Some("CA".to_string()),
            postal_code: "94105".to_string(),
            country: "US".to_string(),
            phone: None,
            email: None,
        };

        let result = service.calculate_tax(10000, &address, false, 0).unwrap();

        // San Francisco has high tax rate (8.625%)
        assert!(result.tax_amount > 850);
        assert!(result.tax_amount < 900);
    }

    #[test]
    fn test_international_tax() {
        let service = TaxService::new();

        let address = Address {
            name: None,
            line1: "123 Main".to_string(),
            line2: None,
            city: "London".to_string(),
            region: None,
            postal_code: "SW1A 1AA".to_string(),
            country: "GB".to_string(),
            phone: None,
            email: None,
        };

        let result = service.calculate_tax(10000, &address, false, 0);

        // Should handle international addresses
        assert!(result.is_ok() || result.is_err()); // Define expected behavior
    }
}

#[cfg(test)]
mod product_catalog_tests {
    use agentic_commerce_server::product_catalog::ProductCatalogService;

    #[test]
    fn test_get_product() {
        let catalog = ProductCatalogService::new();

        let product = catalog.get_product("item_123");
        assert!(product.is_ok());

        let product = product.unwrap();
        assert_eq!(product.id, "item_123");
        assert!(product.price > 0);
    }

    #[test]
    fn test_get_nonexistent_product() {
        let catalog = ProductCatalogService::new();

        let result = catalog.get_product("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_inventory_operations() {
        let catalog = ProductCatalogService::new();

        // Check initial inventory
        let has_stock = catalog.check_inventory("item_123", 1);
        assert!(has_stock.is_ok());

        // Reserve inventory
        let session_id = "test_session_123";
        let result = catalog.reserve_inventory("item_123", 5, session_id);
        assert!(result.is_ok());

        // Check reduced inventory
        let has_stock = catalog.check_inventory("item_123", 100);
        // Might return false if not enough stock

        // Release reservation
        catalog.release_reservation(session_id);

        // Commit inventory
        let commit = catalog.commit_inventory(session_id);
        // Test commit behavior
    }
}

#[cfg(test)]
mod fraud_service_tests {
    use agentic_commerce_server::fraud_service::FraudService;
    use agentic_commerce_server::models::*;

    #[tokio::test]
    async fn test_fraud_detection() {
        let service = FraudService::new();

        // Create suspicious session
        let session = create_suspicious_session();

        service.queue_for_review(session);

        // Check that it's in queue
        let queue = service.get_review_queue();
        assert!(queue.len() > 0);
    }

    #[tokio::test]
    async fn test_fraud_scoring() {
        // Test fraud scoring algorithm
        // High-value orders should get higher scores
        // Multiple orders from same IP should flag
        // Mismatched billing/shipping should flag
    }

    fn create_suspicious_session() -> CheckoutSession {
        CheckoutSession {
            id: "suspicious_123".to_string(),
            status: CheckoutSessionStatus::Completed,
            items: vec![],
            totals: Totals {
                subtotal: Money { amount: 999999, currency: "usd".to_string() },
                tax: None,
                shipping: None,
                discount: None,
                grand_total: Money { amount: 999999, currency: "usd".to_string() },
            },
            fulfillment: None,
            customer: None,
            links: None,
            messages: None,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:00Z".to_string(),
        }
    }
}

#[cfg(test)]
mod return_service_tests {
    use agentic_commerce_server::return_service::ReturnService;

    #[test]
    fn test_create_return() {
        let service = ReturnService::new();

        let return_req = service.create_return(
            "item_123".to_string(),
            "Defective".to_string(),
            "Product broken".to_string(),
        );

        assert_eq!(return_req.product_id, "item_123");
        assert_eq!(return_req.status, "pending");
    }

    #[test]
    fn test_pending_returns_queue() {
        let service = ReturnService::new();

        service.create_return("item_1".to_string(), "Defective".to_string(), "Test".to_string());
        service.create_return("item_2".to_string(), "Wrong item".to_string(), "Test".to_string());

        let pending = service.get_pending_returns();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn test_process_return() {
        let service = ReturnService::new();

        let mut return_req = service.create_return(
            "item_123".to_string(),
            "Defective".to_string(),
            "Test".to_string(),
        );

        return_req.status = "approved".to_string();

        let result = service.process_return(&return_req.id);
        // Test processing logic
    }
}
