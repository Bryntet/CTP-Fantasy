use diesel::IntoSql;
use diesel::row::NamedRow;
use entity::*;
use itertools::Itertools;
use sea_orm::ActiveValue::{NotSet, Set, Unchanged};
use sea_orm::{ActiveModelTrait, Database, DatabaseBackend, DatabaseConnection, DbBackend, DbErr, EntityOrSelect, EntityTrait, InsertResult, IntoActiveModel, QueryTrait, SelectColumns, TransactionTrait};
use serde_derive::Deserialize;
use serde_json::Value;


#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct ApiPlayer {
    #[serde(rename = "PDGANum")]
    pub pdga_number: i32,
    pub first_name: String,
    pub last_name: String,
    pub rating: Option<i32>,
    pub avatar: Option<String>,
    pub division: ApiDivision
}

impl ApiPlayer {
    fn into_active_model(self) -> player::ActiveModel {
        player::Model {
            pdga_number: self.pdga_number,
            first_name: self.first_name,
            last_name: self.last_name,
            rating: self.rating,
            avatar: self.avatar,
        }
        .into_active_model()
    }

    fn to_division(&self) -> player_division::ActiveModel {
        player_division::Model{player_pdga_number: self.pdga_number, division: self.division.to_division()}.into_active_model()
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



#[derive(Debug, Deserialize, Clone)]
enum ApiDivision {
    MPO,
    FPO,
}

impl ApiDivision {
    fn to_division(&self) -> sea_orm_active_enums::Division {
        use sea_orm_active_enums::Division::*;
        match self {
            Self::FPO => Fpo,
            Self::MPO => Mpo
        }

    }
}

async fn get_pdga_things(
    tour_id: i32,
    div_name: &str,
    round_id: i32,
) {
    // Define a struct to mirror the JSON structure
    let db = Database::connect(std::env::var("DATABASE_URL").expect("DATABASE_URL not set"))
        .await
        .unwrap();

    let get_url = format!("https://www.pdga.com/apps/tournament/live-api/live_results_fetch_round.php?TournID={}&Division={}&Round={}", tour_id, div_name, round_id);
    dbg!(&get_url);

    // Directly deserialize the JSON response into the ApiResponse struct
    let response: ApiResponse = reqwest::get(&get_url).await.unwrap().json().await.unwrap();


    for player in response.data.scores.clone() {
        let res = add_player(&db, player).await;
        dbg!(res);
    }

}

async fn add_player(db: &DatabaseConnection, player: ApiPlayer) -> Result<(), DbErr> {
    let txn = db.begin().await?;

    if let Err(e) = {
        player.to_division()
            .insert(&txn)
            .await
    } {
        dbg!(e);
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
    let x = rt.block_on(get_pdga_things(65206, "MPO", 1));
    dbg!(x);
}
