use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20230101_000011_create_warranty_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create warranties table
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
                    .col(
                        ColumnDef::new(Warranties::WarrantyNumber)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Warranties::ProductId).uuid().not_null())
                    .col(ColumnDef::new(Warranties::CustomerId).uuid().not_null())
                    .col(ColumnDef::new(Warranties::OrderId).uuid().null())
                    .col(ColumnDef::new(Warranties::Status).string().not_null())
                    .col(ColumnDef::new(Warranties::StartDate).date_time().not_null())
                    .col(ColumnDef::new(Warranties::EndDate).date_time().not_null())
                    .col(ColumnDef::new(Warranties::Description).text().null())
                    .col(ColumnDef::new(Warranties::Terms).text().null())
                    .col(ColumnDef::new(Warranties::CreatedAt).timestamp().not_null())
                    .col(ColumnDef::new(Warranties::UpdatedAt).timestamp().null())
                    .to_owned(),
            )
            .await?;

        // Create warranty_claims table
        manager
            .create_table(
                Table::create()
                    .table(WarrantyClaims::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(WarrantyClaims::Id)
                            .uuid()
                            .primary_key()
                            .not_null(),
                    )
                    .col(ColumnDef::new(WarrantyClaims::WarrantyId).uuid().not_null())
                    .col(
                        ColumnDef::new(WarrantyClaims::ClaimNumber)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(WarrantyClaims::Status).string().not_null())
                    .col(
                        ColumnDef::new(WarrantyClaims::ClaimDate)
                            .date_time()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WarrantyClaims::Description)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(WarrantyClaims::Resolution).text().null())
                    .col(
                        ColumnDef::new(WarrantyClaims::ResolvedDate)
                            .date_time()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(WarrantyClaims::CreatedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(ColumnDef::new(WarrantyClaims::UpdatedAt).timestamp().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_warranty_claims_warranty_id")
                            .from(WarrantyClaims::Table, WarrantyClaims::WarrantyId)
                            .to(Warranties::Table, Warranties::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop warranty_claims table first (due to foreign key)
        manager
            .drop_table(Table::drop().table(WarrantyClaims::Table).to_owned())
            .await?;

        // Drop warranties table
        manager
            .drop_table(Table::drop().table(Warranties::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Warranties {
    Table,
    Id,
    WarrantyNumber,
    ProductId,
    CustomerId,
    OrderId,
    Status,
    StartDate,
    EndDate,
    Description,
    Terms,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum WarrantyClaims {
    Table,
    Id,
    WarrantyId,
    ClaimNumber,
    Status,
    ClaimDate,
    Description,
    Resolution,
    ResolvedDate,
    CreatedAt,
    UpdatedAt,
}
