mod common;

use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{ActiveModelTrait, EntityTrait, ModelTrait, Set};
use stateset_api::entities::{order::Entity as OrderEntity, order_item};
use stateset_api::models::{
    invoice_line_item, invoices, order as legacy_order, order_line_item, payment,
    shipment::{self, ShipmentStatus, ShippingCarrier, ShippingMethod},
    shipment_item,
};
use uuid::Uuid;

#[tokio::test]
async fn relationship_smoke_test() {
    let app = common::TestApp::new().await;
    let db = &*app.state.db;

    let order_service = app.state.services.order.clone();
    let order_response = order_service
        .create_order_minimal(
            Uuid::new_v4(),
            Decimal::new(12_345, 2),
            Some("USD".to_string()),
            Some("integration relationship test".to_string()),
            Some("123 Shipping Ln".to_string()),
            Some("987 Billing Rd".to_string()),
            Some("card".to_string()),
        )
        .await
        .expect("create base order");
    let order_id = order_response.id;

    // Insert modern order item entity
    let order_item_active = order_item::ActiveModel {
        id: Set(Uuid::new_v4()),
        order_id: Set(order_id),
        product_id: Set(Uuid::new_v4()),
        sku: Set("SKU-TEST".to_string()),
        name: Set("Test Order Item".to_string()),
        quantity: Set(2),
        unit_price: Set(Decimal::new(5_000, 2)),
        total_price: Set(Decimal::new(10_000, 2)),
        discount: Set(Decimal::ZERO),
        tax_rate: Set(Decimal::ZERO),
        tax_amount: Set(Decimal::ZERO),
        status: Set("pending".to_string()),
        notes: Set(Some("relational test line".to_string())),
        ..Default::default()
    };
    order_item_active
        .insert(db)
        .await
        .expect("insert order item");

    // Insert legacy order line item
    let legacy_line_item = order_line_item::ActiveModel {
        id: Set(Uuid::new_v4()),
        order_id: Set(order_id),
        product_name: Set("Legacy Line Item".to_string()),
        quantity: Set(1),
        sale_price: Set(5_000),
        original_price: Set(6_000),
        seller_discount: Set(1_000),
        unit: Set("each".to_string()),
        product_id: Set("PROD-123".to_string()),
        brand: Set("Acme".to_string()),
        stock_code: Set("STK-1".to_string()),
        size: Set("M".to_string()),
        seller_sku: Set("SELLER-SKU".to_string()),
        sku_id: Set("SKU-LEGACY".to_string()),
        sku_image: Set("https://example.com/sku.png".to_string()),
        sku_name: Set("Legacy SKU".to_string()),
        sku_type: Set("standard".to_string()),
        created_date: Set(Utc::now()),
        updated_date: Set(None),
        status: Set(order_line_item::OrderLineItemStatus::Pending),
        ..Default::default()
    };
    legacy_line_item
        .insert(db)
        .await
        .expect("insert order_line_item");

    // Insert shipment and shipment item
    let shipment_id = Uuid::new_v4();
    let shipment_active = shipment::ActiveModel {
        id: Set(shipment_id),
        order_id: Set(order_id),
        tracking_number: Set("TRACK-12345".to_string()),
        carrier: Set(ShippingCarrier::UPS),
        status: Set(ShipmentStatus::Processing),
        shipping_address: Set("123 Shipping Ln".to_string()),
        shipping_method: Set(ShippingMethod::Standard),
        weight_kg: Set(Some(1.5)),
        dimensions_cm: Set(Some("10x10x10".to_string())),
        notes: Set(Some("test shipment".to_string())),
        shipped_at: Set(None),
        estimated_delivery: Set(None),
        delivered_at: Set(None),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
        created_by: Set(Some("tester".to_string())),
        recipient_name: Set("Jane Doe".to_string()),
        recipient_email: Set(Some("jane@example.com".to_string())),
        recipient_phone: Set(Some("555-0100".to_string())),
        tracking_url: Set(None),
        shipping_cost: Set(Some(15.0)),
        insurance_amount: Set(Some(50.0)),
        is_signature_required: Set(false),
        ..Default::default()
    };
    shipment_active.insert(db).await.expect("insert shipment");

    let shipment_item_active = shipment_item::ActiveModel {
        id: Set(Uuid::new_v4()),
        shipment_id: Set(shipment_id),
        product_id: Set(Uuid::new_v4()),
        quantity: Set(1),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    };
    shipment_item_active
        .insert(db)
        .await
        .expect("insert shipment item");

    // Insert invoice and invoice line
    let invoice_id = Uuid::new_v4().to_string();
    let invoice_active = invoices::ActiveModel {
        id: Set(invoice_id.clone()),
        order_id: Set(Some(order_id.to_string())),
        customer_name: Set(Some("Jane Doe".to_string())),
        customer_email: Set(Some("jane@example.com".to_string())),
        created: Set(Some(Utc::now())),
        currency: Set(Some("USD".to_string())),
        status: Set(Some("draft".to_string())),
        subtotal: Set(Some(Decimal::new(10_000, 2))),
        total: Set(Some(Decimal::new(10_500, 2))),
        amount_due: Set(Some(Decimal::new(10_500, 2))),
        amount_paid: Set(Some(Decimal::ZERO)),
        amount_remaining: Set(Some(Decimal::new(10_500, 2))),
        tax_amount: Set(Some(Decimal::new(500, 2))),
        notes: Set(Some("invoice test".to_string())),
        ..Default::default()
    };
    invoice_active.insert(db).await.expect("insert invoice");

    let invoice_line_active = invoice_line_item::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        invoice_id: Set(invoice_id.clone()),
        description: Set("Invoice line".to_string()),
        quantity: Set(Decimal::new(2, 0)),
        unit_price: Set(Decimal::new(5_000, 2)),
        amount: Set(Decimal::new(10_000, 2)),
        product_id: Set(Some("PROD-123".to_string())),
        sku: Set(Some("SKU-LEGACY".to_string())),
        tax_rate: Set(Some(Decimal::new(500, 2))),
        tax_amount: Set(Some(Decimal::new(500, 2))),
        discount_amount: Set(None),
        discount_type: Set(None),
        notes: Set(Some("invoice line for relationship test".to_string())),
    };
    invoice_line_active
        .insert(db)
        .await
        .expect("insert invoice line");

    // Insert payment
    let payment_active = payment::ActiveModel {
        id: Set(Uuid::new_v4()),
        order_id: Set(order_id),
        amount: Set(Decimal::new(10_500, 2)),
        currency: Set("USD".to_string()),
        payment_method: Set("CreditCard".to_string()),
        payment_method_id: Set(Some("pm_123".to_string())),
        status: Set("succeeded".to_string()),
        description: Set(Some("Test payment".to_string())),
        transaction_id: Set(Some("txn_123".to_string())),
        gateway_response: Set(None),
        refunded_amount: Set(Decimal::ZERO),
        refund_reason: Set(None),
        created_at: Set(Utc::now()),
        updated_at: Set(Some(Utc::now())),
        processed_at: Set(Some(Utc::now())),
    };
    payment_active.insert(db).await.expect("insert payment");

    // Assert relationships through modern order entity
    let order_entity = OrderEntity::find_by_id(order_id)
        .one(db)
        .await
        .expect("fetch order")
        .expect("order exists");

    let order_items = order_entity
        .find_related(order_item::Entity)
        .all(db)
        .await
        .expect("order -> order_items");
    assert_eq!(order_items.len(), 1);

    let shipments = order_entity
        .find_related(shipment::Entity)
        .all(db)
        .await
        .expect("order -> shipments");
    assert_eq!(shipments.len(), 1);

    let shipment_items = shipments[0]
        .find_related(shipment_item::Entity)
        .all(db)
        .await
        .expect("shipment -> shipment_items");
    assert_eq!(shipment_items.len(), 1);

    // Assert relationships through legacy order model
    let legacy_order_model = legacy_order::Entity::find_by_id(order_id)
        .one(db)
        .await
        .expect("fetch legacy order")
        .expect("legacy order exists");

    let legacy_line_items = legacy_order_model
        .find_related(order_line_item::Entity)
        .all(db)
        .await
        .expect("order -> order_line_items");
    assert_eq!(legacy_line_items.len(), 1);

    let invoices = legacy_order_model
        .find_related(invoices::Entity)
        .all(db)
        .await
        .expect("order -> invoices");
    assert_eq!(invoices.len(), 1);

    let invoice_lines = invoices[0]
        .find_related(invoice_line_item::Entity)
        .all(db)
        .await
        .expect("invoice -> invoice_line_items");
    assert_eq!(invoice_lines.len(), 1);

    let payments = legacy_order_model
        .find_related(payment::Entity)
        .all(db)
        .await
        .expect("order -> payments");
    assert_eq!(payments.len(), 1);

    let payment_orders = payments[0]
        .find_related(legacy_order::Entity)
        .all(db)
        .await
        .expect("payment -> order");
    assert_eq!(payment_orders.len(), 1);
    assert_eq!(payment_orders[0].id, order_id);
}
