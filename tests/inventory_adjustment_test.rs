use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use stateset_api::{
    db::{create_db_pool, run_migrations},
    entities::{
        inventory_balance::{self, Entity as InventoryBalance},
        inventory_location::{self, Entity as InventoryLocation},
        inventory_transaction::{self, Entity as InventoryTransaction},
        item_master::{self, Entity as ItemMaster},
        purchase_order_headers::{self, Entity as PurchaseOrderHeader},
        purchase_order_lines::{self, Entity as PurchaseOrderLine},
        sales_order_header::{self, Entity as SalesOrderHeader},
        sales_order_line::{self, Entity as SalesOrderLine},
    },
    events::EventSender,
    services::inventory_adjustment_service::{
        InventoryAdjustmentService, PurchaseOrderReceiptLine, SalesOrderAdjustmentType,
    },
};
use std::{env, sync::Arc};
use tokio::sync::mpsc;
use uuid::Uuid;

fn dec(value: i32) -> Decimal {
    Decimal::new((value as i64) * 10_000, 4)
}

fn dec_zero() -> Decimal {
    dec(0)
}

#[tokio::test]
async fn test_inventory_adjustments_with_item_master() {
    env::set_var("APP__DATABASE_URL", "sqlite::memory:?cache=shared");

    // Setup database connection
    let db_pool = Arc::new(create_db_pool().await.expect("Failed to create DB pool"));
    run_migrations(db_pool.as_ref())
        .await
        .expect("Failed to run migrations");
    let db = db_pool.as_ref();

    // Setup event sender
    let (tx, _rx) = mpsc::channel(100);
    let event_sender = Arc::new(EventSender::new(tx));

    // Create inventory adjustment service
    let service = InventoryAdjustmentService::new(db_pool.clone(), event_sender.clone());

    // Start transaction for test isolation
    let txn = db.begin().await.expect("Failed to begin transaction");

    // Step 1: Create test data - Item Master
    println!("Creating item master records...");
    let item1 = create_test_item(&txn, "LAPTOP-001", "Dell Laptop XPS 13").await;
    let item2 = create_test_item(&txn, "MOUSE-001", "Logitech MX Master 3").await;
    let item3 = create_test_item(&txn, "KEYBOARD-001", "Mechanical Keyboard RGB").await;

    // Step 2: Create warehouse locations
    println!("Creating warehouse locations...");
    let location1 = create_test_location(&txn, "MAIN-WH", "Main Warehouse").await;
    let location2 = create_test_location(&txn, "SECONDARY-WH", "Secondary Warehouse").await;

    // Step 3: Initialize inventory balances
    println!("Initializing inventory balances...");
    create_initial_inventory(&txn, item1.inventory_item_id, location1.location_id, 100).await;
    create_initial_inventory(&txn, item2.inventory_item_id, location1.location_id, 500).await;
    create_initial_inventory(&txn, item3.inventory_item_id, location1.location_id, 200).await;

    // Step 4: Test Sales Order Allocation
    println!("\n=== Testing Sales Order Allocation ===");
    let sales_order = create_test_sales_order(&txn).await;
    let so_line1 = create_sales_order_line(
        &txn,
        sales_order.header_id,
        item1.inventory_item_id,
        location1.location_id,
        2,
    )
    .await;
    let so_line2 = create_sales_order_line(
        &txn,
        sales_order.header_id,
        item2.inventory_item_id,
        location1.location_id,
        10,
    )
    .await;

    // Allocate inventory for sales order
    let allocation_results = service
        .adjust_for_sales_order(sales_order.header_id, SalesOrderAdjustmentType::Allocate)
        .await
        .expect("Failed to allocate inventory");

    println!("Allocation Results:");
    for result in &allocation_results {
        println!(
            "  Item {}: Allocated {} units",
            result.item_id, result.quantity_adjusted
        );
        println!(
            "    New Available: {}, Allocated: {}",
            result.new_available, result.new_allocated
        );
    }

    // Verify allocation
    let inv1_after_alloc =
        get_inventory_balance(&txn, item1.inventory_item_id, location1.location_id).await;
    assert_eq!(inv1_after_alloc.quantity_available, dec(98));
    assert_eq!(inv1_after_alloc.quantity_allocated, dec(2));

    // Step 5: Test Sales Order Shipment
    println!("\n=== Testing Sales Order Shipment ===");
    let shipment_results = service
        .adjust_for_sales_order(sales_order.header_id, SalesOrderAdjustmentType::Ship)
        .await
        .expect("Failed to ship inventory");

    println!("Shipment Results:");
    for result in &shipment_results {
        println!(
            "  Item {}: Shipped {} units",
            result.item_id, result.quantity_adjusted
        );
        println!("    New On-Hand: {}", result.new_on_hand);
    }

    // Verify shipment
    let inv1_after_ship =
        get_inventory_balance(&txn, item1.inventory_item_id, location1.location_id).await;
    assert_eq!(inv1_after_ship.quantity_on_hand, dec(98));
    assert_eq!(inv1_after_ship.quantity_allocated, dec(0));

    // Step 6: Test Purchase Order Receipt
    println!("\n=== Testing Purchase Order Receipt ===");
    let po = create_test_purchase_order(&txn).await;
    let po_line1 =
        create_purchase_order_line(&txn, po.po_header_id, item1.inventory_item_id, 50).await;
    let po_line2 =
        create_purchase_order_line(&txn, po.po_header_id, item3.inventory_item_id, 100).await;

    let receipt_lines = vec![
        PurchaseOrderReceiptLine {
            po_line_id: po_line1.po_line_id,
            quantity_received: dec(50),
            location_id: location1.location_id,
        },
        PurchaseOrderReceiptLine {
            po_line_id: po_line2.po_line_id,
            quantity_received: dec(100),
            location_id: location1.location_id,
        },
    ];

    let receipt_results = service
        .adjust_for_purchase_order_receipt(po.po_header_id, receipt_lines)
        .await
        .expect("Failed to receive inventory");

    println!("Receipt Results:");
    for result in &receipt_results {
        println!(
            "  Item {}: Received {} units",
            result.item_id, result.quantity_adjusted
        );
        println!("    New On-Hand: {}", result.new_on_hand);
    }

    // Verify receipt
    let inv1_after_receipt =
        get_inventory_balance(&txn, item1.inventory_item_id, location1.location_id).await;
    assert_eq!(inv1_after_receipt.quantity_on_hand, dec(148)); // 98 + 50
    assert_eq!(inv1_after_receipt.quantity_available, dec(148));

    // Step 7: Test Order Cancellation (Deallocation)
    println!("\n=== Testing Order Cancellation ===");
    let cancel_order = create_test_sales_order(&txn).await;
    let cancel_line = create_sales_order_line(
        &txn,
        cancel_order.header_id,
        item2.inventory_item_id,
        location1.location_id,
        5,
    )
    .await;

    // First allocate
    service
        .adjust_for_sales_order(cancel_order.header_id, SalesOrderAdjustmentType::Allocate)
        .await
        .expect("Failed to allocate for cancellation test");

    // Then cancel
    let cancel_results = service
        .adjust_for_sales_order(cancel_order.header_id, SalesOrderAdjustmentType::Cancel)
        .await
        .expect("Failed to cancel order");

    println!("Cancellation Results:");
    for result in &cancel_results {
        println!(
            "  Item {}: Deallocated {} units",
            result.item_id, result.quantity_adjusted
        );
        println!("    New Available: {}", result.new_available);
    }

    // Step 8: Test Return Processing
    println!("\n=== Testing Return Processing ===");
    let return_results = service
        .adjust_for_sales_order(sales_order.header_id, SalesOrderAdjustmentType::Return)
        .await
        .expect("Failed to process return");

    println!("Return Results:");
    for result in &return_results {
        println!(
            "  Item {}: Returned {} units",
            result.item_id, result.quantity_adjusted
        );
        println!("    New On-Hand: {}", result.new_on_hand);
    }

    // Verify return
    let inv1_after_return =
        get_inventory_balance(&txn, item1.inventory_item_id, location1.location_id).await;
    assert_eq!(inv1_after_return.quantity_on_hand, dec(150)); // 148 + 2 returned

    // Step 9: Query and display transaction history
    println!("\n=== Transaction History ===");
    let transactions = InventoryTransaction::find()
        .order_by_desc(inventory_transaction::Column::CreatedAt)
        .limit(10)
        .all(&txn)
        .await
        .expect("Failed to query transactions");

    for trans in transactions {
        println!(
            "Transaction: {} - Type: {}, Qty: {}, Reference: {:?}",
            trans.id, trans.r#type, trans.quantity, trans.reference_type
        );
    }

    // Step 10: Generate inventory summary report
    println!("\n=== Inventory Summary Report ===");
    println!("Item Master | Location | On-Hand | Available | Allocated");
    println!("---------------------------------------------------------");

    for item in [&item1, &item2, &item3] {
        let balance =
            get_inventory_balance(&txn, item.inventory_item_id, location1.location_id).await;
        println!(
            "{} | {} | {} | {} | {}",
            &item.item_number,
            &location1.location_name,
            balance.quantity_on_hand,
            balance.quantity_available,
            balance.quantity_allocated
        );
    }

    // Rollback transaction (test cleanup)
    txn.rollback()
        .await
        .expect("Failed to rollback transaction");

    env::remove_var("APP__DATABASE_URL");

    println!("\n✅ All inventory adjustment tests passed successfully!");
}

