mod common;

use chrono::Utc;
use common::TestApp;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use stateset_api::{
    entities::commerce::{cart, cart_item, product_variant},
    errors::ServiceError,
    services::commerce::{AddToCartInput, CartService, CreateCartInput},
};
use std::sync::Arc;
use uuid::Uuid;

/// Helper to create a test product variant
async fn setup_test_variant(app: &TestApp, price: Decimal) -> Uuid {
    let variant_id = Uuid::new_v4();
    let product_id = Uuid::new_v4();

    let variant = product_variant::ActiveModel {
        id: Set(variant_id),
        product_id: Set(product_id),
        sku: Set(format!("TEST-SKU-{}", variant_id)),
        name: Set("Test Product Variant".to_string()),
        price: Set(price),
        compare_at_price: Set(None),
        cost: Set(None),
        weight: Set(Some(1.5)),
        dimensions: Set(None),
        options: Set(serde_json::json!({})),
        inventory_tracking: Set(false),
        position: Set(1),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    };

    variant
        .insert(&*app.state.db)
        .await
        .expect("Failed to create test variant");

    variant_id
}

#[tokio::test]
#[cfg_attr(not(feature = "mock-tests"), ignore)]
async fn test_create_cart() {
    let app = TestApp::new().await;
    let cart_service = CartService::new(app.state.db.clone(), Arc::new(app.state.event_sender.clone()));

    let customer_id = Uuid::new_v4();
    let input = CreateCartInput {
        session_id: Some("test_session_123".to_string()),
        customer_id: Some(customer_id),
        currency: Some("USD".to_string()),
        metadata: None,
    };

    let cart = cart_service
        .create_cart(input)
        .await
        .expect("Failed to create cart");

    assert_eq!(cart.currency, "USD");
    assert_eq!(cart.customer_id, Some(customer_id));
    assert_eq!(cart.session_id, Some("test_session_123".to_string()));
    assert_eq!(cart.subtotal, Decimal::ZERO);
    assert_eq!(cart.total, Decimal::ZERO);
    assert_eq!(cart.status, cart::CartStatus::Active);
}

#[tokio::test]
#[cfg_attr(not(feature = "mock-tests"), ignore)]
async fn test_create_cart_with_defaults() {
    let app = TestApp::new().await;
    let cart_service = CartService::new(app.state.db.clone(), Arc::new(app.state.event_sender.clone()));

    let input = CreateCartInput {
        session_id: None,
        customer_id: None,
        currency: None, // Should default to USD
        metadata: None,
    };

    let cart = cart_service
        .create_cart(input)
        .await
        .expect("Failed to create cart");

    assert_eq!(cart.currency, "USD"); // Default currency
    assert_eq!(cart.customer_id, None);
    assert_eq!(cart.session_id, None);
}

#[tokio::test]
#[cfg_attr(not(feature = "mock-tests"), ignore)]
async fn test_add_item_to_cart() {
    let app = TestApp::new().await;
    let cart_service = CartService::new(app.state.db.clone(), Arc::new(app.state.event_sender.clone()));

    // Create cart
    let cart = cart_service
        .create_cart(CreateCartInput {
            session_id: Some("test".to_string()),
            customer_id: None,
            currency: Some("USD".to_string()),
            metadata: None,
        })
        .await
        .expect("Failed to create cart");

    // Create test variant
    let variant_id = setup_test_variant(&app, dec!(19.99)).await;

    // Add item to cart
    let updated_cart = cart_service
        .add_item(
            cart.id,
            AddToCartInput {
                variant_id,
                quantity: 2,
            },
        )
        .await
        .expect("Failed to add item to cart");

    // Verify cart totals
    assert_eq!(updated_cart.subtotal, dec!(39.98)); // 19.99 * 2
    assert_eq!(updated_cart.total, dec!(39.98));

    // Verify item was created
    let cart_with_items = cart_service
        .get_cart(cart.id)
        .await
        .expect("Failed to get cart");

    assert_eq!(cart_with_items.items.len(), 1);
    assert_eq!(cart_with_items.items[0].quantity, 2);
    assert_eq!(cart_with_items.items[0].unit_price, dec!(19.99));
    assert_eq!(cart_with_items.items[0].line_total, dec!(39.98));
}

#[tokio::test]
#[cfg_attr(not(feature = "mock-tests"), ignore)]
async fn test_add_existing_item_increments_quantity() {
    let app = TestApp::new().await;
    let cart_service = CartService::new(app.state.db.clone(), Arc::new(app.state.event_sender.clone()));

    let cart = cart_service
        .create_cart(CreateCartInput {
            session_id: Some("test".to_string()),
            customer_id: None,
            currency: Some("USD".to_string()),
            metadata: None,
        })
        .await
        .expect("Failed to create cart");

    let variant_id = setup_test_variant(&app, dec!(10.00)).await;

    // Add item first time
    cart_service
        .add_item(
            cart.id,
            AddToCartInput {
                variant_id,
                quantity: 2,
            },
        )
        .await
        .expect("Failed to add item");

    // Add same item again
    let updated_cart = cart_service
        .add_item(
            cart.id,
            AddToCartInput {
                variant_id,
                quantity: 3,
            },
        )
        .await
        .expect("Failed to add item again");

    // Verify quantity was incremented
    let cart_with_items = cart_service.get_cart(cart.id).await.unwrap();
    assert_eq!(cart_with_items.items.len(), 1); // Still only one item
    assert_eq!(cart_with_items.items[0].quantity, 5); // 2 + 3
    assert_eq!(updated_cart.subtotal, dec!(50.00)); // 10.00 * 5
}

