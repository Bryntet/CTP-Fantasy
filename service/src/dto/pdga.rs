use std::collections::HashMap;

use sea_orm::DbErr;
use serde::Deserialize;

pub(super) use fetch_people::add_players_from_competition;

#[derive(Deserialize, Debug)]
struct CompetitionInfoResponse {
    data: ApiCompetitionInfo,
}

#[derive(Deserialize, Debug)]
struct ApiDivision {
    #[serde(rename = "Division")]
    division: super::Division,
}

#[derive(Deserialize, Debug)]
struct ApiCompetitionInfo {
    #[serde(rename = "RoundsList")]
    rounds_list: HashMap<String, Round>,
    #[serde(rename = "SimpleName")]
    name: String,
    #[serde(rename = "Divisions")]
    divisions: Vec<ApiDivision>,
    #[serde(rename = "Rounds")]
    rounds: usize,
}

#[derive(Deserialize, Debug)]
struct Round {
    #[serde(rename = "Date")]
    date: sea_orm::prelude::Date,
}

#[derive(Debug, PartialEq)]
pub struct CompetitionInfo {
    pub(crate) name: String,
    pub(crate) date_range: Vec<sea_orm::prelude::Date>,
    pub(crate) competition_id: u32,
    pub(crate) divisions: Vec<super::Division>,
    pub(crate) rounds: usize,
}

impl CompetitionInfo {
    pub async fn from_web(competition_id: u32) -> Result<Self, reqwest::Error> {
        let url = format!("https://www.pdga.com/apps/tournament/live-api/live_results_fetch_event.php?TournID={competition_id}");
        let resp: Result<CompetitionInfoResponse, reqwest::Error> =
            reqwest::get(url).await?.json().await;
        match resp {
            Ok(resp) => {
                dbg!(&resp);
                let dates = parse_date_range(&resp).unwrap();
                let info = resp.data;

                Ok(Self {
                    name: info.name,
                    date_range: dates,
                    competition_id,
                    rounds: info.rounds,
                    divisions: info.divisions.into_iter().map(|d| d.division).collect(),
                })
            }
            Err(e) => {
                dbg!(&e);
                Err(e)
            }
        }
    }
}

fn parse_date_range(res: &CompetitionInfoResponse) -> Result<Vec<sea_orm::prelude::Date>, DbErr> {
    let mut dates = Vec::new();
    for round in res.data.rounds_list.values() {
        dates.push(round.date);
    }
    Ok(dates)
}



use entity::prelude::*;
use entity::*;
use fantasy_tournament::Entity as FantasyTournament;
use itertools::Itertools;
use sea_orm::{DatabaseConnection, EntityTrait, ModelTrait, TransactionTrait};


async fn update_all_active(
    db: &DatabaseConnection,
) -> Result<(), DbErr> {
    let txn = db.begin().await?;

    let active_rounds = super::super::query::active_rounds(&txn).await?;
    active_rounds.iter().for_each(|r| {

    });

    Ok(())
}

pub use player_scoring::get_round_information;

mod player_scoring {
    use itertools::Itertools;
    use serde_derive::Deserialize;
    use crate::dto::Division;

