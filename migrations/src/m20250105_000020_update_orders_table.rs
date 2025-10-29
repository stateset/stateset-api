use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20250105_000020_update_orders_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager
            .has_column("orders", Orders::PaymentMethod.to_string().as_str())
            .await?
        {
            let mut col = ColumnDef::new(Orders::PaymentMethod);
            col.string_len(100).null();
            manager
                .alter_table(
                    Table::alter()
                        .table(Orders::Table)
                        .add_column(col)
                        .to_owned(),
                )
                .await?;
        }

        if !manager
            .has_column("orders", Orders::ShippingMethod.to_string().as_str())
            .await?
        {
            let mut col = ColumnDef::new(Orders::ShippingMethod);
            col.string_len(100).null();
            manager
                .alter_table(
                    Table::alter()
                        .table(Orders::Table)
                        .add_column(col)
                        .to_owned(),
                )
                .await?;
        }

        if !manager
            .has_column("orders", Orders::TrackingNumber.to_string().as_str())
            .await?
        {
            let mut col = ColumnDef::new(Orders::TrackingNumber);
            col.string_len(255).null();
            manager
                .alter_table(
                    Table::alter()
                        .table(Orders::Table)
                        .add_column(col)
                        .to_owned(),
                )
                .await?;
        }

        if !manager
            .has_column("orders", Orders::Version.to_string().as_str())
            .await?
        {
            let mut col = ColumnDef::new(Orders::Version);
            col.integer().not_null().default(1);
            manager
                .alter_table(
                    Table::alter()
                        .table(Orders::Table)
                        .add_column(col)
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager
            .has_column("orders", Orders::PaymentMethod.to_string().as_str())
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(Orders::Table)
                        .drop_column(Orders::PaymentMethod)
                        .to_owned(),
                )
                .await?;
        }

        if manager
            .has_column("orders", Orders::ShippingMethod.to_string().as_str())
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(Orders::Table)
                        .drop_column(Orders::ShippingMethod)
                        .to_owned(),
                )
                .await?;
        }

        if manager
            .has_column("orders", Orders::TrackingNumber.to_string().as_str())
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(Orders::Table)
                        .drop_column(Orders::TrackingNumber)
                        .to_owned(),
                )
                .await?;
        }

        if manager
            .has_column("orders", Orders::Version.to_string().as_str())
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(Orders::Table)
                        .drop_column(Orders::Version)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Orders {
    Table,
    PaymentMethod,
    ShippingMethod,
    TrackingNumber,
    Version,
}