#[tokio::test]
#[cfg_attr(not(feature = "mock-tests"), ignore)]
async fn test_add_item_to_inactive_cart_fails() {
    let app = TestApp::new().await;
    let cart_service = CartService::new(app.state.db.clone(), Arc::new(app.state.event_sender.clone()));

    let cart = cart_service
        .create_cart(CreateCartInput {
            session_id: Some("test".to_string()),
            customer_id: None,
            currency: Some("USD".to_string()),
            metadata: None,
        })
        .await
        .expect("Failed to create cart");

    // Abandon the cart
    cart_service.abandon_cart(cart.id).await.unwrap();

    let variant_id = setup_test_variant(&app, dec!(10.00)).await;

    // Try to add item to abandoned cart
    let result = cart_service
        .add_item(
            cart.id,
            AddToCartInput {
                variant_id,
                quantity: 1,
            },
        )
        .await;

    assert!(matches!(result, Err(ServiceError::InvalidOperation(_))));
}

#[tokio::test]
#[cfg_attr(not(feature = "mock-tests"), ignore)]
async fn test_update_item_quantity() {
    let app = TestApp::new().await;
    let cart_service = CartService::new(app.state.db.clone(), Arc::new(app.state.event_sender.clone()));

    let cart = cart_service
        .create_cart(CreateCartInput {
            session_id: Some("test".to_string()),
            customer_id: None,
            currency: Some("USD".to_string()),
            metadata: None,
        })
        .await
        .unwrap();

    let variant_id = setup_test_variant(&app, dec!(25.00)).await;

    cart_service
        .add_item(
            cart.id,
            AddToCartInput {
                variant_id,
                quantity: 2,
            },
        )
        .await
        .unwrap();

    let cart_with_items = cart_service.get_cart(cart.id).await.unwrap();
    let item_id = cart_with_items.items[0].id;

    // Update quantity to 5
    let updated_cart = cart_service
        .update_item_quantity(cart.id, item_id, 5)
        .await
        .expect("Failed to update quantity");

    assert_eq!(updated_cart.subtotal, dec!(125.00)); // 25.00 * 5

    let cart_with_items = cart_service.get_cart(cart.id).await.unwrap();
    assert_eq!(cart_with_items.items[0].quantity, 5);
}

#[tokio::test]
#[cfg_attr(not(feature = "mock-tests"), ignore)]
async fn test_update_item_quantity_to_zero_removes_item() {
    let app = TestApp::new().await;
    let cart_service = CartService::new(app.state.db.clone(), Arc::new(app.state.event_sender.clone()));

    let cart = cart_service
        .create_cart(CreateCartInput {
            session_id: Some("test".to_string()),
            customer_id: None,
            currency: Some("USD".to_string()),
            metadata: None,
        })
        .await
        .unwrap();

    let variant_id = setup_test_variant(&app, dec!(15.00)).await;

    cart_service
        .add_item(
            cart.id,
            AddToCartInput {
                variant_id,
                quantity: 2,
            },
        )
        .await
        .unwrap();

    let cart_with_items = cart_service.get_cart(cart.id).await.unwrap();
    let item_id = cart_with_items.items[0].id;

    // Update quantity to 0 (should remove item)
    let updated_cart = cart_service
        .update_item_quantity(cart.id, item_id, 0)
        .await
        .expect("Failed to remove item");

    assert_eq!(updated_cart.subtotal, Decimal::ZERO);

    let cart_with_items = cart_service.get_cart(cart.id).await.unwrap();
    assert_eq!(cart_with_items.items.len(), 0);
}

#[tokio::test]
#[cfg_attr(not(feature = "mock-tests"), ignore)]
async fn test_get_cart_not_found() {
    let app = TestApp::new().await;
    let cart_service = CartService::new(app.state.db.clone(), Arc::new(app.state.event_sender.clone()));

    let non_existent_id = Uuid::new_v4();
    let result = cart_service.get_cart(non_existent_id).await;

    assert!(matches!(result, Err(ServiceError::NotFound(_))));
}

