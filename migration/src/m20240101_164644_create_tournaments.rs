use crate::extension::postgres::Type;
use sea_orm::Iterable;

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
                    .as_enum(CompetitionStatus::Table)
                    .values(CompetitionStatus::iter().skip(1))
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Competition::Table)
                    .col(ColumnDef::new(Competition::Id).integer().primary_key())
                    .col(ColumnDef::new(Competition::Name).string().not_null())
                    .col(
                        ColumnDef::new(Competition::Status)
                            .custom(CompetitionStatus::Table)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Competition::Rounds).integer().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Round::Table)
                    .col(ColumnDef::new(Round::Id).integer().auto_increment().primary_key())
                    .col(ColumnDef::new(Round::RoundNumber).integer().not_null())
                    .col(ColumnDef::new(Round::CompetitionId).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Round::Table, Round::CompetitionId)
                            .to(Competition::Table, Competition::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(ColumnDef::new(Round::Date).date().not_null())
                    .index(
                        Index::create()
                            .name("unique_competition_round")
                            .col(Round::CompetitionId)
                            .col(Round::RoundNumber)
                            .col(Round::Date)
                            .unique(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(PlayerRoundScore::Table)
                    .col(
                        ColumnDef::new(PlayerRoundScore::Id)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PlayerRoundScore::PDGANumber)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(PlayerRoundScore::Table, PlayerRoundScore::PDGANumber)
                            .to(Player::Table, Player::PDGANumber)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(PlayerRoundScore::CompetitionId)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(PlayerRoundScore::Table, PlayerRoundScore::CompetitionId)
                            .to(Competition::Table, Competition::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(ColumnDef::new(PlayerRoundScore::Round).integer().not_null())
                    .col(ColumnDef::new(PlayerRoundScore::Score).integer().not_null())
                    .index(
                        Index::create()
                            .name("unique_pdga_tournament_round")
                            .col(PlayerRoundScore::PDGANumber)
                            .col(PlayerRoundScore::CompetitionId)
                            .col(PlayerRoundScore::Round)
                            .unique(),
                    )
                    .index(
                        Index::create()
                            .name("unique_tournament_round_score")
                            .col(PlayerRoundScore::CompetitionId)
                            .col(PlayerRoundScore::PDGANumber)
                            .unique(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(PlayerInCompetition::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlayerInCompetition::Id)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PlayerInCompetition::PDGANumber)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(PlayerInCompetition::Table, PlayerInCompetition::PDGANumber)
                            .to(Player::Table, Player::PDGANumber)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(PlayerInCompetition::CompetitionId)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                PlayerInCompetition::Table,
                                PlayerInCompetition::CompetitionId,
                            )
                            .to(Competition::Table, Competition::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    // Add unique constraint
                    .index(
                        Index::create()
                            .name("unique_pdga_tournament_id")
                            .col(PlayerInCompetition::PDGANumber)
                            .col(PlayerInCompetition::CompetitionId)
                            .unique(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_player_in_competition_player_round_score")
                            .from(
                                PlayerInCompetition::Table,
                                (
                                    PlayerInCompetition::CompetitionId,
                                    PlayerInCompetition::PDGANumber,
                                ),
                            )
                            .to(
                                PlayerRoundScore::Table,
                                (
                                    PlayerInCompetition::CompetitionId,
                                    PlayerInCompetition::PDGANumber,
                                ),
                            )
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(PlayerInCompetition::Division)
                            .custom(Division::Table)
                            .not_null(),
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
                    .col(ColumnDef::new(FantasyTournament::Name).string().not_null())
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

        manager
            .create_table(
                Table::create()
                    .table(FantasyTournamentDivision::Table)
                    .col(
                        ColumnDef::new(FantasyTournamentDivision::Id)
                            .integer()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(FantasyTournamentDivision::FantasyTournamentId)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_tournament_division_tournament")
                            .from(
                                FantasyTournamentDivision::Table,
                                FantasyTournamentDivision::FantasyTournamentId,
                            )
                            .to(FantasyTournament::Table, FantasyTournament::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(FantasyTournamentDivision::Division)
                            .custom(Division::Table)
                            .not_null(),
                    )
                    .index(
                        Index::create()
                            .name("unique_tournament_division")
                            .col(FantasyTournamentDivision::FantasyTournamentId)
                            .col(FantasyTournamentDivision::Division)
                            .unique(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CompetitionInFantasyTournament::Table)
                    .col(
                        ColumnDef::new(CompetitionInFantasyTournament::Id)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CompetitionInFantasyTournament::CompetitionId)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                CompetitionInFantasyTournament::Table,
                                CompetitionInFantasyTournament::CompetitionId,
                            )
                            .to(Competition::Table, Competition::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(CompetitionInFantasyTournament::FantasyTournamentId)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                CompetitionInFantasyTournament::Table,
                                CompetitionInFantasyTournament::FantasyTournamentId,
                            )
                            .to(FantasyTournament::Table, FantasyTournament::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .name("unique_competition_tournament")
                            .col(CompetitionInFantasyTournament::CompetitionId)
                            .col(CompetitionInFantasyTournament::FantasyTournamentId)
                            .unique(),
                    )
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_table!(PlayerInCompetition, manager);
        drop_table!(PlayerRoundScore, manager);
        drop_table!(Competition, manager);
        drop_type!(CompetitionStatus, manager);
        drop_table!(FantasyTournament, manager);
        drop_table!(FantasyTournamentDivision, manager);
        drop_table!(CompetitionInFantasyTournament, manager);
        Ok(())
    }
}
