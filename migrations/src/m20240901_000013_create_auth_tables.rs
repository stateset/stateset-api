use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // users
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Users::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Users::Name).string_len(255).not_null())
                    .col(
                        ColumnDef::new(Users::Email)
                            .string_len(255)
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Users::PasswordHash).text().not_null())
                    .col(ColumnDef::new(Users::TenantId).string_len(255))
                    .col(
                        ColumnDef::new(Users::Active)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(Users::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Users::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // user_roles
        manager
            .create_table(
                Table::create()
                    .table(UserRoles::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserRoles::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UserRoles::UserId).uuid().not_null())
                    .col(
                        ColumnDef::new(UserRoles::RoleName)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserRoles::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_roles_user")
                            .from(UserRoles::Table, UserRoles::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_user_roles_user_id")
                    .table(UserRoles::Table)
                    .col(UserRoles::UserId)
                    .to_owned(),
            )
            .await?;

        // refresh_tokens
        manager
            .create_table(
                Table::create()
                    .table(RefreshTokens::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RefreshTokens::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(RefreshTokens::UserId).uuid().not_null())
                    .col(
                        ColumnDef::new(RefreshTokens::TokenId)
                            .string_len(255)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(RefreshTokens::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RefreshTokens::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RefreshTokens::Revoked)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_refresh_tokens_user")
                            .from(RefreshTokens::Table, RefreshTokens::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_refresh_tokens_user_id")
                    .table(RefreshTokens::Table)
                    .col(RefreshTokens::UserId)
                    .to_owned(),
            )
            .await?;

        // api_keys
        manager
            .create_table(
                Table::create()
                    .table(ApiKeys::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(ApiKeys::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(ApiKeys::Name).string_len(255).not_null())
                    .col(ColumnDef::new(ApiKeys::KeyHash).text().not_null())
                    .col(ColumnDef::new(ApiKeys::UserId).uuid().not_null())
                    .col(ColumnDef::new(ApiKeys::TenantId).string_len(255))
                    .col(
                        ColumnDef::new(ApiKeys::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ApiKeys::ExpiresAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(ApiKeys::LastUsedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(ApiKeys::Revoked)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_api_keys_user")
                            .from(ApiKeys::Table, ApiKeys::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_api_keys_user_id")
                    .table(ApiKeys::Table)
                    .col(ApiKeys::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_api_keys_revoked")
                    .table(ApiKeys::Table)
                    .col(ApiKeys::Revoked)
                    .to_owned(),
            )
            .await?;

        // api_key_permissions
        manager
            .create_table(
                Table::create()
                    .table(ApiKeyPermissions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ApiKeyPermissions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ApiKeyPermissions::ApiKeyId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ApiKeyPermissions::Permission)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ApiKeyPermissions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_api_key_permissions_api_key")
                            .from(ApiKeyPermissions::Table, ApiKeyPermissions::ApiKeyId)
                            .to(ApiKeys::Table, ApiKeys::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_api_key_permissions_api_key_id")
                    .table(ApiKeyPermissions::Table)
                    .col(ApiKeyPermissions::ApiKeyId)
                    .to_owned(),
            )
            .await?;

        // password_reset_tokens
        manager
            .create_table(
                Table::create()
                    .table(PasswordResetTokens::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PasswordResetTokens::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PasswordResetTokens::UserId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PasswordResetTokens::TokenHash)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PasswordResetTokens::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PasswordResetTokens::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PasswordResetTokens::UsedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_password_reset_tokens_user")
                            .from(PasswordResetTokens::Table, PasswordResetTokens::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_password_reset_tokens_user_id")
                    .table(PasswordResetTokens::Table)
                    .col(PasswordResetTokens::UserId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApiKeyPermissions::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ApiKeys::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(PasswordResetTokens::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(RefreshTokens::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(UserRoles::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Users::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum Users {
    Table,
    Id,
    Name,
    Email,
    PasswordHash,
    TenantId,
    Active,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum UserRoles {
    Table,
    Id,
    UserId,
    RoleName,
    CreatedAt,
}

#[derive(Iden)]
enum RefreshTokens {
    Table,
    Id,
    UserId,
    TokenId,
    CreatedAt,
    ExpiresAt,
    Revoked,
}

#[derive(Iden)]
enum ApiKeys {
    Table,
    Id,
    Name,
    KeyHash,
    UserId,
    TenantId,
    CreatedAt,
    ExpiresAt,
    LastUsedAt,
    Revoked,
}

#[derive(Iden)]
enum ApiKeyPermissions {
    Table,
    Id,
    ApiKeyId,
    Permission,
    CreatedAt,
}

#[derive(Iden)]
enum PasswordResetTokens {
    Table,
    Id,
    UserId,
    TokenHash,
    ExpiresAt,
    CreatedAt,
    UsedAt,
}
