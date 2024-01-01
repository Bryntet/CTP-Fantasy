pub use sea_orm_migration::prelude::*;
mod enums;
mod m20240101_000001_create_users_and_players;
mod m20240101_164644_create_tournaments;
mod m20240101_231414_fantasy_tournament_picks_and_user_score;
mod macros;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240101_000001_create_users_and_players::Migration),
            Box::new(m20240101_164644_create_tournaments::Migration),
            Box::new(m20240101_231414_fantasy_tournament_picks_and_user_score::Migration),
        ]
    }
}
