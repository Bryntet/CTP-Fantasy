use std::error::Error;
use std::hash::{Hash, Hasher};

use rocket_okapi::okapi::schemars;
use rocket_okapi::okapi::schemars::JsonSchema;

use sea_orm::{sea_query, ConnectionTrait, DbErr, EntityTrait, IntoActiveModel};
use serde::de::Unexpected;
use serde::{de, Deserialize, Deserializer};

use crate::dto::pdga::player_scoring::PlayerScore;
use crate::dto::Division;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompetitionInfoInput {
    pub id: u32,
    pub division: Division,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct ApiPlayer {
    #[serde(rename = "PDGANum")]
    pub pdga_number: i32,
    pub first_name: String,
    pub last_name: String,
    pub name: String,
    #[serde(rename = "AvatarURL")]
    pub avatar: Option<String>,
    pub division: Division,
    #[serde(rename = "RoundtoPar")]
    pub round_to_par: i32,
    #[serde(deserialize_with = "bool_from_int")]
    pub round_started: bool,
    pub running_place: u8,
    #[serde(rename = "RoundScore", deserialize_with = "flexible_number")]
    pub throws: i64,
    pub hole_scores: Vec<String>,
}

mod serde_things {
    use std::fmt;
    use std::hash::Hasher;
    use serde::{de, Deserialize, Deserializer};
    use serde::de::{Unexpected, Visitor};

    pub(super) fn flexible_number<'de, D>(deserializer: D) -> Result<i64, D::Error>
        where
            D: Deserializer<'de>,
    {
        struct FlexibleNumberVisitor;

        impl<'de> Visitor<'de> for FlexibleNumberVisitor {
            type Value = i64;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a number or a string containing a number")
            }

            fn visit_i64<E>(self, value: i64) -> Result<i64, E>
                where
                    E: de::Error,
            {
                Ok(value)
            }

            fn visit_u64<E>(self, value: u64) -> Result<i64, E>
                where
                    E: de::Error,
            {
                Ok(value as i64)
            }

            fn visit_str<E>(self, value: &str) -> Result<i64, E>
                where
                    E: de::Error,
            {
                value.parse::<i64>().map_err(E::custom)
            }
        }

        deserializer.deserialize_any(FlexibleNumberVisitor)
    }


    pub(super) fn bool_from_int<'de, D>(deserializer: D) -> Result<bool, D::Error>
        where
            D: Deserializer<'de>,
    {
        match u8::deserialize(deserializer)? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(de::Error::invalid_value(
                Unexpected::Unsigned(other as u64),
                &"zero or one",
            )),
        }
    }
}

use serde_things::{bool_from_int, flexible_number};
impl From<ApiPlayer> for PlayerScore {
    fn from(p: ApiPlayer) -> Self {
        Self {
            pdga_number: p.pdga_number,
            throws: p.throws as u8,
            round_to_par: p.round_to_par,
            hole_scores: p
                .hole_scores
                .iter()
                .filter_map(|s| s.parse::<u32>().ok())
                .collect(),
            placement: p.running_place,
        }
    }
}
impl PartialEq for ApiPlayer {
    fn eq(&self, other: &Self) -> bool {
        self.pdga_number == other.pdga_number
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

    fn to_division(
        &self,
        fantasy_tournament_id: i32,
    ) -> entity::player_division_in_fantasy_tournament::ActiveModel {
        entity::player_division_in_fantasy_tournament::Model {
            player_pdga_number: self.pdga_number,
            fantasy_tournament_id,
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
    div: &Division,
    round_id: i32,
) -> Result<Vec<ApiPlayer>, reqwest::Error> {
    let div_name = div.to_string().to_uppercase();
    let get_url = format!("https://www.pdga.com/apps/tournament/live-api/live_results_fetch_round.php?TournID={tour_id}&Division={div_name}&Round={round_id}");
    // dbg!(&get_url,reqwest::get(&get_url).await.map_err(|e|dbg!(e)));
    let response: ApiResponse = reqwest::get(&get_url).await?.json().await.map_err(|e| {
        println!("{:#?}", &e.source());
        e
    })?;
    Ok(response.data.scores)
}

pub async fn add_players(
    db: &impl ConnectionTrait,
    players: Vec<ApiPlayer>,
    fantasy_tournament_id: Option<i32>,
) -> Result<(), DbErr> {
    entity::player::Entity::insert_many(players.clone().into_iter().map(|p| p.into_active_model()))
        .on_conflict(
            sea_query::OnConflict::column(entity::player::Column::PdgaNumber)
                .do_nothing()
                .to_owned(),
        )
        .do_nothing()
        .exec(db)
        .await
        .map_err(|e| {
            dbg!(&e);
            e
        })?;
    if let Some(fantasy_tournament_id) = fantasy_tournament_id {
        entity::player_division_in_fantasy_tournament::Entity::insert_many(
            players
                .into_iter()
                .map(|p| p.to_division(fantasy_tournament_id)),
        ).on_conflict(
            sea_query::OnConflict::column(entity::player_division_in_fantasy_tournament::Column::PlayerPdgaNumber)
                .do_nothing()
                .to_owned(),)
        .do_nothing()
        .exec(db)
        .await
        .map_err(|e| {
            dbg!(&e);
            e
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::fs::File;
    use std::io::Read;
    #[test]
    fn test_serde_into_api_response() {
        let file_path = "/home/brynte/RustroverProjects/CTP-Fantasy/serde-test/fpo_round.json";
        let mut file = File::open(file_path).unwrap();
        let mut data = String::new();
        file.read_to_string(&mut data).unwrap();
        let a: ApiResponse = serde_json::from_str(&data).unwrap();
    }
}
