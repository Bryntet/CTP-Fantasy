use crate::extension::postgres::Type;
use sea_orm::{EnumIter, Iterable};

use crate::enums::*;
use crate::macros::*;

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_type(
                Type::create()
                    .as_enum(TournamentStatus::Table)
                    .values(TournamentStatus::iter().skip(1))
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Tournament::Table)
                    .col(ColumnDef::new(Tournament::Id).integer().primary_key())
                    .col(
                        ColumnDef::new(Tournament::Status)
                            .custom(TournamentStatus::Table)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(PlayerInTournament::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlayerInTournament::Id)
                            .integer()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PlayerInTournament::PDGANumber)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(PlayerInTournament::Table, PlayerInTournament::PDGANumber)
                            .to(Player::Table, Player::PDGANumber)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(PlayerInTournament::TournamentId)
                            .integer()
                            .not_null()
                            .auto_increment(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(PlayerInTournament::Table, PlayerInTournament::TournamentId)
                            .to(Tournament::Table, Tournament::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    // Add unique constraint
                    .index(
                        Index::create()
                            .name("unique_pdga_tournament_id")
                            .col(PlayerInTournament::PDGANumber)
                            .col(PlayerInTournament::TournamentId)
                            .unique(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(FantasyTournament::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FantasyTournament::Id)
                            .integer()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(FantasyTournament::Owner)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(FantasyTournament::Table, FantasyTournament::Owner)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(FantasyTournament::MaxPicksPerUser)
                            .integer()
                            .not_null()
                            .default(10),
                    )
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_table!(PlayerInTournament, manager);
        drop_table!(Tournament, manager);
        drop_type!(TournamentStatus, manager);
        drop_table!(FantasyTournament, manager);
        Ok(())
    }
}
