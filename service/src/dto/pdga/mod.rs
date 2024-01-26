mod fetch_people;
mod get_competition;
mod player_scoring;

use sea_orm::DbErr;

pub use fetch_people::get_players_from_api;

pub use get_competition::CompetitionInfo;

use sea_orm::{DatabaseConnection, ModelTrait, TransactionTrait};

async fn update_all_active(db: &DatabaseConnection) -> Result<(), DbErr> {
    let txn = db.begin().await?;

    let active_rounds = super::super::query::active_rounds(&txn).await?;
    active_rounds.iter().for_each(|r| {});

    Ok(())
}

pub use player_scoring::RoundInformation;

pub use fetch_people::add_players;
