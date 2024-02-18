use crate::enums::*;
use crate::extension::postgres::Type;
use crate::{drop_table, drop_type};
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Iterable;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_type(
                Type::create()
                    .as_enum(FantasyTournamentInvitationStatus::Table)
                    .values(FantasyTournamentInvitationStatus::iter().skip(1))
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(UserInFantasyTournament::Table)
                    .col(
                        ColumnDef::new(UserInFantasyTournament::Id)
                            .integer()
                            .auto_increment()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(UserInFantasyTournament::UserId)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(UserInFantasyTournament::Table, UserInFantasyTournament::UserId)
                            .to(User::Table, User::Id),
                    )
                    .col(
                        ColumnDef::new(UserInFantasyTournament::FantasyTournamentId)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                UserInFantasyTournament::Table,
                                UserInFantasyTournament::FantasyTournamentId,
                            )
                            .to(FantasyTournament::Table, FantasyTournament::Id),
                    )
                    .col(
                        ColumnDef::new(UserInFantasyTournament::InvitationStatus)
                            .custom(FantasyTournamentInvitationStatus::Table)
                            .not_null(),
                    )
                    .index(
                        Index::create()
                            .name("user_in_fantasy_tournament_user_tournament")
                            .col(UserInFantasyTournament::UserId)
                            .col(UserInFantasyTournament::FantasyTournamentId)
                            .unique(),
                    )
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_table!(UserInFantasyTournament, manager);
        drop_type!(FantasyTournamentInvitationStatus, manager);
        Ok(())
    }
}
