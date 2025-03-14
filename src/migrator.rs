use sea_orm_migration::prelude::*;
use std::time::Duration;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use anyhow::Result;
use tracing::{info, error};

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20230101_000001_create_orders_table::Migration),
            Box::new(m20230101_000002_create_order_items_table::Migration),
            Box::new(m20230101_000003_create_customers_table::Migration),
            Box::new(m20230101_000004_create_products_table::Migration),
            Box::new(m20230101_000005_create_inventory_table::Migration),
            Box::new(m20230101_000006_create_returns_table::Migration),
            Box::new(m20230101_000007_create_shipments_table::Migration),
            Box::new(m20230101_000008_create_bill_of_materials_table::Migration),
            Box::new(m20230101_000009_create_work_orders_table::Migration),
            Box::new(m20230101_000010_create_users_table::Migration),
        ]
    }
}

// Migration implementations

mod m20230101_000001_create_orders_table {
    use sea_orm_migration::prelude::*;
    use sea_orm::DbBackend;

    #[derive(DeriveMigrationName)]
    pub struct Migration;

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create orders table
            manager
                .create_table(
                    Table::create()
                        .table(Orders::Table)
                        .if_not_exists()
                        .col(ColumnDef::new(Orders::Id).uuid().primary_key().not_null())
                        .col(ColumnDef::new(Orders::CustomerId).uuid().not_null())
                        .col(ColumnDef::new(Orders::Status).string().not_null())
                        .col(ColumnDef::new(Orders::TotalAmount).decimal().default(0.0))
                        .col(ColumnDef::new(Orders::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(Orders::UpdatedAt).timestamp().null())
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Drop orders table
            manager
                .drop_table(Table::drop().table(Orders::Table).to_owned())
                .await
        }
    }

    #[derive(DeriveIden)]
    enum Orders {
        Table,
        Id,
        CustomerId,
        Status,
        TotalAmount,
        CreatedAt,
        UpdatedAt,
    }
}

mod m20230101_000002_create_order_items_table {
    use sea_orm_migration::prelude::*;
    use sea_orm::DbBackend;

    #[derive(DeriveMigrationName)]
    pub struct Migration;

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create order_items table
            manager
                .create_table(
                    Table::create()
                        .table(OrderItems::Table)
                        .if_not_exists()
                        .col(ColumnDef::new(OrderItems::Id).uuid().primary_key().not_null())
                        .col(ColumnDef::new(OrderItems::OrderId).uuid().not_null())
                        .col(ColumnDef::new(OrderItems::ProductId).uuid().not_null())
                        .col(ColumnDef::new(OrderItems::Quantity).integer().not_null())
                        .col(ColumnDef::new(OrderItems::UnitPrice).decimal().not_null())
                        .col(ColumnDef::new(OrderItems::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(OrderItems::UpdatedAt).timestamp().null())
                        .foreign_key(
                            ForeignKey::create()
                                .name("fk_order_items_order_id")
                                .from(OrderItems::Table, OrderItems::OrderId)
                                .to("orders", "id")
                                .on_delete(ForeignKeyAction::Cascade)
                                .on_update(ForeignKeyAction::Cascade),
                        )
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Drop order_items table
            manager
                .drop_table(Table::drop().table(OrderItems::Table).to_owned())
                .await
        }
    }

    #[derive(DeriveIden)]
    enum OrderItems {
        Table,
        Id,
        OrderId,
        ProductId,
        Quantity,
        UnitPrice,
        CreatedAt,
        UpdatedAt,
    }
}

mod m20230101_000003_create_customers_table {
    use sea_orm_migration::prelude::*;

