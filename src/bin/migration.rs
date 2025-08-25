use sea_orm::{ConnectOptions, Database};
use sea_orm_migration::MigratorTrait;
use std::time::Duration;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting database migration binary");

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/stateset_db".to_string());
    info!("Connecting to database: {}", database_url);

    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(5)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(10))
        .acquire_timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(300))
        .sqlx_logging(true);

    let db = Database::connect(options).await?;

    // Use the shared migrator from the library
    stateset_api::migrator::Migrator::up(&db, None).await?;

    info!("Migration completed successfully");
    Ok(())
}
