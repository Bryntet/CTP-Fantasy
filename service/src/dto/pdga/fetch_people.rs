use std::hash::{Hash, Hasher};

use rocket_okapi::okapi::schemars;
use rocket_okapi::okapi::schemars::JsonSchema;
use sea_orm::{sea_query, ConnectionTrait, DbErr, EntityTrait, IntoActiveModel};
use serde::Deserialize;

use crate::dto;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompetitionInfoInput {
    pub id: u32,
    pub division: dto::Division,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct ApiPlayer {
    #[serde(rename = "PDGANum")]
    pub pdga_number: i32,
    pub first_name: String,
    pub last_name: String,
    #[serde(rename = "AvatarURL")]
    pub avatar: Option<String>,
    pub division: dto::Division,
}

impl PartialEq for ApiPlayer {
    fn eq(&self, other: &Self) -> bool {
        self.pdga_number == other.pdga_number
    }
}

impl Eq for ApiPlayer {}

impl Hash for ApiPlayer {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pdga_number.hash(state);
    }
}

impl ApiPlayer {
    fn into_active_model(self) -> entity::player::ActiveModel {
        entity::player::Model {
            pdga_number: self.pdga_number,
            first_name: self.first_name,
            last_name: self.last_name,
            avatar: self.avatar.map(|s| "https://www.pdga.com".to_string() + &s),
        }
        .into_active_model()
    }

    fn to_division(&self) -> entity::player_division::ActiveModel {
        entity::player_division::Model {
            player_pdga_number: self.pdga_number,
            division: self.division.clone().into(),
        }
        .into_active_model()
    }
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    data: Data,
}

#[derive(Debug, Deserialize)]
struct Data {
    scores: Vec<ApiPlayer>,
}

pub async fn get_players_from_api(
    tour_id: u32,
    div_name: String,
    round_id: i32,
) -> Result<Vec<ApiPlayer>, reqwest::Error> {
    let get_url = format!("https://www.pdga.com/apps/tournament/live-api/live_results_fetch_round.php?TournID={tour_id}&Division={div_name}&Round={round_id}");
    let response: ApiResponse = reqwest::get(&get_url).await?.json().await?;

    Ok(response.data.scores)
}

pub async fn add_players(db: &impl ConnectionTrait, players: Vec<ApiPlayer>) -> Result<(), DbErr> {
    entity::player::Entity::insert_many(players.into_iter().map(|p| p.into_active_model()))
        .on_conflict(
            sea_query::OnConflict::column(entity::player::Column::PdgaNumber)
                .do_nothing()
                .to_owned(),
        )
        .on_empty_do_nothing()
        .exec(db)
        .await?;
    Ok(())
}
