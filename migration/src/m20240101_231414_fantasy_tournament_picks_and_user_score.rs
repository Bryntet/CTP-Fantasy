use crate::enums::*;
use crate::{drop_table, drop_type};
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
                            .to(Competition::Table, Competition::Id),
                    )
                    .index(
                        Index::create()
                            .name("fantasy_pick_user_player")
                            .col(FantasyPick::User)
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
                    .table(FantasyScores::Table)
                    .col(
                        ColumnDef::new(FantasyScores::Id)
                            .integer()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(FantasyScores::User).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(FantasyScores::Table, FantasyScores::User)
                            .to(User::Table, User::Id),
                    )
                    .col(
                        ColumnDef::new(FantasyScores::FantasyTournamentId)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(FantasyScores::Table, FantasyScores::FantasyTournamentId)
                            .to(Competition::Table, Competition::Id),
                    )
                    .col(ColumnDef::new(FantasyScores::Score).integer().not_null())
                    .index(
                        Index::create()
                            .name("fantasy_scores_user_tournament")
                            .col(FantasyScores::User)
                            .col(FantasyScores::FantasyTournamentId)
                            .unique(),
                    )
                    .col(ColumnDef::new(FantasyScores::Ranking).integer().not_null())
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_table!(FantasyPick, manager);
        drop_table!(FantasyScores, manager);
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Post {
    Table,
    Id,
    Title,
    Text,
}
