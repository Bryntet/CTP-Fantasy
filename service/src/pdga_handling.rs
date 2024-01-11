use dotenvy::dotenv;
use rocket_okapi::okapi::schemars;
use rocket_okapi::okapi::schemars::JsonSchema;
use sea_orm::{
    ActiveModelTrait, Database, DatabaseConnection, DbErr, EntityTrait, IntoActiveModel,
    TransactionTrait,
};
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompetitionInfoInput {
    pub id: u32,
    pub division: ApiDivision,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct ApiPlayer {
    #[serde(rename = "PDGANum")]
    pub pdga_number: i32,
    pub first_name: String,
    pub last_name: String,
    pub rating: Option<i32>,
    #[serde(rename = "AvatarURL")]
    pub avatar: Option<String>,
    pub division: ApiDivision,
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
            division: self.division.to_division(),
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

#[derive(Debug, Deserialize, Clone, JsonSchema)]
pub enum ApiDivision {
    MPO,
    FPO,
}

impl ApiDivision {
    pub fn to_division(&self) -> entity::sea_orm_active_enums::Division {
        use entity::sea_orm_active_enums::Division::{Fpo, Mpo};
        match self {
            Self::FPO => Fpo,
            Self::MPO => Mpo,
        }
    }
}

pub async fn fetch_people_from_competition(
    tour_id: u32,
    div_name: &str,
    round_id: i32,
) -> Result<(), reqwest::Error> {
    // Define a struct to mirror the JSON structure
    dotenv().ok();
    let db = Database::connect(std::env::var("DATABASE_URL").expect("DATABASE_URL not set"))
        .await
        .unwrap();

    let get_url = format!("https://www.pdga.com/apps/tournament/live-api/live_results_fetch_round.php?TournID={}&Division={}&Round={}", tour_id, div_name, round_id);
    //dbg!(&get_url);

    // Directly deserialize the JSON response into the ApiResponse struct
    let response: ApiResponse = reqwest::get(&get_url).await?.json().await?;
    for player in response.data.scores.clone() {
        //dbg!(res);
        let _ = add_player(&db, player).await;
    }
    Ok(())
}

pub async fn fetch_lots_of_people() {
    for i in 65206..=66206 {
        for div in ["MPO", "FPO"] {
            if let Err(e) = fetch_people_from_competition(i, div, 1).await {
                dbg!(e);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
            dbg!(i, div);
        }
    }
}

async fn add_player(db: &DatabaseConnection, player: ApiPlayer) -> Result<(), DbErr> {
    let txn = db.begin().await?;

    if let Err(e) = { player.to_division().insert(&txn).await } {
        //dbg!(e);
    }
    player.into_active_model().insert(&txn).await?;
    let res = txn.commit().await;
    if let Err(e) = res {
        dbg!(e);
    }
    Ok(())
}









#[test]
fn test_get_pdga_things() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(fetch_people_from_competition(65206, "MPO", 1));
}
