use anyhow::Result;
use sea_orm::{ConnectOptions, Database};
use sea_orm_migration::prelude::*;
use std::time::Duration;
use tracing::{error, info};

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
            Box::new(m20230101_000011_create_commerce_tables::Migration),
            Box::new(m20230101_000012_create_warranties_table::Migration),
            Box::new(m20230101_000013_create_inventory_locations_table::Migration),
            Box::new(m20230101_000014_create_procurement_tables::Migration),
            Box::new(m20230101_000015_create_manufacturing_tables::Migration),
            Box::new(m20230101_000016_create_inventory_balances_table::Migration),
            Box::new(m20230101_000017_create_payments_table::Migration),
        ]
    }
}

// Migration implementations

mod m20230101_000001_create_orders_table {

    use sea_orm_migration::prelude::*;

    pub struct Migration;

    impl MigrationName for Migration {
        fn name(&self) -> &str {
            "m20230101_000001_create_orders_table"
        }
    }

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create orders table aligned with entities::order Model
            manager
                .create_table(
                    Table::create()
                        .table(Orders::Table)
                        .if_not_exists()
                        .col(ColumnDef::new(Orders::Id).uuid().primary_key().not_null())
                        .col(ColumnDef::new(Orders::OrderNumber).string().not_null())
                        .col(ColumnDef::new(Orders::CustomerId).uuid().not_null())
                        .col(ColumnDef::new(Orders::Status).string().not_null())
                        .col(ColumnDef::new(Orders::OrderDate).timestamp().not_null())
                        .col(
                            ColumnDef::new(Orders::TotalAmount)
                                .decimal()
                                .not_null()
                                .default(0),
                        )
                        .col(ColumnDef::new(Orders::Currency).string().not_null())
                        .col(ColumnDef::new(Orders::PaymentStatus).string().not_null())
                        .col(
                            ColumnDef::new(Orders::FulfillmentStatus)
                                .string()
                                .not_null(),
                        )
                        .col(ColumnDef::new(Orders::PaymentMethod).string().null())
                        .col(ColumnDef::new(Orders::ShippingMethod).string().null())
                        .col(ColumnDef::new(Orders::TrackingNumber).string().null())
                        .col(ColumnDef::new(Orders::Notes).string().null())
                        .col(ColumnDef::new(Orders::ShippingAddress).string().null())
                        .col(ColumnDef::new(Orders::BillingAddress).string().null())
                        .col(
                            ColumnDef::new(Orders::IsArchived)
                                .boolean()
                                .not_null()
                                .default(false),
                        )
                        .col(ColumnDef::new(Orders::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(Orders::UpdatedAt).timestamp().null())
                        .col(
                            ColumnDef::new(Orders::Version)
                                .integer()
                                .not_null()
                                .default(1),
                        )
                        .to_owned(),
                )
                .await?;

            // Useful indexes
            manager
                .create_index(
                    Index::create()
                        .if_not_exists()
                        .name("idx_orders_customer_id")
                        .table(Orders::Table)
                        .col(Orders::CustomerId)
                        .to_owned(),
                )
                .await?;

            manager
                .create_index(
                    Index::create()
                        .if_not_exists()
                        .name("idx_orders_status")
                        .table(Orders::Table)
                        .col(Orders::Status)
                        .to_owned(),
                )
                .await?;

            manager
                .create_index(
                    Index::create()
                        .if_not_exists()
                        .name("idx_orders_created_at")
                        .table(Orders::Table)
                        .col(Orders::CreatedAt)
                        .to_owned(),
                )
                .await?;

            manager
                .create_index(
                    Index::create()
                        .if_not_exists()
                        .name("idx_orders_order_number")
                        .table(Orders::Table)
                        .col(Orders::OrderNumber)
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
    pub(super) enum Orders {
        Table,
        Id,
        OrderNumber,
        CustomerId,
        Status,
        OrderDate,
        TotalAmount,
        Currency,
        PaymentStatus,
        FulfillmentStatus,
        PaymentMethod,
        ShippingMethod,
        TrackingNumber,
        Notes,
        ShippingAddress,
        BillingAddress,
        IsArchived,
        CreatedAt,
        UpdatedAt,
        Version,
    }
}

mod m20230101_000002_create_order_items_table {

    use sea_orm_migration::prelude::*;

    pub struct Migration;

    impl MigrationName for Migration {
        fn name(&self) -> &str {
            "m20230101_000002_create_order_items_table"
        }
    }

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create order_items table aligned with entities::order_item Model
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
                        .col(ColumnDef::new(OrderItems::Sku).string().not_null())
                        .col(ColumnDef::new(OrderItems::Name).string().not_null())
                        .col(ColumnDef::new(OrderItems::Quantity).integer().not_null())
                        .col(ColumnDef::new(OrderItems::UnitPrice).decimal().not_null())
                        .col(ColumnDef::new(OrderItems::TotalPrice).decimal().not_null())
                        .col(
                            ColumnDef::new(OrderItems::Discount)
                                .decimal()
                                .not_null()
                                .default(0),
                        )
                        .col(
                            ColumnDef::new(OrderItems::TaxRate)
                                .decimal()
                                .not_null()
                                .default(0),
                        )
                        .col(
                            ColumnDef::new(OrderItems::TaxAmount)
                                .decimal()
                                .not_null()
                                .default(0),
                        )
                        .col(ColumnDef::new(OrderItems::Status).string().not_null())
                        .col(ColumnDef::new(OrderItems::Notes).string().null())
                        .col(ColumnDef::new(OrderItems::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(OrderItems::UpdatedAt).timestamp().null())
                        .foreign_key(
                            ForeignKey::create()
                                .name("fk_order_items_order_id")
                                .from(OrderItems::Table, OrderItems::OrderId)
                                .to(Orders::Table, Orders::Id)
                                .on_delete(ForeignKeyAction::Cascade)
                                .on_update(ForeignKeyAction::Cascade),
                        )
                        .to_owned(),
                )
                .await?;

            manager
                .create_index(
                    Index::create()
                        .if_not_exists()
                        .name("idx_order_items_order_id")
                        .table(OrderItems::Table)
                        .col(OrderItems::OrderId)
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
        Sku,
        Name,
        Quantity,
        UnitPrice,
        TotalPrice,
        Discount,
        TaxRate,
        TaxAmount,
        Status,
        Notes,
        CreatedAt,
        UpdatedAt,
    }

    #[derive(DeriveIden)]
    enum Orders {
        Table,
        Id,
    }
}

mod m20230101_000003_create_customers_table {
    use sea_orm_migration::prelude::*;

    pub struct Migration;

    impl MigrationName for Migration {
        fn name(&self) -> &str {
            "m20230101_000003_create_customers_table"
        }
    }

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create customers table
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

    pub struct Migration;

    impl MigrationName for Migration {
        fn name(&self) -> &str {
            "m20230101_000004_create_products_table"
        }
    }

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create products table with all columns matching entities::product::Model
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
                        .col(
                            ColumnDef::new(Products::Currency)
                                .string()
                                .not_null()
                                .default("USD"),
                        )
                        .col(ColumnDef::new(Products::WeightKg).decimal().null())
                        .col(ColumnDef::new(Products::DimensionsCm).string().null())
                        .col(ColumnDef::new(Products::Barcode).string().null())
                        .col(ColumnDef::new(Products::Brand).string().null())
                        .col(ColumnDef::new(Products::Manufacturer).string().null())
                        .col(
                            ColumnDef::new(Products::IsActive)
                                .boolean()
                                .not_null()
                                .default(true),
                        )
                        .col(
                            ColumnDef::new(Products::IsDigital)
                                .boolean()
                                .not_null()
                                .default(false),
                        )
                        .col(ColumnDef::new(Products::ImageUrl).string().null())
                        .col(ColumnDef::new(Products::CategoryId).uuid().null())
                        .col(ColumnDef::new(Products::ReorderPoint).integer().null())
                        .col(ColumnDef::new(Products::TaxRate).decimal().null())
                        .col(ColumnDef::new(Products::CostPrice).decimal().null())
                        .col(ColumnDef::new(Products::Msrp).decimal().null())
                        .col(ColumnDef::new(Products::Tags).string().null())
                        .col(ColumnDef::new(Products::MetaTitle).string().null())
                        .col(ColumnDef::new(Products::MetaDescription).string().null())
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
        Currency,
        WeightKg,
        DimensionsCm,
        Barcode,
        Brand,
        Manufacturer,
        IsActive,
        IsDigital,
        ImageUrl,
        CategoryId,
        ReorderPoint,
        TaxRate,
        CostPrice,
        Msrp,
        Tags,
        MetaTitle,
        MetaDescription,
        CreatedAt,
        UpdatedAt,
    }
}

mod m20230101_000005_create_inventory_table {
    use sea_orm_migration::prelude::*;

