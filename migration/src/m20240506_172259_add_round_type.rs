use crate::enums::{FantasyTournamentInvitationStatus, RoundTypeEnum, RoundTypeVariants};
use sea_orm_migration::prelude::extension::postgres::Type;
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
                    .as_enum(RoundTypeEnum)
                    .values(RoundTypeVariants::iter())
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
            ALTER TABLE round
            ADD COLUMN round_type round_type_enum;
            "#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
            ALTER TABLE round
            DROP COLUMN round_type;
            "#,
            )
            .await?;
        manager
            .drop_type(Type::drop().name(RoundTypeEnum).to_owned())
            .await?;
        Ok(())
    }
}
