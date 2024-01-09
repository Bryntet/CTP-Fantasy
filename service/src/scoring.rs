use entity::prelude::*;
use entity::*;
use fantasy_tournament::Entity as FantasyTournament;
use rand::distributions::Alphanumeric;
use rand::Rng;
use rocket::State;
use sea_orm::ActiveValue::*;
use sea_orm::{ActiveModelTrait, DatabaseConnection, DbErr, EntityTrait, IntoActiveModel, TransactionTrait};
use serde::Deserialize;

use rocket_okapi::okapi::schemars;
use rocket_okapi::okapi::schemars::JsonSchema;
use sea_orm::sea_query::BinOper::In;
use entity::sea_orm_active_enums::FantasyTournamentInvitationStatus;
use sea_orm::{QueryFilter, ColumnTrait};

struct SimplePlayer {
    name: String,
    pdga_number: i32,
    score: u32,
}

pub async fn get_all_scores(db: &DatabaseConnection, competition_number: u32) -> Result<Vec<SimplePlayer>, DbErr> {
    let players = Player::find().all(db).await?;
    let mut out = Vec::new();
    for p in &players {
        let scores = FantasyScores::find()
            .filter(fantasy_scores::Column::User.eq(p.id))
            .all(db)
            .await?;
        let mut score = 0;
        for s in &scores {
            score += s.score as u32;
        }
        out.push(SimplePlayer {
            name: p.name.to_string(),
            pdga_number: p.pdga_number,
            score,
        })
    }
    Ok(out)
}