    pub struct Migration;

    impl MigrationName for Migration {
        fn name(&self) -> &str {
            "m20230101_000005_create_inventory_table"
        }
    }

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create inventory table
            manager
                .create_table(
                    Table::create()
                        .table(Inventory::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(Inventory::Id)
                                .uuid()
                                .primary_key()
                                .not_null(),
                        )
                        .col(ColumnDef::new(Inventory::ProductId).uuid().not_null())
                        .col(ColumnDef::new(Inventory::WarehouseId).uuid().not_null())
                        .col(ColumnDef::new(Inventory::Quantity).integer().not_null())
                        .col(
                            ColumnDef::new(Inventory::Reserved)
                                .integer()
                                .not_null()
                                .default(0),
                        )
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

    pub struct Migration;

    impl MigrationName for Migration {
        fn name(&self) -> &str {
            "m20230101_000006_create_returns_table"
        }
    }

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
    use super::m20230101_000001_create_orders_table::Orders as OrdersTable;
    use sea_orm_migration::prelude::*;

    pub struct Migration;

    impl MigrationName for Migration {
        fn name(&self) -> &str {
            "m20230101_000007_create_shipments_table"
        }
    }

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create shipments table that mirrors models::shipment::Model
            manager
                .create_table(
                    Table::create()
                        .table(Shipments::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(Shipments::Id)
                                .uuid()
                                .primary_key()
                                .not_null(),
                        )
                        .col(ColumnDef::new(Shipments::OrderId).uuid().not_null())
                        .col(
                            ColumnDef::new(Shipments::TrackingNumber)
                                .string()
                                .not_null(),
                        )
                        .col(ColumnDef::new(Shipments::Carrier).string().not_null())
                        .col(ColumnDef::new(Shipments::Status).string().not_null())
                        .col(
                            ColumnDef::new(Shipments::ShippingAddress)
                                .string()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(Shipments::ShippingMethod)
                                .string()
                                .not_null(),
                        )
                        .col(ColumnDef::new(Shipments::WeightKg).float().null())
                        .col(ColumnDef::new(Shipments::DimensionsCm).string().null())
                        .col(ColumnDef::new(Shipments::Notes).string().null())
                        .col(ColumnDef::new(Shipments::ShippedAt).timestamp().null())
                        .col(
                            ColumnDef::new(Shipments::EstimatedDelivery)
                                .timestamp()
                                .null(),
                        )
                        .col(ColumnDef::new(Shipments::DeliveredAt).timestamp().null())
                        .col(ColumnDef::new(Shipments::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(Shipments::UpdatedAt).timestamp().not_null())
                        .col(ColumnDef::new(Shipments::CreatedBy).string().null())
                        .col(ColumnDef::new(Shipments::RecipientName).string().not_null())
                        .col(ColumnDef::new(Shipments::RecipientEmail).string().null())
                        .col(ColumnDef::new(Shipments::RecipientPhone).string().null())
                        .col(ColumnDef::new(Shipments::TrackingUrl).string().null())
                        .col(ColumnDef::new(Shipments::ShippingCost).decimal().null())
                        .col(ColumnDef::new(Shipments::InsuranceAmount).decimal().null())
                        .col(
                            ColumnDef::new(Shipments::IsSignatureRequired)
                                .boolean()
                                .not_null()
                                .default(false),
                        )
                        .foreign_key(
                            ForeignKey::create()
                                .name("fk_shipments_order_id")
                                .from(Shipments::Table, Shipments::OrderId)
                                .to(OrdersTable::Table, OrdersTable::Id)
                                .on_delete(ForeignKeyAction::Cascade)
                                .on_update(ForeignKeyAction::Cascade),
                        )
                        .to_owned(),
                )
                .await?;

            manager
                .create_index(
                    Index::create()
                        .if_not_exists()
                        .name("idx_shipments_order_id")
                        .table(Shipments::Table)
                        .col(Shipments::OrderId)
                        .to_owned(),
                )
                .await?;

            manager
                .create_index(
                    Index::create()
                        .if_not_exists()
                        .name("idx_shipments_tracking_number")
                        .table(Shipments::Table)
                        .col(Shipments::TrackingNumber)
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
        TrackingNumber,
        Carrier,
        Status,
        ShippingAddress,
        ShippingMethod,
        WeightKg,
        DimensionsCm,
        Notes,
        ShippedAt,
        EstimatedDelivery,
        DeliveredAt,
        CreatedAt,
        UpdatedAt,
        CreatedBy,
        RecipientName,
        RecipientEmail,
        RecipientPhone,
        TrackingUrl,
        ShippingCost,
        InsuranceAmount,
        IsSignatureRequired,
    }
}

mod m20230101_000008_create_bill_of_materials_table {
    use sea_orm_migration::prelude::*;

    pub struct Migration;

    impl MigrationName for Migration {
        fn name(&self) -> &str {
            "m20230101_000008_create_bill_of_materials_table"
        }
    }

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create bill_of_materials table
            manager
                .create_table(
                    Table::create()
                        .table(BillOfMaterials::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(BillOfMaterials::Id)
                                .uuid()
                                .primary_key()
                                .not_null(),
                        )
                        .col(ColumnDef::new(BillOfMaterials::ProductId).uuid().not_null())
                        .col(ColumnDef::new(BillOfMaterials::Name).string().not_null())
                        .col(ColumnDef::new(BillOfMaterials::Description).string().null())
                        .col(ColumnDef::new(BillOfMaterials::Version).string().not_null())
                        .col(ColumnDef::new(BillOfMaterials::Status).string().not_null())
                        .col(
                            ColumnDef::new(BillOfMaterials::CreatedAt)
                                .timestamp()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(BillOfMaterials::UpdatedAt)
                                .timestamp()
                                .null(),
                        )
                        .to_owned(),
                )
                .await?;

            // Create bom_components table
            manager
                .create_table(
                    Table::create()
                        .table(BomComponents::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(BomComponents::Id)
                                .uuid()
                                .primary_key()
                                .not_null(),
                        )
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

    pub struct Migration;

    impl MigrationName for Migration {
        fn name(&self) -> &str {
            "m20230101_000009_create_work_orders_table"
        }
    }

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create work_orders table
            manager
                .create_table(
                    Table::create()
                        .table(WorkOrders::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(WorkOrders::Id)
                                .uuid()
                                .primary_key()
                                .not_null(),
                        )
                        .col(ColumnDef::new(WorkOrders::BomId).uuid().not_null())
                        .col(ColumnDef::new(WorkOrders::Status).string().not_null())
                        .col(
                            ColumnDef::new(WorkOrders::QuantityPlanned)
                                .integer()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(WorkOrders::QuantityCompleted)
                                .integer()
                                .not_null()
                                .default(0),
                        )
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

    pub struct Migration;

    impl MigrationName for Migration {
        fn name(&self) -> &str {
            "m20230101_000010_create_users_table"
        }
    }

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
                        .col(
                            ColumnDef::new(Users::Email)
                                .string()
                                .not_null()
                                .unique_key(),
                        )
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

mod m20230101_000011_create_commerce_tables {
    use sea_orm_migration::prelude::*;

    pub struct Migration;

    impl MigrationName for Migration {
        fn name(&self) -> &str {
            "m20230101_000011_create_commerce_tables"
        }
    }

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create product_variants table
            manager
                .create_table(
                    Table::create()
                        .table(ProductVariants::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(ProductVariants::Id)
                                .uuid()
                                .primary_key()
                                .not_null(),
                        )
                        .col(ColumnDef::new(ProductVariants::ProductId).uuid().not_null())
                        .col(ColumnDef::new(ProductVariants::Sku).string().not_null())
                        .col(ColumnDef::new(ProductVariants::Name).string().not_null())
                        .col(ColumnDef::new(ProductVariants::Price).decimal().not_null())
                        .col(
                            ColumnDef::new(ProductVariants::CompareAtPrice)
                                .decimal()
                                .null(),
                        )
                        .col(ColumnDef::new(ProductVariants::Cost).decimal().null())
                        .col(ColumnDef::new(ProductVariants::Weight).float().null())
                        .col(ColumnDef::new(ProductVariants::Dimensions).json().null())
                        .col(
                            ColumnDef::new(ProductVariants::Options)
                                .json()
                                .not_null()
                                .default("{}"),
                        )
                        .col(
                            ColumnDef::new(ProductVariants::InventoryTracking)
                                .boolean()
                                .not_null()
                                .default(true),
                        )
                        .col(
                            ColumnDef::new(ProductVariants::Position)
                                .integer()
                                .not_null()
                                .default(0),
                        )
                        .col(
                            ColumnDef::new(ProductVariants::CreatedAt)
                                .timestamp()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(ProductVariants::UpdatedAt)
                                .timestamp()
                                .not_null(),
                        )
                        .to_owned(),
                )
                .await?;

            // Create carts table
            manager
                .create_table(
                    Table::create()
                        .table(Carts::Table)
                        .if_not_exists()
                        .col(ColumnDef::new(Carts::Id).uuid().primary_key().not_null())
                        .col(ColumnDef::new(Carts::SessionId).string().null())
                        .col(ColumnDef::new(Carts::CustomerId).uuid().null())
                        .col(
                            ColumnDef::new(Carts::Currency)
                                .string()
                                .not_null()
                                .default("USD"),
                        )
                        .col(
                            ColumnDef::new(Carts::Subtotal)
                                .decimal()
                                .not_null()
                                .default(0),
                        )
                        .col(
                            ColumnDef::new(Carts::TaxTotal)
                                .decimal()
                                .not_null()
                                .default(0),
                        )
                        .col(
                            ColumnDef::new(Carts::ShippingTotal)
                                .decimal()
                                .not_null()
                                .default(0),
                        )
                        .col(
                            ColumnDef::new(Carts::DiscountTotal)
                                .decimal()
                                .not_null()
                                .default(0),
                        )
                        .col(ColumnDef::new(Carts::Total).decimal().not_null().default(0))
                        .col(ColumnDef::new(Carts::Metadata).json().null())
                        .col(
                            ColumnDef::new(Carts::Status)
                                .string()
                                .not_null()
                                .default("active"),
                        )
                        .col(ColumnDef::new(Carts::ExpiresAt).timestamp().not_null())
                        .col(ColumnDef::new(Carts::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(Carts::UpdatedAt).timestamp().not_null())
                        .to_owned(),
                )
                .await?;

            // Create cart_items table
            manager
                .create_table(
                    Table::create()
                        .table(CartItems::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(CartItems::Id)
                                .uuid()
                                .primary_key()
                                .not_null(),
                        )
                        .col(ColumnDef::new(CartItems::CartId).uuid().not_null())
                        .col(ColumnDef::new(CartItems::VariantId).uuid().not_null())
                        .col(ColumnDef::new(CartItems::Quantity).integer().not_null())
                        .col(ColumnDef::new(CartItems::UnitPrice).decimal().not_null())
                        .col(ColumnDef::new(CartItems::LineTotal).decimal().not_null())
                        .col(
                            ColumnDef::new(CartItems::DiscountAmount)
                                .decimal()
                                .not_null()
                                .default(0),
                        )
                        .col(ColumnDef::new(CartItems::Metadata).json().null())
                        .col(ColumnDef::new(CartItems::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(CartItems::UpdatedAt).timestamp().not_null())
                        .foreign_key(
                            ForeignKey::create()
                                .name("fk_cart_items_cart_id")
                                .from(CartItems::Table, CartItems::CartId)
                                .to(Carts::Table, Carts::Id)
                                .on_delete(ForeignKeyAction::Cascade)
                                .on_update(ForeignKeyAction::Cascade),
                        )
                        .foreign_key(
                            ForeignKey::create()
                                .name("fk_cart_items_variant_id")
                                .from(CartItems::Table, CartItems::VariantId)
                                .to(ProductVariants::Table, ProductVariants::Id)
                                .on_delete(ForeignKeyAction::Cascade)
                                .on_update(ForeignKeyAction::Cascade),
                        )
                        .to_owned(),
                )
                .await?;

            // Create indexes
            manager
                .create_index(
                    Index::create()
                        .if_not_exists()
                        .name("idx_product_variants_sku")
                        .table(ProductVariants::Table)
                        .col(ProductVariants::Sku)
                        .to_owned(),
                )
                .await?;

            manager
                .create_index(
                    Index::create()
                        .if_not_exists()
                        .name("idx_carts_customer_id")
                        .table(Carts::Table)
                        .col(Carts::CustomerId)
                        .to_owned(),
                )
                .await?;

            manager
                .create_index(
                    Index::create()
                        .if_not_exists()
                        .name("idx_cart_items_cart_id")
                        .table(CartItems::Table)
                        .col(CartItems::CartId)
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            manager
                .drop_table(Table::drop().table(CartItems::Table).to_owned())
                .await?;
            manager
                .drop_table(Table::drop().table(Carts::Table).to_owned())
                .await?;
            manager
                .drop_table(Table::drop().table(ProductVariants::Table).to_owned())
                .await
        }
    }

    #[derive(DeriveIden)]
    enum ProductVariants {
        Table,
        Id,
        ProductId,
        Sku,
        Name,
        Price,
        CompareAtPrice,
        Cost,
        Weight,
        Dimensions,
        Options,
        InventoryTracking,
        Position,
        CreatedAt,
        UpdatedAt,
    }

    #[derive(DeriveIden)]
    enum Carts {
        Table,
        Id,
        SessionId,
        CustomerId,
        Currency,
        Subtotal,
        TaxTotal,
        ShippingTotal,
        DiscountTotal,
        Total,
        Metadata,
        Status,
        ExpiresAt,
        CreatedAt,
        UpdatedAt,
    }

    #[derive(DeriveIden)]
    enum CartItems {
        Table,
        Id,
        CartId,
        VariantId,
        Quantity,
        UnitPrice,
        LineTotal,
        DiscountAmount,
        Metadata,
        CreatedAt,
        UpdatedAt,
    }
}

mod m20230101_000012_create_warranties_table {
    use sea_orm_migration::prelude::*;

    pub struct Migration;

    impl MigrationName for Migration {
        fn name(&self) -> &str {
            "m20230101_000012_create_warranties_table"
        }
    }

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            manager
                .create_table(
                    Table::create()
                        .table(Warranties::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(Warranties::Id)
                                .uuid()
                                .primary_key()
                                .not_null(),
                        )
                        .col(ColumnDef::new(Warranties::OrderId).uuid().not_null())
                        .col(ColumnDef::new(Warranties::ProductId).uuid().not_null())
                        .col(ColumnDef::new(Warranties::Status).string().not_null())
                        .col(ColumnDef::new(Warranties::WarrantyType).string().not_null())
                        .col(ColumnDef::new(Warranties::StartDate).timestamp().not_null())
                        .col(ColumnDef::new(Warranties::EndDate).timestamp().not_null())
                        .col(ColumnDef::new(Warranties::Terms).string().null())
                        .col(ColumnDef::new(Warranties::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(Warranties::UpdatedAt).timestamp().null())
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            manager
                .drop_table(Table::drop().table(Warranties::Table).to_owned())
                .await
        }
    }

    #[derive(DeriveIden)]
    enum Warranties {
        Table,
        Id,
        OrderId,
        ProductId,
        Status,
        WarrantyType,
        StartDate,
        EndDate,
        Terms,
        CreatedAt,
        UpdatedAt,
    }
}

mod m20230101_000013_create_inventory_locations_table {
    use sea_orm_migration::prelude::*;

    pub struct Migration;

    impl MigrationName for Migration {
        fn name(&self) -> &str {
            "m20230101_000013_create_inventory_locations_table"
        }
    }

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Migration matches the simple inventory_location entity
            // which only has location_id and location_name
            manager
                .create_table(
                    Table::create()
                        .table(InventoryLocations::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(InventoryLocations::LocationId)
                                .integer()
                                .primary_key()
                                .auto_increment()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(InventoryLocations::LocationName)
                                .string()
                                .not_null(),
                        )
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            manager
                .drop_table(Table::drop().table(InventoryLocations::Table).to_owned())
                .await
        }
    }

    #[derive(DeriveIden)]
    enum InventoryLocations {
        Table,
        LocationId,
        LocationName,
    }
}

mod m20230101_000014_create_procurement_tables {
    use sea_orm_migration::prelude::*;

    pub struct Migration;

    impl MigrationName for Migration {
        fn name(&self) -> &str {
            "m20230101_000014_create_procurement_tables"
        }
    }

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create suppliers table
            manager
                .create_table(
                    Table::create()
                        .table(Suppliers::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(Suppliers::Id)
                                .uuid()
                                .primary_key()
                                .not_null(),
                        )
                        .col(ColumnDef::new(Suppliers::Name).string().not_null())
                        .col(ColumnDef::new(Suppliers::ContactEmail).string().null())
                        .col(ColumnDef::new(Suppliers::ContactPhone).string().null())
                        .col(ColumnDef::new(Suppliers::Address).string().null())
                        .col(
                            ColumnDef::new(Suppliers::Status)
                                .string()
                                .not_null()
                                .default("active"),
                        )
                        .col(ColumnDef::new(Suppliers::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(Suppliers::UpdatedAt).timestamp().null())
                        .to_owned(),
                )
                .await?;

            // Create purchase_order_headers table
            manager
                .create_table(
                    Table::create()
                        .table(PurchaseOrderHeaders::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(PurchaseOrderHeaders::PoHeaderId)
                                .big_integer()
                                .primary_key()
                                .auto_increment()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(PurchaseOrderHeaders::PoNumber)
                                .string()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(PurchaseOrderHeaders::TypeCode)
                                .string()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(PurchaseOrderHeaders::VendorId)
                                .big_integer()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(PurchaseOrderHeaders::AgentId)
                                .big_integer()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(PurchaseOrderHeaders::ApprovedFlag)
                                .boolean()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(PurchaseOrderHeaders::CreatedAt)
                                .timestamp()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(PurchaseOrderHeaders::UpdatedAt)
                                .timestamp()
                                .not_null(),
                        )
                        .to_owned(),
                )
                .await?;

            // Create purchase_order_lines table
            manager
                .create_table(
                    Table::create()
                        .table(PurchaseOrderLines::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(PurchaseOrderLines::PoLineId)
                                .big_integer()
                                .primary_key()
                                .auto_increment()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(PurchaseOrderLines::PoHeaderId)
                                .big_integer()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(PurchaseOrderLines::ItemId)
                                .big_integer()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(PurchaseOrderLines::Quantity)
                                .integer()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(PurchaseOrderLines::UnitPrice)
                                .decimal()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(PurchaseOrderLines::CreatedAt)
                                .timestamp()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(PurchaseOrderLines::UpdatedAt)
                                .timestamp()
                                .not_null(),
                        )
                        .to_owned(),
                )
                .await?;

            // Create asns table (Advance Shipping Notices)
            manager
                .create_table(
                    Table::create()
                        .table(Asns::Table)
                        .if_not_exists()
                        .col(ColumnDef::new(Asns::Id).uuid().primary_key().not_null())
                        .col(ColumnDef::new(Asns::AsnNumber).string().not_null())
                        .col(ColumnDef::new(Asns::PurchaseOrderId).uuid().null())
                        .col(ColumnDef::new(Asns::SupplierId).uuid().null())
                        .col(
                            ColumnDef::new(Asns::Status)
                                .string()
                                .not_null()
                                .default("pending"),
                        )
                        .col(ColumnDef::new(Asns::ShippedDate).timestamp().null())
                        .col(
                            ColumnDef::new(Asns::ExpectedDeliveryDate)
                                .timestamp()
                                .null(),
                        )
                        .col(ColumnDef::new(Asns::Carrier).string().null())
                        .col(ColumnDef::new(Asns::TrackingNumber).string().null())
                        .col(ColumnDef::new(Asns::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(Asns::UpdatedAt).timestamp().not_null())
                        .to_owned(),
                )
                .await?;

            // Create asn_items table
            manager
                .create_table(
                    Table::create()
                        .table(AsnItems::Table)
                        .if_not_exists()
                        .col(ColumnDef::new(AsnItems::Id).uuid().primary_key().not_null())
                        .col(ColumnDef::new(AsnItems::AsnId).uuid().not_null())
                        .col(
                            ColumnDef::new(AsnItems::PurchaseOrderItemId)
                                .uuid()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(AsnItems::QuantityShipped)
                                .integer()
                                .not_null(),
                        )
                        .col(ColumnDef::new(AsnItems::PackageNumber).string().null())
                        .col(ColumnDef::new(AsnItems::LotNumber).string().null())
                        .col(
                            ColumnDef::new(AsnItems::Status)
                                .string()
                                .not_null()
                                .default("pending"),
                        )
                        .col(ColumnDef::new(AsnItems::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(AsnItems::UpdatedAt).timestamp().not_null())
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            manager
                .drop_table(Table::drop().table(AsnItems::Table).to_owned())
                .await?;
            manager
                .drop_table(Table::drop().table(Asns::Table).to_owned())
                .await?;
            manager
                .drop_table(Table::drop().table(PurchaseOrderLines::Table).to_owned())
                .await?;
            manager
                .drop_table(Table::drop().table(PurchaseOrderHeaders::Table).to_owned())
                .await?;
            manager
                .drop_table(Table::drop().table(Suppliers::Table).to_owned())
                .await
        }
    }

    #[derive(DeriveIden)]
    enum Suppliers {
        Table,
        Id,
        Name,
        ContactEmail,
        ContactPhone,
        Address,
        Status,
        CreatedAt,
        UpdatedAt,
    }

    #[derive(DeriveIden)]
    enum PurchaseOrderHeaders {
        Table,
        PoHeaderId,
        PoNumber,
        TypeCode,
        VendorId,
        AgentId,
        ApprovedFlag,
        CreatedAt,
        UpdatedAt,
    }

    #[derive(DeriveIden)]
    enum PurchaseOrderLines {
        Table,
        PoLineId,
        PoHeaderId,
        ItemId,
        Quantity,
        UnitPrice,
        CreatedAt,
        UpdatedAt,
    }

    #[derive(DeriveIden)]
    enum Asns {
        Table,
        Id,
        AsnNumber,
        PurchaseOrderId,
        SupplierId,
        Status,
        ShippedDate,
        ExpectedDeliveryDate,
        Carrier,
        TrackingNumber,
        CreatedAt,
        UpdatedAt,
    }

    #[derive(DeriveIden)]
    enum AsnItems {
        Table,
        Id,
        AsnId,
        PurchaseOrderItemId,
        QuantityShipped,
        PackageNumber,
        LotNumber,
        Status,
        CreatedAt,
        UpdatedAt,
    }
}

mod m20230101_000015_create_manufacturing_tables {
    use sea_orm_migration::prelude::*;

    pub struct Migration;

    impl MigrationName for Migration {
        fn name(&self) -> &str {
            "m20230101_000015_create_manufacturing_tables"
        }
    }

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create manufacture_orders table
            manager
                .create_table(
                    Table::create()
                        .table(ManufactureOrders::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(ManufactureOrders::Id)
                                .uuid()
                                .primary_key()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(ManufactureOrders::OrderNumber)
                                .string()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(ManufactureOrders::ProductId)
                                .uuid()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(ManufactureOrders::Quantity)
                                .integer()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(ManufactureOrders::Status)
                                .string()
                                .not_null()
                                .default("draft"),
                        )
                        .col(ColumnDef::new(ManufactureOrders::Priority).string().null())
                        .col(
                            ColumnDef::new(ManufactureOrders::ScheduledStartDate)
                                .timestamp()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(ManufactureOrders::ScheduledEndDate)
                                .timestamp()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(ManufactureOrders::ActualStartDate)
                                .timestamp()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(ManufactureOrders::ActualEndDate)
                                .timestamp()
                                .null(),
                        )
                        .col(ColumnDef::new(ManufactureOrders::Notes).string().null())
                        .col(
                            ColumnDef::new(ManufactureOrders::CreatedAt)
                                .timestamp()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(ManufactureOrders::UpdatedAt)
                                .timestamp()
                                .null(),
                        )
                        .to_owned(),
                )
                .await?;

            // Create manufacture_order_line_items table
            manager
                .create_table(
                    Table::create()
                        .table(ManufactureOrderLineItems::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(ManufactureOrderLineItems::Id)
                                .uuid()
                                .primary_key()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(ManufactureOrderLineItems::ManufactureOrderId)
                                .uuid()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(ManufactureOrderLineItems::ProductId)
                                .uuid()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(ManufactureOrderLineItems::Quantity)
                                .integer()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(ManufactureOrderLineItems::UnitCost)
                                .decimal()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(ManufactureOrderLineItems::Status)
                                .string()
                                .not_null()
                                .default("pending"),
                        )
                        .col(
                            ColumnDef::new(ManufactureOrderLineItems::CreatedAt)
                                .timestamp()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(ManufactureOrderLineItems::UpdatedAt)
                                .timestamp()
                                .null(),
                        )
                        .to_owned(),
                )
                .await?;

            // Create item_master table - matches entities/item_master.rs
            manager
                .create_table(
                    Table::create()
                        .table(ItemMaster::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(ItemMaster::InventoryItemId)
                                .big_integer()
                                .primary_key()
                                .auto_increment()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(ItemMaster::OrganizationId)
                                .big_integer()
                                .not_null()
                                .default(1),
                        )
                        .col(ColumnDef::new(ItemMaster::ItemNumber).string().not_null())
                        .col(ColumnDef::new(ItemMaster::Description).string().null())
                        .col(ColumnDef::new(ItemMaster::PrimaryUomCode).string().null())
                        .col(ColumnDef::new(ItemMaster::ItemType).string().null())
                        .col(ColumnDef::new(ItemMaster::StatusCode).string().null())
                        .col(ColumnDef::new(ItemMaster::LeadTimeWeeks).integer().null())
                        .col(ColumnDef::new(ItemMaster::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(ItemMaster::UpdatedAt).timestamp().not_null())
                        .to_owned(),
                )
                .await?;

            // Create inventory_items table
            manager
                .create_table(
                    Table::create()
                        .table(InventoryItems::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(InventoryItems::Id)
                                .uuid()
                                .primary_key()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(InventoryItems::ItemMasterId)
                                .uuid()
                                .not_null(),
                        )
                        .col(ColumnDef::new(InventoryItems::LocationId).uuid().null())
                        .col(
                            ColumnDef::new(InventoryItems::QuantityOnHand)
                                .integer()
                                .not_null()
                                .default(0),
                        )
                        .col(
                            ColumnDef::new(InventoryItems::QuantityReserved)
                                .integer()
                                .not_null()
                                .default(0),
                        )
                        .col(
                            ColumnDef::new(InventoryItems::QuantityAvailable)
                                .integer()
                                .not_null()
                                .default(0),
                        )
                        .col(
                            ColumnDef::new(InventoryItems::ReorderPoint)
                                .integer()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(InventoryItems::ReorderQuantity)
                                .integer()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(InventoryItems::CreatedAt)
                                .timestamp()
                                .not_null(),
                        )
                        .col(ColumnDef::new(InventoryItems::UpdatedAt).timestamp().null())
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            manager
                .drop_table(Table::drop().table(InventoryItems::Table).to_owned())
                .await?;
            manager
                .drop_table(Table::drop().table(ItemMaster::Table).to_owned())
                .await?;
            manager
                .drop_table(
                    Table::drop()
                        .table(ManufactureOrderLineItems::Table)
                        .to_owned(),
                )
                .await?;
            manager
                .drop_table(Table::drop().table(ManufactureOrders::Table).to_owned())
                .await
        }
    }

    #[derive(DeriveIden)]
    enum ManufactureOrders {
        Table,
        Id,
        OrderNumber,
        ProductId,
        Quantity,
        Status,
        Priority,
        ScheduledStartDate,
        ScheduledEndDate,
        ActualStartDate,
        ActualEndDate,
        Notes,
        CreatedAt,
        UpdatedAt,
    }

    #[derive(DeriveIden)]
    enum ManufactureOrderLineItems {
        Table,
        Id,
        ManufactureOrderId,
        ProductId,
        Quantity,
        UnitCost,
        Status,
        CreatedAt,
        UpdatedAt,
    }

    #[derive(DeriveIden)]
    enum ItemMaster {
        Table,
        InventoryItemId,
        OrganizationId,
        ItemNumber,
        Description,
        PrimaryUomCode,
        ItemType,
        StatusCode,
        LeadTimeWeeks,
        CreatedAt,
        UpdatedAt,
    }

    #[derive(DeriveIden)]
    enum InventoryItems {
        Table,
        Id,
        ItemMasterId,
        LocationId,
        QuantityOnHand,
        QuantityReserved,
        QuantityAvailable,
        ReorderPoint,
        ReorderQuantity,
        CreatedAt,
        UpdatedAt,
    }
}

mod m20230101_000016_create_inventory_balances_table {
    use sea_orm_migration::prelude::*;

    pub struct Migration;

    impl MigrationName for Migration {
        fn name(&self) -> &str {
            "m20230101_000016_create_inventory_balances_table"
        }
    }

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create inventory_balances table - matches entities/inventory_balance.rs
            manager
                .create_table(
                    Table::create()
                        .table(InventoryBalances::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(InventoryBalances::InventoryBalanceId)
                                .big_integer()
                                .primary_key()
                                .auto_increment()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(InventoryBalances::InventoryItemId)
                                .big_integer()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(InventoryBalances::LocationId)
                                .integer()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(InventoryBalances::QuantityOnHand)
                                .decimal()
                                .not_null()
                                .default(0),
                        )
                        .col(
                            ColumnDef::new(InventoryBalances::QuantityAllocated)
                                .decimal()
                                .not_null()
                                .default(0),
                        )
                        .col(
                            ColumnDef::new(InventoryBalances::QuantityAvailable)
                                .decimal()
                                .not_null()
                                .default(0),
                        )
                        .col(
                            ColumnDef::new(InventoryBalances::ReorderPoint)
                                .decimal()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(InventoryBalances::SafetyStock)
                                .decimal()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(InventoryBalances::ReorderQuantity)
                                .decimal()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(InventoryBalances::MaxStockLevel)
                                .decimal()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(InventoryBalances::LeadTimeDays)
                                .integer()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(InventoryBalances::Version)
                                .integer()
                                .not_null()
                                .default(1),
                        )
                        .col(
                            ColumnDef::new(InventoryBalances::LastCountedAt)
                                .timestamp()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(InventoryBalances::LastCountedBy)
                                .string()
                                .null(),
                        )
                        .col(
                            ColumnDef::new(InventoryBalances::DeletedAt)
                                .timestamp()
                                .null(),
                        )
                        .col(ColumnDef::new(InventoryBalances::DeletedBy).string().null())
                        .col(
                            ColumnDef::new(InventoryBalances::CreatedAt)
                                .timestamp()
                                .not_null(),
                        )
                        .col(
                            ColumnDef::new(InventoryBalances::UpdatedAt)
                                .timestamp()
                                .not_null(),
                        )
                        .to_owned(),
                )
                .await?;

            // Add unique constraint on item_id + location_id
            manager
                .create_index(
                    Index::create()
                        .name("idx_inventory_balances_item_location")
                        .table(InventoryBalances::Table)
                        .col(InventoryBalances::InventoryItemId)
                        .col(InventoryBalances::LocationId)
                        .unique()
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            manager
                .drop_table(Table::drop().table(InventoryBalances::Table).to_owned())
                .await
        }
    }

    #[derive(DeriveIden)]
    enum InventoryBalances {
        Table,
        InventoryBalanceId,
        InventoryItemId,
        LocationId,
        QuantityOnHand,
        QuantityAllocated,
        QuantityAvailable,
        ReorderPoint,
        SafetyStock,
        ReorderQuantity,
        MaxStockLevel,
        LeadTimeDays,
        Version,
        LastCountedAt,
        LastCountedBy,
        DeletedAt,
        DeletedBy,
        CreatedAt,
        UpdatedAt,
    }
}

mod m20230101_000017_create_payments_table {
    use sea_orm_migration::prelude::*;

    pub struct Migration;

    impl MigrationName for Migration {
        fn name(&self) -> &str {
            "m20230101_000017_create_payments_table"
        }
    }

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            // Create payments table - matches models/payment.rs Model
            manager
                .create_table(
                    Table::create()
                        .table(Payments::Table)
                        .if_not_exists()
                        .col(ColumnDef::new(Payments::Id).uuid().primary_key().not_null())
                        .col(ColumnDef::new(Payments::OrderId).uuid().not_null())
                        .col(ColumnDef::new(Payments::Amount).decimal().not_null())
                        .col(ColumnDef::new(Payments::Currency).text().not_null())
                        .col(ColumnDef::new(Payments::PaymentMethod).text().not_null())
                        .col(ColumnDef::new(Payments::PaymentMethodId).string().null())
                        .col(ColumnDef::new(Payments::Status).string().not_null())
                        .col(ColumnDef::new(Payments::Description).string().null())
                        .col(ColumnDef::new(Payments::TransactionId).string().null())
                        .col(ColumnDef::new(Payments::GatewayResponse).json().null())
                        .col(
                            ColumnDef::new(Payments::RefundedAmount)
                                .decimal()
                                .not_null()
                                .default(0),
                        )
                        .col(ColumnDef::new(Payments::RefundReason).string().null())
                        .col(ColumnDef::new(Payments::CreatedAt).timestamp().not_null())
                        .col(ColumnDef::new(Payments::UpdatedAt).timestamp().null())
                        .col(ColumnDef::new(Payments::ProcessedAt).timestamp().null())
                        .to_owned(),
                )
                .await?;

            // Add indexes
            manager
                .create_index(
                    Index::create()
                        .name("idx_payments_order_id")
                        .table(Payments::Table)
                        .col(Payments::OrderId)
                        .to_owned(),
                )
                .await?;

            manager
                .create_index(
                    Index::create()
                        .name("idx_payments_status")
                        .table(Payments::Table)
                        .col(Payments::Status)
                        .to_owned(),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            manager
                .drop_table(Table::drop().table(Payments::Table).to_owned())
                .await
        }
    }

    #[derive(DeriveIden)]
    enum Payments {
        Table,
        Id,
        OrderId,
        Amount,
        Currency,
        PaymentMethod,
        PaymentMethodId,
        Status,
        Description,
        TransactionId,
        GatewayResponse,
        RefundedAmount,
        RefundReason,
        CreatedAt,
        UpdatedAt,
        ProcessedAt,
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
        }
        Err(e) => {
            error!("Migration failed: {}", e);
            Err(e.into())
        }
    }
}
