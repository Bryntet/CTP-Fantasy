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
                    .col(
                        ColumnDef::new(Competition::Status)
                            .custom(CompetitionStatus::Table)
                            .not_null(),
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
                            .not_null()
                            .auto_increment(),
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
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_table!(PlayerInCompetition, manager);
        drop_table!(Competition, manager);
        drop_type!(CompetitionStatus, manager);
        drop_table!(FantasyTournament, manager);
        Ok(())
    }
}