    #[derive(DeriveMigrationName)]
    pub struct Migration;

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create customers table
            manager
                .create_table(
                    Table::create()
                        .table(Customers::Table)
                        .if_not_exists()
                        .col(ColumnDef::new(Customers::Id).uuid().primary_key().not_null())
                        .col(ColumnDef::new(Customers::Name).string().not_null())
                        .col(ColumnDef::new(Customers::Email).string().not_null())
                        .col(ColumnDef::new(Customers::Phone).string().null())
                        .col(ColumnDef::new(Customers::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(Customers::UpdatedAt).timestamp().null())
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Drop customers table
            manager
                .drop_table(Table::drop().table(Customers::Table).to_owned())
                .await
        }
    }

    #[derive(DeriveIden)]
    enum Customers {
        Table,
        Id,
        Name,
        Email,
        Phone,
        CreatedAt,
        UpdatedAt,
    }
}

mod m20230101_000004_create_products_table {
    use sea_orm_migration::prelude::*;

    #[derive(DeriveMigrationName)]
    pub struct Migration;

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create products table
            manager
                .create_table(
                    Table::create()
                        .table(Products::Table)
                        .if_not_exists()
                        .col(ColumnDef::new(Products::Id).uuid().primary_key().not_null())
                        .col(ColumnDef::new(Products::Name).string().not_null())
                        .col(ColumnDef::new(Products::Description).string().null())
                        .col(ColumnDef::new(Products::Sku).string().not_null())
                        .col(ColumnDef::new(Products::Price).decimal().not_null())
                        .col(ColumnDef::new(Products::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(Products::UpdatedAt).timestamp().null())
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Drop products table
            manager
                .drop_table(Table::drop().table(Products::Table).to_owned())
                .await
        }
    }

    #[derive(DeriveIden)]
    enum Products {
        Table,
        Id,
        Name,
        Description,
        Sku,
        Price,
        CreatedAt,
        UpdatedAt,
    }
}

mod m20230101_000005_create_inventory_table {
    use sea_orm_migration::prelude::*;

    #[derive(DeriveMigrationName)]
    pub struct Migration;

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create inventory table
            manager
                .create_table(
                    Table::create()
                        .table(Inventory::Table)
                        .if_not_exists()
                        .col(ColumnDef::new(Inventory::Id).uuid().primary_key().not_null())
                        .col(ColumnDef::new(Inventory::ProductId).uuid().not_null())
                        .col(ColumnDef::new(Inventory::WarehouseId).uuid().not_null())
                        .col(ColumnDef::new(Inventory::Quantity).integer().not_null())
                        .col(ColumnDef::new(Inventory::Reserved).integer().not_null().default(0))
                        .col(ColumnDef::new(Inventory::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(Inventory::UpdatedAt).timestamp().null())
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Drop inventory table
            manager
                .drop_table(Table::drop().table(Inventory::Table).to_owned())
                .await
        }
    }

    #[derive(DeriveIden)]
    enum Inventory {
        Table,
        Id,
        ProductId,
        WarehouseId,
        Quantity,
        Reserved,
        CreatedAt,
        UpdatedAt,
    }
}

mod m20230101_000006_create_returns_table {
    use sea_orm_migration::prelude::*;

    #[derive(DeriveMigrationName)]
    pub struct Migration;

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create returns table
            manager
                .create_table(
                    Table::create()
                        .table(Returns::Table)
                        .if_not_exists()
                        .col(ColumnDef::new(Returns::Id).uuid().primary_key().not_null())
                        .col(ColumnDef::new(Returns::OrderId).uuid().not_null())
                        .col(ColumnDef::new(Returns::Status).string().not_null())
                        .col(ColumnDef::new(Returns::Reason).string().null())
                        .col(ColumnDef::new(Returns::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(Returns::UpdatedAt).timestamp().null())
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Drop returns table
            manager
                .drop_table(Table::drop().table(Returns::Table).to_owned())
                .await
        }
    }

    #[derive(DeriveIden)]
    enum Returns {
        Table,
        Id,
        OrderId,
        Status,
        Reason,
        CreatedAt,
        UpdatedAt,
    }
}

