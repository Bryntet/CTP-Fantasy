use crate::drop_table;
use crate::enums::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserCookies::Table)
                    .col(
                        ColumnDef::new(UserCookies::Cookie)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UserCookies::UserId).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(UserCookies::Table, UserCookies::UserId)
                            .to(User::Table, User::Id),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(UserAuthentication::Table)
                    .col(
                        ColumnDef::new(UserAuthentication::UserId)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(UserAuthentication::Table, UserAuthentication::UserId)
                            .to(User::Table, User::Id),
                    )
                    .col(
                        ColumnDef::new(UserAuthentication::HashedPassword)
                            .string()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_table!(UserCookies, manager);
        drop_table!(UserAuthentication, manager);
        Ok(())
    }
}
