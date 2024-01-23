use crate::macros::*;
use sea_orm::Iterable;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

use crate::enums::*;
use crate::extension::postgres::Type;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(User::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(User::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(User::Name).string().unique_key().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_type(
                Type::create()
                    .as_enum(Division::Table)
                    .values(Division::iter().skip(1))
                    .to_owned(),
            )
            .await?;



        manager
            .create_table(
                Table::create()
                    .table(Player::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Player::PDGANumber)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Player::FirstName).string().not_null())
                    .col(ColumnDef::new(Player::LastName).string().not_null())
                    .col(ColumnDef::new(Player::Avatar).string())

                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(PlayerDivision::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlayerDivision::PlayerPDGANumber)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PlayerDivision::Division)
                            .custom(Division::Table)
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_player_division_to_player")
                            .to(Player::Table, Player::PDGANumber)
                            .from(PlayerDivision::Table, PlayerDivision::PlayerPDGANumber),
                    )
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_table!(Player, manager);
        drop_table!(PlayerDivision, manager);
        drop_table!(User, manager);
        drop_type!(Division, manager);
        Ok(())
    }
}