    #[derive(Deserialize)]
    enum Unit {
        Meters,
        Feet,
    }
    #[derive(Deserialize)]
    struct ApiRes {
        data: RoundFromApi
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Layout {
        #[serde(rename = "Detail")]
        holes: Vec<Hole>,
        length: u32,
        #[serde(rename = "Units")]
        unit: Unit
    }

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "PascalCase")]
    pub(super) struct Hole {
        pub par: u32,
        #[serde(rename = "HoleOrdinal")]
        pub hole_number: u32,
        pub length: u32,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    pub struct PlayerInRound {
        #[serde(rename = "PDGANum")]
        pub pdga_number: i32,
        pub name: String,
        pub hole_scores: Vec<u32>,
        pub round_score: u32,
        #[serde(rename = "RoundtoPar")]
        pub round_to_par:i32
    }
    impl From<PlayerInRound> for PlayerScore {
        fn from(p: PlayerInRound) -> Self {
            Self {
                pdga_number: p.pdga_number,
                hole_scores: p.hole_scores,
                round_score: p.round_score,
                round_to_par: p.round_to_par
            }
        }
    }

    #[derive(Debug)]
    pub struct PlayerScore {
        pub pdga_number: i32,
        pub hole_scores: Vec<u32>,
        pub round_score: u32,
        pub round_to_par:i32
    }


    #[derive(Deserialize)]
    struct RoundFromApi {
        layouts: Vec<Layout>,
        scores: Vec<PlayerInRound>,
    }

    impl From<RoundFromApi> for RoundInformation {
        fn from(round_from_api: RoundFromApi) -> Self {

            let layout = round_from_api.layouts.first().unwrap();
            let holes = layout.holes.iter().map(|h| Hole {
                par: h.par,
                hole_number: h.hole_number,
                length: match layout.unit {
                    Unit::Feet => {
                        (h.length as f64 * 0.3048).round() as u32
                    }
                    Unit::Meters => {
                        (h.length as f64).round() as u32
                    }
                },
            }).collect_vec();

            let length = match layout.unit {
                Unit::Feet => {
                    (layout.length as f64 * 0.3048).round() as u32
                }
                Unit::Meters => {
                    (layout.length as f64).round() as u32
                }
            };

            RoundInformation {
                holes,
                players:round_from_api.scores.into_iter().map(|p|p.into() ).collect(),
                course_length: length
            }
        }
    }


    #[derive(Debug)]
    pub struct RoundInformation {
        pub holes: Vec<Hole>,
        pub players: Vec<PlayerScore>,
        pub course_length: u32
    }
    pub async fn get_round_information(competition_id:u32, round:u32, div:Division) -> Result<RoundInformation, reqwest::Error> {

        let div_str = div.to_string();
        let url = format!("https://www.pdga.com/apps/tournament/live-api/live_results_fetch_round.php?TournID={competition_id}&Round={round}&Division={div_str}");
        dbg!(&url);
        let resp: ApiRes = reqwest::get(url).await?.json().await?;
        Ok(resp.data.into())
    }
}



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
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_date_range() {
        let url = "https://www.pdga.com/apps/tournament/live-api/live_results_fetch_event.php?TournID=77583";

        let resp: CompetitionInfoResponse = reqwest::get(url).await.unwrap().json().await.unwrap();
        let mut dates = parse_date_range(&resp).unwrap();
        dates.sort();
        assert_eq!(dates.len(), 1);
    }

    #[tokio::test]
    async fn test_get_competition_information() {
        let info = CompetitionInfo::from_web(77583).await.unwrap();
        dbg!(&info);
        let c_info = CompetitionInfo {
            name: "Winter Warriors at Järva DGP 1-27".to_string(),
            date_range: vec![sea_orm::prelude::Date::from_ymd_opt(2024, 1, 28).unwrap()],
            competition_id: 77583,
            divisions: vec![super::super::Division::FPO, super::super::Division::MPO],
            rounds: 0,
        };
        assert_eq!(info, c_info);
    }
}

pub(crate) mod fetch_people {
    use std::hash::{Hash, Hasher};

    use rocket_okapi::okapi::schemars;
    use rocket_okapi::okapi::schemars::JsonSchema;
    use sea_orm::{ConnectionTrait, DbErr, EntityTrait, IntoActiveModel, sea_query};
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

    pub async fn add_players_from_competition(
        tour_id: u32,
        div_name: String,
        round_id: i32,
    ) -> Result<Vec<ApiPlayer>, reqwest::Error> {
        let get_url = format!("https://www.pdga.com/apps/tournament/live-api/live_results_fetch_round.php?TournID={tour_id}&Division={div_name}&Round={round_id}");
        let response: ApiResponse = reqwest::get(&get_url).await?.json().await?;

        Ok(response.data.scores)
    }

    pub async fn add_players<C>(db: &C, players: Vec<ApiPlayer>) -> Result<(), DbErr>
        where
            C: ConnectionTrait,
    {
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
}

