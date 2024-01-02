use entity::prelude::*;
use entity::*;
use sea_orm::entity::prelude::*;
use sea_orm::DatabaseConnection;

pub async fn get_user_picks_for_tournament(
    db: &DatabaseConnection,
    user_id: i32,
    tournament_id: i32,
) -> Result<Vec<fantasy_pick::Model>, sea_orm::error::DbErr> {
    let picks = FantasyPick::find()
        .filter(fantasy_pick::Column::User.eq(user_id))
        .filter(fantasy_pick::Column::FantasyTournamentId.eq(tournament_id))
        .all(db)
        .await?;

    Ok(picks)
}