mod m20230101_000007_create_shipments_table {
    use sea_orm_migration::prelude::*;

    #[derive(DeriveMigrationName)]
    pub struct Migration;

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create shipments table
            manager
                .create_table(
                    Table::create()
                        .table(Shipments::Table)
                        .if_not_exists()
                        .col(ColumnDef::new(Shipments::Id).uuid().primary_key().not_null())
                        .col(ColumnDef::new(Shipments::OrderId).uuid().not_null())
                        .col(ColumnDef::new(Shipments::Status).string().not_null())
                        .col(ColumnDef::new(Shipments::TrackingNumber).string().null())
                        .col(ColumnDef::new(Shipments::Carrier).string().null())
                        .col(ColumnDef::new(Shipments::ShippedAt).timestamp().null())
                        .col(ColumnDef::new(Shipments::DeliveredAt).timestamp().null())
                        .col(ColumnDef::new(Shipments::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(Shipments::UpdatedAt).timestamp().null())
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Drop shipments table
            manager
                .drop_table(Table::drop().table(Shipments::Table).to_owned())
                .await
        }
    }

    #[derive(DeriveIden)]
    enum Shipments {
        Table,
        Id,
        OrderId,
        Status,
        TrackingNumber,
        Carrier,
        ShippedAt,
        DeliveredAt,
        CreatedAt,
        UpdatedAt,
    }
}

mod m20230101_000008_create_bill_of_materials_table {
    use sea_orm_migration::prelude::*;

    #[derive(DeriveMigrationName)]
    pub struct Migration;

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create bill_of_materials table
            manager
                .create_table(
                    Table::create()
                        .table(BillOfMaterials::Table)
                        .if_not_exists()
                        .col(ColumnDef::new(BillOfMaterials::Id).uuid().primary_key().not_null())
                        .col(ColumnDef::new(BillOfMaterials::ProductId).uuid().not_null())
                        .col(ColumnDef::new(BillOfMaterials::Name).string().not_null())
                        .col(ColumnDef::new(BillOfMaterials::Description).string().null())
                        .col(ColumnDef::new(BillOfMaterials::Version).string().not_null())
                        .col(ColumnDef::new(BillOfMaterials::Status).string().not_null())
                        .col(ColumnDef::new(BillOfMaterials::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(BillOfMaterials::UpdatedAt).timestamp().null())
                        .to_owned(),
                )
                .await?;

            // Create bom_components table
            manager
                .create_table(
                    Table::create()
                        .table(BomComponents::Table)
                        .if_not_exists()
                        .col(ColumnDef::new(BomComponents::Id).uuid().primary_key().not_null())
                        .col(ColumnDef::new(BomComponents::BomId).uuid().not_null())
                        .col(ColumnDef::new(BomComponents::ComponentId).uuid().not_null())
                        .col(ColumnDef::new(BomComponents::Quantity).integer().not_null())
                        .foreign_key(
                            ForeignKey::create()
                                .name("fk_bom_components_bom_id")
                                .from(BomComponents::Table, BomComponents::BomId)
                                .to(BillOfMaterials::Table, BillOfMaterials::Id)
                                .on_delete(ForeignKeyAction::Cascade)
                                .on_update(ForeignKeyAction::Cascade),
                        )
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Drop bom_components table
            manager
                .drop_table(Table::drop().table(BomComponents::Table).to_owned())
                .await?;
            
            // Drop bill_of_materials table
            manager
                .drop_table(Table::drop().table(BillOfMaterials::Table).to_owned())
                .await
        }
    }

    #[derive(DeriveIden)]
    enum BillOfMaterials {
        Table,
        Id,
        ProductId,
        Name,
        Description,
        Version,
        Status,
        CreatedAt,
        UpdatedAt,
    }

    #[derive(DeriveIden)]
    enum BomComponents {
        Table,
        Id,
        BomId,
        ComponentId,
        Quantity,
    }
}

