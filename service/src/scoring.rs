use itertools::Itertools;
use entity::prelude::*;
use entity::*;
use fantasy_tournament::Entity as FantasyTournament;
use rand::distributions::Alphanumeric;
use rand::Rng;
use rocket::futures::FutureExt;
use rocket::State;
use sea_orm::ActiveValue::*;
use sea_orm::{ActiveModelTrait, DatabaseConnection, DbErr, EntityTrait, IntoActiveModel, ModelTrait, TransactionTrait};
use serde::Deserialize;

use rocket_okapi::okapi::schemars;
use rocket_okapi::okapi::schemars::JsonSchema;
use sea_orm::sea_query::BinOper::In;
use entity::sea_orm_active_enums::FantasyTournamentInvitationStatus;
use sea_orm::{QueryFilter, ColumnTrait};








async fn get_player_round_score_from_fantasy(db: &DatabaseConnection, fantasy_id: i32) -> Result<Vec<player_round_score::Model>, DbErr> {
    let txn = db.begin().await?;

    let player_round_score = FantasyTournament::find_by_id(fantasy_id).one(&txn).await?.unwrap().find_related(CompetitionInFantasyTournament).all(&txn).await?.iter().map(|x| x.find_related(Competition).one(&txn)).collect_vec();

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





