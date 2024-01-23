use sea_orm::prelude::Date;
use sea_orm::ActiveValue::Set;
use sea_orm::ColumnTrait;
use sea_orm::{DbErr, EntityTrait, IntoActiveModel, QueryFilter};
use serde::Deserialize;
use std::collections::HashMap;



#[derive(Deserialize)]
struct CompetitionInfoResponse {
    data: ApiCompetitionInfo,
}
#[derive(Deserialize)]
struct ApiDivision {
    #[serde(rename="Division")]
    division: super::Division
}
#[derive(Deserialize)]
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



#[derive(Deserialize)]
struct Round {
    #[serde(rename = "Date")]
    date: Date,
}

#[derive(Debug, PartialEq)]
pub struct CompetitionInfo {
    pub(crate) name: String,
    pub(crate) date_range: Vec<Date>,
    pub(crate) competition_id: u32,
    pub(crate) divisions: Vec<super::Division>,
    pub(crate) rounds: usize
}

impl CompetitionInfo {
    pub async fn from_web(competition_id: u32) -> Result<Self, reqwest::Error> {
        let url = format!("https://www.pdga.com/apps/tournament/live-api/live_results_fetch_event.php?TournID={competition_id}");

        let resp: CompetitionInfoResponse = reqwest::get(url).await?.json().await?;
        let dates = parse_date_range(&resp).unwrap();
        let info = resp.data;


        Ok(Self {
            name: info.name,
            date_range: dates,
            competition_id,
            rounds: info.rounds,
            divisions: info.divisions.into_iter().map(|d|d.division).collect(),

        })
    }
}

fn parse_date_range(res: &CompetitionInfoResponse) -> Result<Vec<Date>, DbErr> {
    let mut dates = Vec::new();
    for round in res.data.rounds_list.values() {
        dates.push(round.date);
    }
    Ok(dates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::prelude::Date;

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
            date_range: vec![Date::from_ymd_opt(2024, 1, 28).unwrap()],
            competition_id: 77583,
        };
        assert_eq!(info, c_info);
    }
}


pub(crate) mod fetch_people {
    use std::hash::{Hash, Hasher};
    use crate::dto;
    use dotenvy::dotenv;
    use rocket_okapi::okapi::schemars;
    use rocket_okapi::okapi::schemars::JsonSchema;
    use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, DbErr, EntityTrait, IntoActiveModel, TransactionTrait};
    use serde::Deserialize;
    use std::time::Duration;

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
        // Define a struct to mirror the JSON structure
        dotenv().ok();


        let get_url = format!("https://www.pdga.com/apps/tournament/live-api/live_results_fetch_round.php?TournID={}&Division={}&Round={}", tour_id, div_name, round_id);
        //dbg!(&get_url);

        // Directly deserialize the JSON response into the ApiResponse struct
        let response: ApiResponse = reqwest::get(&get_url).await?.json().await?;

        Ok( response.data.scores )
    }
    
    pub async fn add_players<C>(db: &C, players: Vec<ApiPlayer>) -> Result<(), DbErr> where C: ConnectionTrait {

        entity::player::Entity::insert_many(players.into_iter().map(|p|p.into_active_model())).exec(db).await?;
        Ok(())
    }

}

pub(super) use fetch_people::{add_players_from_competition,add_players};