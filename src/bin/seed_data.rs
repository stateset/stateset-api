//! Seed data script - populates the database with realistic demo data
//!
//! Run with: cargo run --bin seed-data
//!
//! This creates:
//! - 10 products (electronics, apparel, accessories)
//! - 5 customers with addresses
//! - 15 orders in various states
//! - Inventory for all products

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use sea_orm::{ActiveModelTrait, ConnectOptions, Database, Set};
use std::time::Duration as StdDuration;
use tracing::info;
use uuid::Uuid;

use stateset_api::entities::{
    commerce::customer::{self, CustomerStatus},
    inventory_items, order, order_item, product,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("=== StateSet API Seed Data ===");
    info!("Creating realistic demo data for exploration...\n");

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/stateset_db".to_string());

    let mut options = ConnectOptions::new(database_url.clone());
    options
        .max_connections(5)
        .min_connections(1)
        .connect_timeout(StdDuration::from_secs(10))
        .acquire_timeout(StdDuration::from_secs(10));

    info!("Connecting to database: {}", database_url);
    let db = Database::connect(options).await?;
    info!("Connected!\n");

    // Create products
    info!("Creating products...");
    let products = create_products(&db).await?;
    info!("  Created {} products", products.len());

    // Create customers
    info!("Creating customers...");
    let customers = create_customers(&db).await?;
    info!("  Created {} customers", customers.len());

    // Create inventory
    info!("Creating inventory...");
    let inventory_count = create_inventory(&db, &products).await?;
    info!("  Created {} inventory items", inventory_count);

    // Create orders
    info!("Creating orders...");
    let order_count = create_orders(&db, &products, &customers).await?;
    info!("  Created {} orders with items", order_count);

    info!("\n=== Seed Data Complete ===");
    info!("Your StateSet API is now populated with demo data!");
    info!("");
    info!("Try these API calls:");
    info!("  curl http://localhost:8080/api/v1/products");
    info!("  curl http://localhost:8080/api/v1/orders");
    info!("  curl http://localhost:8080/api/v1/inventory");
    info!("  curl http://localhost:8080/api/v1/customers");
    info!("");
    info!("Or explore interactively at: http://localhost:8080/swagger-ui");

    Ok(())
}

async fn create_products(
    db: &sea_orm::DatabaseConnection,
) -> anyhow::Result<Vec<product::Model>> {
    let products_data = vec![
        // Electronics
        ("Wireless Bluetooth Headphones", "WBH-001", dec!(79.99), "High-quality over-ear headphones with 30-hour battery life and active noise cancellation.", "Electronics,Audio"),
        ("USB-C Fast Charger 65W", "CHG-065", dec!(34.99), "GaN technology charger compatible with laptops, phones, and tablets.", "Electronics,Accessories"),
        ("Mechanical Keyboard RGB", "KBD-RGB", dec!(129.99), "Hot-swappable mechanical keyboard with per-key RGB lighting.", "Electronics,Peripherals"),
        ("4K Webcam Pro", "WEB-4K1", dec!(149.99), "4K resolution webcam with auto-focus and built-in microphone.", "Electronics,Video"),

        // Apparel
        ("Classic Cotton T-Shirt", "TSH-BLK-M", dec!(24.99), "Premium 100% organic cotton t-shirt. Comfortable fit.", "Apparel,Basics"),
        ("Slim Fit Denim Jeans", "JNS-SLM-32", dec!(89.99), "Stretch denim jeans with modern slim fit.", "Apparel,Bottoms"),
        ("Merino Wool Sweater", "SWT-MRN-L", dec!(119.99), "Temperature-regulating merino wool sweater.", "Apparel,Tops"),

        // Accessories
        ("Leather Bifold Wallet", "WLT-LTH", dec!(49.99), "Genuine leather wallet with RFID blocking.", "Accessories,Wallets"),
        ("Canvas Backpack 25L", "BAG-CNV-25", dec!(79.99), "Water-resistant canvas backpack with laptop compartment.", "Accessories,Bags"),
        ("Stainless Steel Water Bottle", "BTL-SS-32", dec!(29.99), "32oz double-wall insulated bottle. Keeps drinks cold 24hrs.", "Accessories,Outdoor"),
    ];

    let mut created = Vec::new();
    let now = Utc::now();

    for (name, sku, price, description, tags) in products_data {
        let product = product::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(name.to_string()),
            sku: Set(sku.to_string()),
            price: Set(price),
            currency: Set("USD".to_string()),
            description: Set(Some(description.to_string())),
            tags: Set(Some(tags.to_string())),
            is_active: Set(true),
            is_digital: Set(false),
            brand: Set(Some("StateSet Demo".to_string())),
            cost_price: Set(Some(price * dec!(0.6))),
            reorder_point: Set(Some(10)),
            tax_rate: Set(Some(dec!(0.0875))),
            weight_kg: Set(Some(dec!(0.5))),
            dimensions_cm: Set(None),
            barcode: Set(None),
            manufacturer: Set(None),
            image_url: Set(None),
            category_id: Set(None),
            msrp: Set(None),
            meta_title: Set(None),
            meta_description: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };

        let model = product.insert(db).await?;
        created.push(model);
    }

    Ok(created)
}

