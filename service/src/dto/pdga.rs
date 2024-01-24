use sea_orm::DbErr;
use serde::Deserialize;
use std::collections::HashMap;

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

#[cfg(test)]
mod tests {
    use super::*;
    use rocket::time::Month;

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
    use crate::dto;
    use std::hash::{Hash, Hasher};

    use rocket_okapi::okapi::schemars;
    use rocket_okapi::okapi::schemars::JsonSchema;
    use sea_orm::{sea_query, ConnectionTrait, DbErr, EntityTrait, IntoActiveModel};
    use serde::Deserialize;

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

pub(super) use fetch_people::add_players_from_competition;