// Helper functions

async fn create_test_item(
    db: &DatabaseTransaction,
    item_number: &str,
    description: &str,
) -> item_master::Model {
    let item = item_master::ActiveModel {
        item_number: Set(item_number.to_string()),
        description: Set(Some(description.to_string())),
        organization_id: Set(1),
        primary_uom_code: Set(Some("EA".to_string())),
        item_type: Set(Some("FINISHED_GOOD".to_string())),
        status_code: Set(Some("ACTIVE".to_string())),
        lead_time_weeks: Set(Some(2)),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
        ..Default::default()
    };

    item.insert(db).await.expect("Failed to create item master")
}

async fn create_test_location(
    db: &DatabaseTransaction,
    location_code: &str,
    description: &str,
) -> inventory_location::Model {
    let location = inventory_location::ActiveModel {
        location_name: Set(format!("{} - {}", location_code, description)),
        ..Default::default()
    };

    location
        .insert(db)
        .await
        .expect("Failed to create location")
}

async fn create_initial_inventory(
    db: &DatabaseTransaction,
    item_id: i64,
    location_id: i32,
    quantity: i32,
) {
    let now = Utc::now().to_rfc3339();
    let inventory = inventory_balance::ActiveModel {
        inventory_item_id: Set(item_id),
        location_id: Set(location_id),
        quantity_on_hand: Set(dec(quantity)),
        quantity_allocated: Set(dec_zero()),
        quantity_available: Set(dec(quantity)),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
        ..Default::default()
    };

    inventory_balance::Entity::insert(inventory)
        .exec(db)
        .await
        .expect("Failed to create initial inventory");
}

