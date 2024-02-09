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
                    .table(FantasyPick::Table)
                    .col(
                        ColumnDef::new(FantasyPick::Id)
                            .integer()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(FantasyPick::User).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(FantasyPick::Table, FantasyPick::User)
                            .to(User::Table, User::Id),
                    )
                    .col(ColumnDef::new(FantasyPick::Player).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(FantasyPick::Table, FantasyPick::Player)
                            .to(Player::Table, Player::PDGANumber),
                    )
                    .col(
                        ColumnDef::new(FantasyPick::FantasyTournamentId)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(FantasyPick::Table, FantasyPick::FantasyTournamentId)
                            .to(FantasyTournament::Table, FantasyTournament::Id),
                    )
                    .index(
                        Index::create()
                            .name("fantasy_pick_player_tournament")
                            .col(FantasyPick::Player)
                            .col(FantasyPick::FantasyTournamentId)
                            .unique(),
                    )
                    .col(ColumnDef::new(FantasyPick::PickNumber).integer().not_null())
                    .col(
                        ColumnDef::new(FantasyPick::Division)
                            .custom(Division::Table)
                            .not_null(),
                    )
                    .col(ColumnDef::new(FantasyPick::Benched).boolean().not_null())
                    .index(
                        Index::create()
                            .name("fantasy_pick_user_tournament")
                            .col(FantasyPick::User)
                            .col(FantasyPick::FantasyTournamentId)
                            .col(FantasyPick::PickNumber)
                            .col(FantasyPick::Division)
                            .unique(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(UserCompetitionScoreInFantasyTournament::Table)
                    .col(
                        ColumnDef::new(UserCompetitionScoreInFantasyTournament::Id)
                            .integer()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(UserCompetitionScoreInFantasyTournament::User)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                UserCompetitionScoreInFantasyTournament::Table,
                                UserCompetitionScoreInFantasyTournament::User,
                            )
                            .to(User::Table, User::Id),
                    )
                    .col(
                        ColumnDef::new(
                            UserCompetitionScoreInFantasyTournament::FantasyTournamentId,
                        )
                        .integer()
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserCompetitionScoreInFantasyTournament::Score)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserCompetitionScoreInFantasyTournament::CompetitionId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserCompetitionScoreInFantasyTournament::PdgaNumber)
                            .integer()
                            .not_null(),)
                    .index(
                        Index::create()
                            .name("fantasy_scores_user_tournament")
                            .col(UserCompetitionScoreInFantasyTournament::User)
                            .col(UserCompetitionScoreInFantasyTournament::FantasyTournamentId)
                            .col(UserCompetitionScoreInFantasyTournament::CompetitionId)
                            .col(UserCompetitionScoreInFantasyTournament::PdgaNumber)
                            .unique(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                UserCompetitionScoreInFantasyTournament::Table,
                                UserCompetitionScoreInFantasyTournament::FantasyTournamentId,
                            )
                            .to(FantasyTournament::Table, FantasyTournament::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fantasy_scores_competition")
                            .from(
                                UserCompetitionScoreInFantasyTournament::Table,
                                UserCompetitionScoreInFantasyTournament::CompetitionId,
                            )
                            .to(Competition::Table, Competition::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fantasy_scores_pdga")
                            .from(
                                UserCompetitionScoreInFantasyTournament::Table,
                                UserCompetitionScoreInFantasyTournament::PdgaNumber,
                            )
                            .to(Player::Table, Player::PDGANumber),)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_table!(FantasyPick, manager);
        drop_table!(UserCompetitionScoreInFantasyTournament, manager);
        Ok(())
    }
}
