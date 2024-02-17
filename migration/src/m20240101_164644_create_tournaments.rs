use sea_orm::Iterable;
use sea_orm_migration::prelude::*;

use crate::enums::*;
use crate::extension::postgres::Type;
use crate::macros::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_type(
                Type::create()
                    .as_enum(CompetitionLevel::Table)
                    .values(CompetitionLevel::iter())
                    .to_owned(),
            )
            .await?;

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
                    .col(
                        ColumnDef::new(Competition::Level)
                            .custom(CompetitionLevel::Table)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Competition::EndedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Competition::StartDate).date().not_null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(PhantomCompetition::Table)
                    .col(
                        ColumnDef::new(PhantomCompetition::Id)
                            .integer()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PhantomCompetition::Name).string().not_null())
                    .col(ColumnDef::new(PhantomCompetition::Date).date().not_null())
                    .col(
                        ColumnDef::new(PhantomCompetition::Level)
                            .custom(CompetitionLevel::Table)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Round::Table)
                    .col(
                        ColumnDef::new(Round::Id)
                            .integer()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Round::RoundNumber).integer().not_null())
                    .col(ColumnDef::new(Round::CompetitionId).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Round::Table, Round::CompetitionId)
                            .to(Competition::Table, Competition::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(Round::Date)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
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
                            .auto_increment()
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
                    .col(
                        ColumnDef::new(PlayerRoundScore::Throws)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerRoundScore::Division)
                            .custom(Division::Table)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerRoundScore::Placement)
                            .integer()
                            .not_null(),
                    )
                    .index(
                        Index::create()
                            .name("unique_round_score_competition_round")
                            .col(PlayerRoundScore::PDGANumber)
                            .col(PlayerRoundScore::CompetitionId)
                            .col(PlayerRoundScore::Round)
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
                            .auto_increment()
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
                    .index(
                        Index::create()
                            .name("unique_pdga_competition_id")
                            .col(PlayerInCompetition::PDGANumber)
                            .col(PlayerInCompetition::CompetitionId)
                            .unique(),
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
                            .default(3),
                    )
                    .col(
                        ColumnDef::new(FantasyTournament::BenchSize)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(PhantomCompetitionInFantasyTournament::Table)
                    .col(
                        ColumnDef::new(PhantomCompetitionInFantasyTournament::Id)
                            .integer()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PhantomCompetitionInFantasyTournament::FantasyTournamentId)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_phantom_competition_in_fantasy_tournament_tournament")
                            .from(
                                PhantomCompetitionInFantasyTournament::Table,
                                PhantomCompetitionInFantasyTournament::FantasyTournamentId,
                            )
                            .to(FantasyTournament::Table, FantasyTournament::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(PhantomCompetitionInFantasyTournament::PhantomCompetitionId)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name(
                                "fk_phantom_competition_in_fantasy_tournament_phantom_competition",
                            )
                            .from(
                                PhantomCompetitionInFantasyTournament::Table,
                                PhantomCompetitionInFantasyTournament::PhantomCompetitionId,
                            )
                            .to(PhantomCompetition::Table, PhantomCompetition::Id)
                            .on_delete(ForeignKeyAction::Cascade),
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
                    .table(PlayerDivisionInFantasyTournament::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlayerDivisionInFantasyTournament::PlayerPDGANumber)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PlayerDivisionInFantasyTournament::FantasyTournamentId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlayerDivisionInFantasyTournament::Division)
                            .custom(Division::Table)
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_player_division_to_player")
                            .to(Player::Table, Player::PDGANumber)
                            .from(
                                PlayerDivisionInFantasyTournament::Table,
                                PlayerDivisionInFantasyTournament::PlayerPDGANumber,
                            ),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_player_division_to_fantasy_tournament")
                            .from(
                                PlayerDivisionInFantasyTournament::Table,
                                PlayerDivisionInFantasyTournament::FantasyTournamentId,
                            )
                            .to(FantasyTournament::Table, FantasyTournament::Id),
                    )
                    .index(
                        Index::create()
                            .name("idx_player_division_to_player")
                            .col(PlayerDivisionInFantasyTournament::PlayerPDGANumber)
                            .col(PlayerDivisionInFantasyTournament::FantasyTournamentId)
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
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CompetitionInFantasyTournament::CompetitionId)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_competition_in_fantasy_tournament_competition")
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
        drop_table!(PhantomCompetition, manager);
        drop_table!(Competition, manager);
        drop_type!(CompetitionStatus, manager);
        drop_table!(FantasyTournament, manager);
        drop_table!(FantasyTournamentDivision, manager);
        drop_table!(PlayerDivisionInFantasyTournament, manager);
        drop_table!(CompetitionInFantasyTournament, manager);
        drop_table!(Round, manager);
        drop_type!(CompetitionLevel, manager);

        /*manager.drop_foreign_key(
            ForeignKey::drop()
                .name("fk_competition_to_competition_in_tournament").to_owned()
        ).await?;*/
        Ok(())
    }
}
