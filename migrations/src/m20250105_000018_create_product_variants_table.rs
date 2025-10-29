use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20250105_000018_create_product_variants_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProductVariants::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProductVariants::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ProductVariants::ProductId).uuid().not_null())
                    .col(
                        ColumnDef::new(ProductVariants::Sku)
                            .string_len(100)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(ProductVariants::Name)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProductVariants::Price)
                            .decimal_len(19, 4)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProductVariants::CompareAtPrice)
                            .decimal_len(19, 4)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ProductVariants::Cost)
                            .decimal_len(19, 4)
                            .null(),
                    )
                    .col(ColumnDef::new(ProductVariants::Weight).double().null())
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
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ProductVariants::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_product_variants_product_id")
                            .from(ProductVariants::Table, ProductVariants::ProductId)
                            .to(
                                super::m20250105_000017_create_products_table::Products::Table,
                                super::m20250105_000017_create_products_table::Products::Id,
                            )
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_product_variants_product_id")
                    .table(ProductVariants::Table)
                    .col(ProductVariants::ProductId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
