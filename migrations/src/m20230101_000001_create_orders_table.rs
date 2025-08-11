use sea_orm_migration::prelude::*;

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
                    .col(
                        ColumnDef::new(Orders::OrderNumber)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Orders::CustomerId).uuid().not_null())
                    .col(ColumnDef::new(Orders::Status).string().not_null())
                    .col(ColumnDef::new(Orders::OrderDate).date_time().not_null())
                    .col(
                        ColumnDef::new(Orders::TotalAmount)
                            .decimal()
                            .not_null()
                            .default(0.0),
                    )
                    .col(
                        ColumnDef::new(Orders::Currency)
                            .string()
                            .not_null()
                            .default("USD"),
                    )
                    .col(
                        ColumnDef::new(Orders::PaymentStatus)
                            .string()
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(Orders::FulfillmentStatus)
                            .string()
                            .not_null()
                            .default("unfulfilled"),
                    )
                    .col(ColumnDef::new(Orders::Notes).text().null())
                    .col(ColumnDef::new(Orders::ShippingAddress).text().null())
                    .col(ColumnDef::new(Orders::BillingAddress).text().null())
                    .col(
                        ColumnDef::new(Orders::IsArchived)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
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
pub enum Orders {
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
    Notes,
    ShippingAddress,
    BillingAddress,
    IsArchived,
    CreatedAt,
    UpdatedAt,
}