async fn create_customers(
    db: &sea_orm::DatabaseConnection,
) -> anyhow::Result<Vec<customer::Model>> {
    let customers_data = vec![
        ("alice@example.com", "Alice", "Johnson", Some("+1-555-0101")),
        ("bob@example.com", "Bob", "Smith", Some("+1-555-0102")),
        ("carol@example.com", "Carol", "Williams", Some("+1-555-0103")),
        ("david@example.com", "David", "Brown", None),
        ("eva@example.com", "Eva", "Martinez", Some("+1-555-0105")),
    ];

    let mut created = Vec::new();
    let now = Utc::now();

    for (email, first_name, last_name, phone) in customers_data {
        let customer = customer::ActiveModel {
            id: Set(Uuid::new_v4()),
            email: Set(email.to_string()),
            first_name: Set(first_name.to_string()),
            last_name: Set(last_name.to_string()),
            phone: Set(phone.map(|p| p.to_string())),
            accepts_marketing: Set(true),
            customer_group_id: Set(None),
            default_shipping_address_id: Set(None),
            default_billing_address_id: Set(None),
            tags: Set(serde_json::json!(["demo", "seed-data"])),
            metadata: Set(None),
            email_verified: Set(true),
            email_verified_at: Set(Some(now)),
            status: Set(CustomerStatus::Active),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let model = customer.insert(db).await?;
        created.push(model);
    }

    Ok(created)
}

async fn create_inventory(
    db: &sea_orm::DatabaseConnection,
    products: &[product::Model],
) -> anyhow::Result<usize> {
    let warehouses = vec!["MAIN", "WEST", "EAST"];
    let now = Utc::now();
    let mut count = 0;

    for product in products {
        for warehouse in &warehouses {
            // Vary quantities by warehouse
            let base_qty = match *warehouse {
                "MAIN" => 100,
                "WEST" => 50,
                "EAST" => 25,
                _ => 10,
            };

            let inventory = inventory_items::ActiveModel {
                id: Set(format!("{}-{}", product.sku, warehouse)),
                sku: Set(product.sku.clone()),
                warehouse: Set(warehouse.to_string()),
                available: Set(base_qty),
                allocated_quantity: Set(Some(0)),
                reserved_quantity: Set(Some(0)),
                unit_cost: Set(product.cost_price),
                last_movement_date: Set(Some(now.naive_utc())),
                arrival_date: Set(now.date_naive()),
                created_at: Set(now.naive_utc()),
                updated_at: Set(now.naive_utc()),
            };

            inventory.insert(db).await?;
            count += 1;
        }
    }

    Ok(count)
}

async fn create_orders(
    db: &sea_orm::DatabaseConnection,
    products: &[product::Model],
    customers: &[customer::Model],
) -> anyhow::Result<usize> {
    let order_scenarios = vec![
        // (status, payment_status, fulfillment_status, days_ago)
        ("pending", "pending", "unfulfilled", 0),
        ("pending", "paid", "unfulfilled", 1),
        ("processing", "paid", "processing", 2),
        ("processing", "paid", "partially_fulfilled", 3),
        ("shipped", "paid", "fulfilled", 5),
        ("shipped", "paid", "fulfilled", 7),
        ("delivered", "paid", "delivered", 10),
        ("delivered", "paid", "delivered", 14),
        ("delivered", "paid", "delivered", 21),
        ("cancelled", "refunded", "cancelled", 4),
        ("pending", "pending", "unfulfilled", 0),
        ("processing", "paid", "processing", 1),
        ("shipped", "paid", "in_transit", 3),
        ("delivered", "paid", "delivered", 30),
        ("pending", "failed", "unfulfilled", 0),
    ];

    let now = Utc::now();
    let mut count = 0;

    for (i, (status, payment_status, fulfillment_status, days_ago)) in
        order_scenarios.iter().enumerate()
    {
        let customer = &customers[i % customers.len()];
        let order_date = now - Duration::days(*days_ago as i64);
        let order_id = Uuid::new_v4();

        // Select 1-3 random products for this order
        let num_items = (i % 3) + 1;
        let order_products: Vec<&product::Model> = products
            .iter()
            .skip(i % products.len())
            .take(num_items)
            .collect();

        // Calculate totals
        let mut subtotal = Decimal::ZERO;
        let mut items_data = Vec::new();

        for (j, prod) in order_products.iter().enumerate() {
            let qty = ((j % 3) + 1) as i32;
            let line_total = prod.price * Decimal::from(qty);
            let tax = line_total * dec!(0.0875);

            items_data.push((prod, qty, line_total, tax));
            subtotal += line_total + tax;
        }

        // Create order
        let shipping_address = serde_json::json!({
            "street": format!("{} Main Street", 100 + i),
            "city": "San Francisco",
            "state": "CA",
            "postal_code": "94105",
            "country": "US"
        });

        let order = order::ActiveModel {
            id: Set(order_id),
            order_number: Set(format!("ORD-{:05}", 10000 + i)),
            customer_id: Set(customer.id),
            status: Set(status.to_string()),
            order_date: Set(order_date),
            total_amount: Set(subtotal),
            currency: Set("USD".to_string()),
            payment_status: Set(payment_status.to_string()),
            fulfillment_status: Set(fulfillment_status.to_string()),
            payment_method: Set(Some("credit_card".to_string())),
            shipping_method: Set(Some("standard".to_string())),
            tracking_number: Set(if *status == "shipped" || *status == "delivered" {
                Some(format!("1Z999AA{:08}", 10000000 + i))
            } else {
                None
            }),
            notes: Set(None),
            shipping_address: Set(Some(shipping_address.to_string())),
            billing_address: Set(None),
            is_archived: Set(false),
            created_at: Set(order_date),
            updated_at: Set(Some(now)),
            version: Set(1),
        };

        order.insert(db).await?;

        // Create order items
        for (prod, qty, line_total, tax) in items_data {
            let item = order_item::ActiveModel {
                id: Set(Uuid::new_v4()),
                order_id: Set(order_id),
                product_id: Set(prod.id),
                sku: Set(prod.sku.clone()),
                name: Set(prod.name.clone()),
                quantity: Set(qty),
                unit_price: Set(prod.price),
                total_price: Set(line_total),
                discount: Set(Decimal::ZERO),
                tax_rate: Set(dec!(0.0875)),
                tax_amount: Set(tax),
                status: Set("active".to_string()),
                notes: Set(None),
                created_at: Set(order_date),
                updated_at: Set(Some(now)),
            };

            item.insert(db).await?;
        }

        count += 1;
    }

    Ok(count)
}
