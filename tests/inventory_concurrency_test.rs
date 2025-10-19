use stateset_api::events::{process_events, EventSender};
use stateset_api::services::inventory::AdjustInventoryCommand;
use stateset_api::{db, services::inventory::InventoryService};
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

// This test is ignored by default because it requires a real SQLite/Postgres DB and migrations.
// Run with: cargo test -- --ignored inventory_concurrency
#[tokio::test]
#[ignore]
async fn inventory_concurrency() {
    // Set up DB (use SQLite memory or configured DB)
    let cfg = stateset_api::config::AppConfig::new(
        // Use SQLite memory URL if supported by your build; adjust as needed
        "sqlite::memory:".to_string(),
        "redis://127.0.0.1:6379".to_string(),
        "test_secret_key_for_testing_purposes_only_32chars".to_string(),
        3600,
        86400,
        "127.0.0.1".to_string(),
        18080,
        "test".to_string(),
    );
    let pool = db::establish_connection_from_app_config(&cfg)
        .await
        .expect("db connect");
    let _ = db::run_migrations(&pool).await; // best-effort

    let db_arc = Arc::new(pool);
    let (tx, rx) = mpsc::channel(100);
    let sender = EventSender::new(tx);
    tokio::spawn(process_events(rx));

    let svc = InventoryService::new(db_arc.clone(), sender.clone());

    // Seed one inventory row (sku=product, warehouse=location)
    let product = Uuid::new_v4();
    let location = Uuid::new_v4();
    svc.adjust_inventory(AdjustInventoryCommand {
        product_id: Some(product),
        location_id: Some(location),
        adjustment_quantity: Some(10),
        reason: Some("seed".into()),
    })
    .await
    .expect("seed adjust");

    // Try 20 concurrent reservations of 1 unit each, expect only 10 successes
    let mut tasks = vec![];
    for _ in 0..20 {
        let svc = svc.clone();
        let p = product;
        let l = location;
        tasks.push(tokio::spawn(async move {
            svc.reserve_inventory_simple(&p, &l, 1)
                .await
                .map(|_| ())
                .is_ok()
        }));
    }
    let mut success = 0;
    for t in tasks {
        if t.await.unwrap_or(false) {
            success += 1;
        }
    }
    assert_eq!(
        success, 10,
        "exactly 10 reservations should succeed; got {}",
        success
    );
}
