use entity::prelude::*;
use entity::*;
use fantasy_tournament::Entity as FantasyTournament;
use itertools::Itertools;
use sea_orm::{DatabaseConnection, DbErr, EntityTrait, ModelTrait, TransactionTrait};

async fn get_player_round_score_from_fantasy(
    db: &DatabaseConnection,
    fantasy_id: i32,
) -> Result<Vec<player_round_score::Model>, DbErr> {
    let txn = db.begin().await?;

    let player_round_score = FantasyTournament::find_by_id(fantasy_id)
        .one(&txn)
        .await?
        .unwrap()
        .find_related(CompetitionInFantasyTournament)
        .all(&txn)
        .await?
        .iter()
        .map(|x| x.find_related(Competition).one(&txn))
        .collect_vec();

    let mut competitions = Vec::new();
    for x in player_round_score {
        if let Some(x) = x.await? {
            competitions.push(x);
        }
    }

    let mut player_round_score = Vec::new();
    for comp in competitions {
        player_round_score.push(comp.find_related(PlayerRoundScore).all(&txn).await?);
    }

    Ok(player_round_score.iter().flatten().cloned().collect_vec())
}
