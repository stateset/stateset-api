use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ============================================
        // ORDERS TABLE INDEXES
        // ============================================

        // Composite index for customer orders filtered by status
        manager
            .create_index(
                Index::create()
                    .name("idx_orders_customer_status")
                    .table(Orders::Table)
                    .col(Orders::CustomerId)
                    .col(Orders::Status)
                    .to_owned(),
            )
            .await?;

        // Index for recent orders sorted by creation date with status
        manager
            .create_index(
                Index::create()
                    .name("idx_orders_created_status")
                    .table(Orders::Table)
                    .col((Orders::CreatedAt, IndexOrder::Desc))
                    .col(Orders::Status)
                    .to_owned(),
            )
            .await?;

        // Index for order number lookups (frequent)
        manager
            .create_index(
                Index::create()
                    .name("idx_orders_order_number")
                    .table(Orders::Table)
                    .col(Orders::OrderNumber)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // ============================================
        // ORDER_ITEMS TABLE INDEXES
        // ============================================

        // Foreign key index for order items (CRITICAL for joins)
        manager
            .create_index(
                Index::create()
                    .name("idx_order_items_order_id")
                    .table(OrderItems::Table)
                    .col(OrderItems::OrderId)
                    .to_owned(),
            )
            .await?;

        // Index for product lookup in order items
        manager
            .create_index(
                Index::create()
                    .name("idx_order_items_product_id")
                    .table(OrderItems::Table)
                    .col(OrderItems::ProductId)
                    .to_owned(),
            )
            .await?;

        // ============================================
        // SHIPMENTS TABLE INDEXES
        // ============================================

        // Index for tracking number lookups (very frequent)
        manager
            .create_index(
                Index::create()
                    .name("idx_shipments_tracking_number")
                    .table(Shipments::Table)
                    .col(Shipments::TrackingNumber)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Foreign key index for shipments by order
        manager
            .create_index(
                Index::create()
                    .name("idx_shipments_order_id")
                    .table(Shipments::Table)
                    .col(Shipments::OrderId)
                    .to_owned(),
            )
            .await?;

        // Index for active shipments by status
        manager
            .create_index(
                Index::create()
                    .name("idx_shipments_status")
                    .table(Shipments::Table)
                    .col(Shipments::Status)
                    .to_owned(),
            )
            .await?;

        // ============================================
        // RETURNS TABLE INDEXES
        // ============================================

        // Composite index for returns by order and status
        manager
            .create_index(
                Index::create()
                    .name("idx_returns_order_status")
                    .table(Returns::Table)
                    .col(Returns::OrderId)
                    .col(Returns::Status)
                    .to_owned(),
            )
            .await?;

        // Index for pending returns sorted by creation date
        manager
            .create_index(
                Index::create()
                    .name("idx_returns_created_status")
                    .table(Returns::Table)
                    .col((Returns::CreatedAt, IndexOrder::Desc))
                    .col(Returns::Status)
                    .to_owned(),
            )
            .await?;

        // ============================================
        // INVENTORY TABLE INDEXES
        // ============================================

        // Composite index for inventory by location and item
        manager
            .create_index(
                Index::create()
                    .name("idx_inventory_location_item")
                    .table(Inventory::Table)
                    .col(Inventory::LocationId)
                    .col(Inventory::ItemId)
                    .to_owned(),
            )
            .await?;

        // Index for low stock queries
        manager
            .create_index(
                Index::create()
                    .name("idx_inventory_quantity")
                    .table(Inventory::Table)
                    .col(Inventory::QuantityAvailable)
                    .to_owned(),
            )
            .await?;

        // ============================================
        // WORK_ORDERS TABLE INDEXES
        // ============================================

        // Composite index for work orders by status and scheduled date
        manager
            .create_index(
                Index::create()
                    .name("idx_work_orders_status_scheduled")
                    .table(WorkOrders::Table)
                    .col(WorkOrders::Status)
                    .col(WorkOrders::ScheduledStart)
                    .to_owned(),
            )
            .await?;

        // Index for work orders by assignee
        manager
            .create_index(
                Index::create()
                    .name("idx_work_orders_assignee")
                    .table(WorkOrders::Table)
                    .col(WorkOrders::AssignedTo)
                    .to_owned(),
            )
            .await?;

        // ============================================
        // PRODUCTS TABLE INDEXES
        // ============================================

        // Index for product SKU lookups (very frequent)
        manager
            .create_index(
                Index::create()
                    .name("idx_products_sku")
                    .table(Products::Table)
                    .col(Products::Sku)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index for active products
        manager
            .create_index(
                Index::create()
                    .name("idx_products_active")
                    .table(Products::Table)
                    .col(Products::Active)
                    .to_owned(),
            )
            .await?;

        // ============================================
        // CUSTOMERS TABLE INDEXES
        // ============================================

        // Index for customer email lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_customers_email")
                    .table(Customers::Table)
                    .col(Customers::Email)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // ============================================
        // AUTH TABLES INDEXES
        // ============================================

        // Index for API keys lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_api_keys_key_hash")
                    .table(ApiKeys::Table)
                    .col(ApiKeys::KeyHash)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index for refresh tokens lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_refresh_tokens_token_hash")
                    .table(RefreshTokens::Table)
                    .col(RefreshTokens::TokenHash)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index for user tokens (foreign key)
        manager
            .create_index(
                Index::create()
                    .name("idx_refresh_tokens_user_id")
                    .table(RefreshTokens::Table)
                    .col(RefreshTokens::UserId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop all indexes in reverse order

        // Auth tables
        manager
            .drop_index(Index::drop().name("idx_refresh_tokens_user_id").to_owned())
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_refresh_tokens_token_hash")
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(Index::drop().name("idx_api_keys_key_hash").to_owned())
            .await?;

        // Customers
        manager
            .drop_index(Index::drop().name("idx_customers_email").to_owned())
            .await?;

        // Products
        manager
            .drop_index(Index::drop().name("idx_products_active").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx_products_sku").to_owned())
            .await?;

        // Work Orders
        manager
            .drop_index(Index::drop().name("idx_work_orders_assignee").to_owned())
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_work_orders_status_scheduled")
                    .to_owned(),
            )
            .await?;

        // Inventory
        manager
            .drop_index(Index::drop().name("idx_inventory_quantity").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx_inventory_location_item").to_owned())
            .await?;

        // Returns
        manager
            .drop_index(Index::drop().name("idx_returns_created_status").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx_returns_order_status").to_owned())
            .await?;

        // Shipments
        manager
            .drop_index(Index::drop().name("idx_shipments_status").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx_shipments_order_id").to_owned())
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_shipments_tracking_number")
                    .to_owned(),
            )
            .await?;

        // Order Items
        manager
            .drop_index(Index::drop().name("idx_order_items_product_id").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx_order_items_order_id").to_owned())
            .await?;

        // Orders
        manager
            .drop_index(Index::drop().name("idx_orders_order_number").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx_orders_created_status").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx_orders_customer_status").to_owned())
            .await?;

        Ok(())
    }
}

// Table identifiers
#[derive(Iden)]
enum Orders {
    Table,
    CustomerId,
    Status,
    CreatedAt,
    OrderNumber,
}

#[derive(Iden)]
enum OrderItems {
    Table,
    OrderId,
    ProductId,
}

#[derive(Iden)]
enum Shipments {
    Table,
    TrackingNumber,
    OrderId,
    Status,
}

#[derive(Iden)]
enum Returns {
    Table,
    OrderId,
    Status,
    CreatedAt,
}

#[derive(Iden)]
enum Inventory {
    Table,
    LocationId,
    ItemId,
    QuantityAvailable,
}

#[derive(Iden)]
enum WorkOrders {
    Table,
    Status,
    ScheduledStart,
    AssignedTo,
}

#[derive(Iden)]
enum Products {
    Table,
    Sku,
    Active,
}

#[derive(Iden)]
enum Customers {
    Table,
    Email,
}

#[derive(Iden)]
enum ApiKeys {
    Table,
    KeyHash,
}

#[derive(Iden)]
enum RefreshTokens {
    Table,
    TokenHash,
    UserId,
}
