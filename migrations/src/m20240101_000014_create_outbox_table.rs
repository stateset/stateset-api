use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240101_000014_create_outbox_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OutboxEvents::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OutboxEvents::Id)
                            .uuid()
                            .primary_key()
                            .not_null(),
                    )
                    .col(ColumnDef::new(OutboxEvents::EventType).string().not_null())
                    .col(
                        ColumnDef::new(OutboxEvents::Payload)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OutboxEvents::Status)
                            .string()
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(OutboxEvents::Attempts)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(OutboxEvents::AvailableAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OutboxEvents::CreatedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(ColumnDef::new(OutboxEvents::UpdatedAt).timestamp().null())
                    .col(ColumnDef::new(OutboxEvents::Metadata).json_binary().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OutboxEvents::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum OutboxEvents {
    Table,
    Id,
    EventType,
    Payload,
    Status,
    Attempts,
    AvailableAt,
    CreatedAt,
    UpdatedAt,
    Metadata,
}