#[tokio::test]
#[cfg_attr(not(feature = "mock-tests"), ignore)]
async fn test_clear_cart() {
    let app = TestApp::new().await;
    let cart_service = CartService::new(app.state.db.clone(), Arc::new(app.state.event_sender.clone()));

    let cart = cart_service
        .create_cart(CreateCartInput {
            session_id: Some("test".to_string()),
            customer_id: None,
            currency: Some("USD".to_string()),
            metadata: None,
        })
        .await
        .unwrap();

    // Add multiple items
    let variant1 = setup_test_variant(&app, dec!(10.00)).await;
    let variant2 = setup_test_variant(&app, dec!(20.00)).await;

    cart_service
        .add_item(
            cart.id,
            AddToCartInput {
                variant_id: variant1,
                quantity: 2,
            },
        )
        .await
        .unwrap();

    cart_service
        .add_item(
            cart.id,
            AddToCartInput {
                variant_id: variant2,
                quantity: 1,
            },
        )
        .await
        .unwrap();

    // Verify items exist
    let cart_before = cart_service.get_cart(cart.id).await.unwrap();
    assert_eq!(cart_before.items.len(), 2);

    // Clear cart
    cart_service
        .clear_cart(cart.id)
        .await
        .expect("Failed to clear cart");

    // Verify cart is empty and totals are zero
    let cart_after = cart_service.get_cart(cart.id).await.unwrap();
    assert_eq!(cart_after.items.len(), 0);
    assert_eq!(cart_after.cart.subtotal, Decimal::ZERO);
    assert_eq!(cart_after.cart.total, Decimal::ZERO);
}

#[tokio::test]
#[cfg_attr(not(feature = "mock-tests"), ignore)]
async fn test_abandon_cart() {
    let app = TestApp::new().await;
    let cart_service = CartService::new(app.state.db.clone(), Arc::new(app.state.event_sender.clone()));

    let cart = cart_service
        .create_cart(CreateCartInput {
            session_id: Some("test".to_string()),
            customer_id: Some(Uuid::new_v4()),
            currency: Some("USD".to_string()),
            metadata: None,
        })
        .await
        .unwrap();

    assert_eq!(cart.status, cart::CartStatus::Active);

    let abandoned_cart = cart_service
        .abandon_cart(cart.id)
        .await
        .expect("Failed to abandon cart");

    assert_eq!(abandoned_cart.status, cart::CartStatus::Abandoned);
}

#[tokio::test]
#[cfg_attr(not(feature = "mock-tests"), ignore)]
async fn test_list_carts_for_customer() {
    let app = TestApp::new().await;
    let cart_service = CartService::new(app.state.db.clone(), Arc::new(app.state.event_sender.clone()));

    let customer_id = Uuid::new_v4();

    // Create 3 carts for customer
    for _ in 0..3 {
        cart_service
            .create_cart(CreateCartInput {
                session_id: None,
                customer_id: Some(customer_id),
                currency: Some("USD".to_string()),
                metadata: None,
            })
            .await
            .unwrap();
    }

    // Create 1 cart for different customer
    cart_service
        .create_cart(CreateCartInput {
            session_id: None,
            customer_id: Some(Uuid::new_v4()),
            currency: Some("USD".to_string()),
            metadata: None,
        })
        .await
        .unwrap();

    // List carts for first customer
    let (carts, total) = cart_service
        .list_carts_for_customer(customer_id, 1, 10)
        .await
        .expect("Failed to list carts");

    assert_eq!(carts.len(), 3);
    assert_eq!(total, 3);

    // Verify all carts belong to customer
    for cart in carts {
        assert_eq!(cart.customer_id, Some(customer_id));
    }
}

#[tokio::test]
#[cfg_attr(not(feature = "mock-tests"), ignore)]
async fn test_cart_with_multiple_items_calculates_correctly() {
    let app = TestApp::new().await;
    let cart_service = CartService::new(app.state.db.clone(), Arc::new(app.state.event_sender.clone()));

    let cart = cart_service
        .create_cart(CreateCartInput {
            session_id: Some("test".to_string()),
            customer_id: None,
            currency: Some("USD".to_string()),
            metadata: None,
        })
        .await
        .unwrap();

    // Add multiple different items
    let variant1 = setup_test_variant(&app, dec!(10.50)).await;
    let variant2 = setup_test_variant(&app, dec!(25.75)).await;
    let variant3 = setup_test_variant(&app, dec!(5.25)).await;

    cart_service
        .add_item(
            cart.id,
            AddToCartInput {
                variant_id: variant1,
                quantity: 2,
            },
        )
        .await
        .unwrap();

    cart_service
        .add_item(
            cart.id,
            AddToCartInput {
                variant_id: variant2,
                quantity: 1,
            },
        )
        .await
        .unwrap();

    let updated_cart = cart_service
        .add_item(
            cart.id,
            AddToCartInput {
                variant_id: variant3,
                quantity: 4,
            },
        )
        .await
        .unwrap();

    // Calculate expected total: (10.50 * 2) + (25.75 * 1) + (5.25 * 4)
    // = 21.00 + 25.75 + 21.00 = 67.75
    assert_eq!(updated_cart.subtotal, dec!(67.75));

    let cart_with_items = cart_service.get_cart(cart.id).await.unwrap();
    assert_eq!(cart_with_items.items.len(), 3);
}