async fn create_test_sales_order(db: &DatabaseTransaction) -> sales_order_header::Model {
    let order = sales_order_header::ActiveModel {
        order_number: Set(format!("SO-{}", Uuid::new_v4())),
        ordered_date: Set(Some(Utc::now().date_naive())),
        status_code: Set(Some("OPEN".to_string())),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
        ..Default::default()
    };

    order
        .insert(db)
        .await
        .expect("Failed to create sales order")
}

async fn create_sales_order_line(
    db: &DatabaseTransaction,
    header_id: i64,
    item_id: i64,
    location_id: i32,
    quantity: i32,
) -> sales_order_line::Model {
    let line = sales_order_line::ActiveModel {
        header_id: Set(Some(header_id)),
        line_number: Set(Some(1)),
        inventory_item_id: Set(Some(item_id)),
        ordered_quantity: Set(Some(dec(quantity))),
        unit_selling_price: Set(Some(dec(100))),
        line_status: Set(Some("OPEN".to_string())),
        location_id: Set(Some(location_id)),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
        ..Default::default()
    };

    line.insert(db)
        .await
        .expect("Failed to create sales order line")
}

async fn create_test_purchase_order(db: &DatabaseTransaction) -> purchase_order_headers::Model {
    let po = purchase_order_headers::ActiveModel {
        po_number: Set(format!("PO-{}", Uuid::new_v4())),
        vendor_id: Set(Some(1)),
        approved_flag: Set(Some(true)),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
        ..Default::default()
    };

    po.insert(db)
        .await
        .expect("Failed to create purchase order")
}

async fn create_purchase_order_line(
    db: &DatabaseTransaction,
    header_id: i64,
    item_id: i64,
    quantity: i32,
) -> purchase_order_lines::Model {
    let line = purchase_order_lines::ActiveModel {
        po_header_id: Set(Some(header_id)),
        line_num: Set(Some(1)),
        item_id: Set(Some(item_id)),
        quantity: Set(Some(dec(quantity))),
        unit_price: Set(Some(dec(50))),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
        ..Default::default()
    };

    line.insert(db)
        .await
        .expect("Failed to create purchase order line")
}
async fn get_inventory_balance(
    db: &DatabaseTransaction,
    item_id: i64,
    location_id: i32,
) -> inventory_balance::Model {
    InventoryBalance::find()
        .filter(inventory_balance::Column::InventoryItemId.eq(item_id))
        .filter(inventory_balance::Column::LocationId.eq(location_id))
        .one(db)
        .await
        .expect("Failed to query inventory balance")
        .expect("Inventory balance not found")
}
