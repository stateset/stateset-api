use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::prelude::*;
use std::time::Duration;
use tracing::{error, info};

mod migrations {
    
    use sea_orm_migration::prelude::*;
    

    /// Create orders table
    #[derive(DeriveMigrationName)]
    pub struct Migration20230101000001CreateOrdersTable;

    #[async_trait::async_trait]
    impl MigrationTrait for Migration20230101000001CreateOrdersTable {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            manager
                .create_table(
                    Table::create()
                        .table(Orders::Table)
                        .if_not_exists()
                        .col(ColumnDef::new(Orders::Id).uuid().primary_key().not_null())
                        .col(ColumnDef::new(Orders::CustomerId).uuid().not_null())
                        .col(ColumnDef::new(Orders::Status).string().not_null())
                        .col(ColumnDef::new(Orders::TotalAmount).double().not_null())
                        .col(ColumnDef::new(Orders::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(Orders::UpdatedAt).timestamp().not_null())
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            manager
                .drop_table(Table::drop().table(Orders::Table).to_owned())
                .await
        }
    }

    /// Order schema identifiers
    #[derive(Iden)]
    enum Orders {
        Table,
        Id,
        CustomerId,
        Status,
        TotalAmount,
        CreatedAt,
        UpdatedAt,
    }

    /// Create order items table
    #[derive(DeriveMigrationName)]
    pub struct Migration20230101000002CreateOrderItemsTable;

    #[async_trait::async_trait]
    impl MigrationTrait for Migration20230101000002CreateOrderItemsTable {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            manager
                .create_table(
                    Table::create()
                        .table(OrderItems::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(OrderItems::Id)
                                .uuid()
                                .primary_key()
                                .not_null(),
                        )
                        .col(ColumnDef::new(OrderItems::OrderId).uuid().not_null())
                        .col(ColumnDef::new(OrderItems::ProductId).uuid().not_null())
                        .col(ColumnDef::new(OrderItems::Quantity).integer().not_null())
                        .col(ColumnDef::new(OrderItems::Price).double().not_null())
                        .col(ColumnDef::new(OrderItems::CreatedAt).timestamp().not_null())
                        .foreign_key(
                            ForeignKey::create()
                                .name("fk_order_items_order_id")
                                .from(OrderItems::Table, OrderItems::OrderId)
                                .to(Orders::Table, Orders::Id)
                                .on_delete(ForeignKeyAction::Cascade),
                        )
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            manager
                .drop_table(Table::drop().table(OrderItems::Table).to_owned())
                .await
        }
    }

    /// Order items schema identifiers
    #[derive(Iden)]
    enum OrderItems {
        Table,
        Id,
        OrderId,
        ProductId,
        Quantity,
        Price,
        CreatedAt,
    }

    /// Create customers table
    #[derive(DeriveMigrationName)]
    pub struct Migration20230101000003CreateCustomersTable;

    #[async_trait::async_trait]
    impl MigrationTrait for Migration20230101000003CreateCustomersTable {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            manager
                .create_table(
                    Table::create()
                        .table(Customers::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(Customers::Id)
                                .uuid()
                                .primary_key()
                                .not_null(),
                        )
                        .col(ColumnDef::new(Customers::Name).string().not_null())
                        .col(ColumnDef::new(Customers::Email).string().not_null())
                        .col(ColumnDef::new(Customers::Phone).string())
                        .col(ColumnDef::new(Customers::Address).string())
                        .col(ColumnDef::new(Customers::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(Customers::UpdatedAt).timestamp().not_null())
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            manager
                .drop_table(Table::drop().table(Customers::Table).to_owned())
                .await
        }
    }

    /// Customers schema identifiers
    #[derive(Iden)]
    enum Customers {
        Table,
        Id,
        Name,
        Email,
        Phone,
        Address,
        CreatedAt,
        UpdatedAt,
    }

    /// Create products table
    #[derive(DeriveMigrationName)]
    pub struct Migration20230101000004CreateProductsTable;

    #[async_trait::async_trait]
    impl MigrationTrait for Migration20230101000004CreateProductsTable {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            manager
                .create_table(
                    Table::create()
                        .table(Products::Table)
                        .if_not_exists()
                        .col(ColumnDef::new(Products::Id).uuid().primary_key().not_null())
                        .col(ColumnDef::new(Products::Name).string().not_null())
                        .col(ColumnDef::new(Products::Description).string())
                        .col(ColumnDef::new(Products::Sku).string().not_null())
                        .col(ColumnDef::new(Products::Price).double().not_null())
                        .col(ColumnDef::new(Products::Status).string().not_null())
                        .col(ColumnDef::new(Products::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(Products::UpdatedAt).timestamp().not_null())
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            manager
                .drop_table(Table::drop().table(Products::Table).to_owned())
                .await
        }
    }

    /// Products schema identifiers
    #[derive(Iden)]
    enum Products {
        Table,
        Id,
        Name,
        Description,
        Sku,
        Price,
        Status,
        CreatedAt,
        UpdatedAt,
    }
}

#[async_trait::async_trait]
pub trait MigratorTrait {
    fn migrations() -> Vec<Box<dyn MigrationTrait>>;

    async fn up(
        db: &DatabaseConnection,
        schema_manager: Option<&SchemaManager>,
    ) -> Result<(), DbErr> {
        let schema_manager = match schema_manager {
            Some(schema_manager) => schema_manager,
            None => &SchemaManager::new(db),
        };

        for migration in Self::migrations() {
            let migration_name = migration.name();
            info!("Running migration: {}", migration_name);

            let result = migration.up(schema_manager).await;
            match result {
                Ok(_) => info!("Migration {} completed successfully", migration_name),
                Err(e) => {
                    error!("Migration {} failed: {}", migration_name, e);
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    async fn down(
        db: &DatabaseConnection,
        schema_manager: Option<&SchemaManager>,
    ) -> Result<(), DbErr> {
        let schema_manager = match schema_manager {
            Some(schema_manager) => schema_manager,
            None => &SchemaManager::new(db),
        };

        for migration in Self::migrations().into_iter().rev() {
            let migration_name = migration.name();
            info!("Rolling back migration: {}", migration_name);

            let result = migration.down(schema_manager).await;
            match result {
                Ok(_) => info!("Rollback of {} completed successfully", migration_name),
                Err(e) => {
                    error!("Rollback of {} failed: {}", migration_name, e);
                    return Err(e);
                }
            }
        }

        Ok(())
    }
}

pub struct Migrator;

impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(migrations::Migration20230101000001CreateOrdersTable),
            Box::new(migrations::Migration20230101000002CreateOrderItemsTable),
            Box::new(migrations::Migration20230101000003CreateCustomersTable),
            Box::new(migrations::Migration20230101000004CreateProductsTable),
        ]
    }
}

#[tokio::main]
async fn main() -> Result<(), DbErr> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting database migration");

    // Configure database connection
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());

    info!("Connecting to database: {}", database_url);

    // Setup connection options
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(5)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(10))
        .acquire_timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(300))
        .sqlx_logging(true);

    // Create database connection
    let db = Database::connect(options).await?;

    // Run migrations
    Migrator::up(&db, None).await?;

    info!("Migration completed successfully");

    Ok(())
}