mod m20230101_000009_create_work_orders_table {
    use sea_orm_migration::prelude::*;

    #[derive(DeriveMigrationName)]
    pub struct Migration;

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create work_orders table
            manager
                .create_table(
                    Table::create()
                        .table(WorkOrders::Table)
                        .if_not_exists()
                        .col(ColumnDef::new(WorkOrders::Id).uuid().primary_key().not_null())
                        .col(ColumnDef::new(WorkOrders::BomId).uuid().not_null())
                        .col(ColumnDef::new(WorkOrders::Status).string().not_null())
                        .col(ColumnDef::new(WorkOrders::QuantityPlanned).integer().not_null())
                        .col(ColumnDef::new(WorkOrders::QuantityCompleted).integer().not_null().default(0))
                        .col(ColumnDef::new(WorkOrders::StartDate).timestamp().null())
                        .col(ColumnDef::new(WorkOrders::EndDate).timestamp().null())
                        .col(ColumnDef::new(WorkOrders::AssignedTo).uuid().null())
                        .col(ColumnDef::new(WorkOrders::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(WorkOrders::UpdatedAt).timestamp().null())
                        .foreign_key(
                            ForeignKey::create()
                                .name("fk_work_orders_bom_id")
                                .from(WorkOrders::Table, WorkOrders::BomId)
                                .to("bill_of_materials", "id")
                                .on_delete(ForeignKeyAction::Restrict)
                                .on_update(ForeignKeyAction::Cascade),
                        )
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Drop work_orders table
            manager
                .drop_table(Table::drop().table(WorkOrders::Table).to_owned())
                .await
        }
    }

    #[derive(DeriveIden)]
    enum WorkOrders {
        Table,
        Id,
        BomId,
        Status,
        QuantityPlanned,
        QuantityCompleted,
        StartDate,
        EndDate,
        AssignedTo,
        CreatedAt,
        UpdatedAt,
    }
}

mod m20230101_000010_create_users_table {
    use sea_orm_migration::prelude::*;

    #[derive(DeriveMigrationName)]
    pub struct Migration;

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create users table
            manager
                .create_table(
                    Table::create()
                        .table(Users::Table)
                        .if_not_exists()
                        .col(ColumnDef::new(Users::Id).uuid().primary_key().not_null())
                        .col(ColumnDef::new(Users::Email).string().not_null().unique_key())
                        .col(ColumnDef::new(Users::Name).string().not_null())
                        .col(ColumnDef::new(Users::PasswordHash).string().not_null())
                        .col(ColumnDef::new(Users::Role).string().not_null())
                        .col(ColumnDef::new(Users::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(Users::UpdatedAt).timestamp().null())
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Drop users table
            manager
                .drop_table(Table::drop().table(Users::Table).to_owned())
                .await
        }
    }

    #[derive(DeriveIden)]
    enum Users {
        Table,
        Id,
        Email,
        Name,
        PasswordHash,
        Role,
        CreatedAt,
        UpdatedAt,
    }
}

// Database migration CLI runner
pub async fn run_migration(db_url: &str) -> Result<()> {
    info!("Setting up database connection for migrations");
    
    let mut opt = ConnectOptions::new(db_url);
    opt.max_connections(10)
       .min_connections(1)
       .connect_timeout(Duration::from_secs(30))
       .acquire_timeout(Duration::from_secs(30))
       .idle_timeout(Duration::from_secs(300))
       .max_lifetime(Duration::from_secs(1800))
       .sqlx_logging(true);

    let db = Database::connect(opt).await?;
    
    info!("Running database migrations");
    
    let result = Migrator::up(&db, None).await;
    
    match result {
        Ok(_) => {
            info!("Migrations completed successfully");
            Ok(())
        },
        Err(e) => {
            error!("Migration failed: {}", e);
            Err(e.into())
        }
    }
}