use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20250105_000017_create_products_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Products::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Products::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Products::Name).string_len(255).not_null())
                    .col(ColumnDef::new(Products::Description).text().null())
                    .col(
                        ColumnDef::new(Products::Sku)
                            .string_len(100)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Products::Price)
                            .decimal_len(19, 4)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Products::Currency)
                            .string_len(3)
                            .not_null()
                            .default("USD"),
                    )
                    .col(ColumnDef::new(Products::WeightKg).decimal_len(19, 4).null())
                    .col(
                        ColumnDef::new(Products::DimensionsCm)
                            .string_len(255)
                            .null(),
                    )
                    .col(ColumnDef::new(Products::Barcode).string_len(255).null())
                    .col(ColumnDef::new(Products::Brand).string_len(255).null())
                    .col(
                        ColumnDef::new(Products::Manufacturer)
                            .string_len(255)
                            .null(),
                    )
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
                    .col(ColumnDef::new(Products::ImageUrl).string_len(1024).null())
                    .col(ColumnDef::new(Products::CategoryId).uuid().null())
                    .col(ColumnDef::new(Products::ReorderPoint).integer().null())
                    .col(ColumnDef::new(Products::TaxRate).decimal_len(19, 4).null())
                    .col(
                        ColumnDef::new(Products::CostPrice)
                            .decimal_len(19, 4)
                            .null(),
                    )
                    .col(ColumnDef::new(Products::Msrp).decimal_len(19, 4).null())
                    .col(ColumnDef::new(Products::Tags).text().null())
                    .col(ColumnDef::new(Products::MetaTitle).string_len(255).null())
                    .col(
                        ColumnDef::new(Products::MetaDescription)
                            .string_len(1024)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Products::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Products::UpdatedAt)
                            .timestamp()
                            .null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_products_is_active")
                    .table(Products::Table)
                    .col(Products::IsActive)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Products::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Products {
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
