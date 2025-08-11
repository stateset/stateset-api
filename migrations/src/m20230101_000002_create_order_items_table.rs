use sea_orm_migration::prelude::*;

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
                            .default(0.0),
                    )
                    .col(
                        ColumnDef::new(OrderItems::TaxRate)
                            .decimal()
                            .not_null()
                            .default(0.0),
                    )
                    .col(
                        ColumnDef::new(OrderItems::TaxAmount)
                            .decimal()
                            .not_null()
                            .default(0.0),
                    )
                    .col(
                        ColumnDef::new(OrderItems::Status)
                            .string()
                            .not_null()
                            .default("pending"),
                    )
                    .col(ColumnDef::new(OrderItems::Notes).text().null())
                    .col(ColumnDef::new(OrderItems::CreatedAt).timestamp().not_null())
                    .col(ColumnDef::new(OrderItems::UpdatedAt).timestamp().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_order_items_order_id")
                            .from(OrderItems::Table, OrderItems::OrderId)
                            .to(
                                super::m20230101_000001_create_orders_table::Orders::Table,
                                super::m20230101_000001_create_orders_table::Orders::Id,
                            )
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
