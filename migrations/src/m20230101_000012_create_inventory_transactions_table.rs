use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create inventory_transactions table for detailed tracking
        manager
            .create_table(
                Table::create()
                    .table(InventoryTransactions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(InventoryTransactions::Id)
                            .uuid()
                            .primary_key()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryTransactions::ProductId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryTransactions::LocationId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryTransactions::Type)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryTransactions::Quantity)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryTransactions::PreviousQuantity)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryTransactions::NewQuantity)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryTransactions::ReferenceId)
                            .uuid()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(InventoryTransactions::ReferenceType)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(InventoryTransactions::Reason)
                            .string()
                            .null(),
                    )
                    .col(ColumnDef::new(InventoryTransactions::Notes).text().null())
                    .col(
                        ColumnDef::new(InventoryTransactions::CreatedBy)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryTransactions::CreatedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Create inventory_reservations table
        manager
            .create_table(
                Table::create()
                    .table(InventoryReservations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(InventoryReservations::Id)
                            .uuid()
                            .primary_key()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryReservations::ProductId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryReservations::LocationId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryReservations::Quantity)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryReservations::Status)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryReservations::ReferenceId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryReservations::ReferenceType)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryReservations::ExpiresAt)
                            .timestamp()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(InventoryReservations::CreatedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryReservations::UpdatedAt)
                            .timestamp()
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop inventory_reservations table
        manager
            .drop_table(Table::drop().table(InventoryReservations::Table).to_owned())
            .await?;

        // Drop inventory_transactions table
        manager
            .drop_table(Table::drop().table(InventoryTransactions::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum InventoryTransactions {
    Table,
    Id,
    ProductId,
    LocationId,
    Type,
    Quantity,
    PreviousQuantity,
    NewQuantity,
    ReferenceId,
    ReferenceType,
    Reason,
    Notes,
    CreatedBy,
    CreatedAt,
}

#[derive(DeriveIden)]
enum InventoryReservations {
    Table,
    Id,
    ProductId,
    LocationId,
    Quantity,
    Status,
    ReferenceId,
    ReferenceType,
    ExpiresAt,
    CreatedAt,
    UpdatedAt,
}
