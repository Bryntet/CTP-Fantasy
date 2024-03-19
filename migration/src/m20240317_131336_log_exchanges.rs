use crate::{drop_table, enums};
use enums::{Player, PlayerTrade, User};
use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts

        manager
            .create_table(
                Table::create()
                    .table(PlayerTrade::Table)
                    .col(
                        ColumnDef::new(PlayerTrade::Id)
                            .integer()
                            .auto_increment()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PlayerTrade::User).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(PlayerTrade::Table, PlayerTrade::User)
                            .to(User::Table, User::Id),
                    )
                    .col(ColumnDef::new(PlayerTrade::Player).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(PlayerTrade::Table, PlayerTrade::Player)
                            .to(Player::Table, Player::PDGANumber),
                    )
                    .col(ColumnDef::new(PlayerTrade::Slot).integer().not_null())
                    .col(
                        ColumnDef::new(PlayerTrade::FantasyTournamentId)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(PlayerTrade::Table, PlayerTrade::FantasyTournamentId)
                            .to(enums::FantasyTournament::Table, enums::FantasyTournament::Id),
                    )
                    .col(
                        ColumnDef::new(PlayerTrade::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PlayerTrade::IsLocalSwap).boolean().not_null())
                    .col(ColumnDef::new(PlayerTrade::OtherPlayer).integer())
                    .foreign_key(
                        ForeignKey::create()
                            .from(PlayerTrade::Table, PlayerTrade::OtherPlayer)
                            .to(Player::Table, Player::PDGANumber),
                    )
                    .col(ColumnDef::new(PlayerTrade::OtherSlot).integer())
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_table!(PlayerTrade, manager);
        Ok(())
    }
}
