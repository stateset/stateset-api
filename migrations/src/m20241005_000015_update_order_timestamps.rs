use sea_orm_migration::{prelude::*, sea_orm::DatabaseBackend};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20241005_000015_update_order_timestamps"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager
                    .alter_table(
                        Table::alter()
                            .table(Orders::Table)
                            .modify_column(
                                ColumnDef::new(Orders::OrderDate)
                                    .timestamp_with_time_zone()
                                    .not_null(),
                            )
                            .modify_column(
                                ColumnDef::new(Orders::CreatedAt)
                                    .timestamp_with_time_zone()
                                    .not_null(),
                            )
                            .modify_column(
                                ColumnDef::new(Orders::UpdatedAt)
                                    .timestamp_with_time_zone()
                                    .null(),
                            )
                            .to_owned(),
                    )
                    .await?;

                manager
                    .alter_table(
                        Table::alter()
                            .table(OrderItems::Table)
                            .modify_column(
                                ColumnDef::new(OrderItems::CreatedAt)
                                    .timestamp_with_time_zone()
                                    .not_null(),
                            )
                            .modify_column(
                                ColumnDef::new(OrderItems::UpdatedAt)
                                    .timestamp_with_time_zone()
                                    .null(),
                            )
                            .to_owned(),
                    )
                    .await?;
            }
            _ => {
                // SQLite and other backends do not distinguish timezones for timestamp columns.
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager
                    .alter_table(
                        Table::alter()
                            .table(Orders::Table)
                            .modify_column(ColumnDef::new(Orders::OrderDate).timestamp().not_null())
                            .modify_column(ColumnDef::new(Orders::CreatedAt).timestamp().not_null())
                            .modify_column(ColumnDef::new(Orders::UpdatedAt).timestamp().null())
                            .to_owned(),
                    )
                    .await?;

                manager
                    .alter_table(
                        Table::alter()
                            .table(OrderItems::Table)
                            .modify_column(
                                ColumnDef::new(OrderItems::CreatedAt).timestamp().not_null(),
                            )
                            .modify_column(ColumnDef::new(OrderItems::UpdatedAt).timestamp().null())
                            .to_owned(),
                    )
                    .await?;
            }
            _ => {
                // No-op for backends without timezone distinction.
            }
        }

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Orders {
    Table,
    OrderDate,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum OrderItems {
    Table,
    CreatedAt,
    UpdatedAt,
}
