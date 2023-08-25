use reqwest;
use serde_derive::Deserialize;
#[derive(Debug, Deserialize)]
pub struct Round {
    #[serde(rename = "scores")]
    players: Vec<TempPlayer>,
}

#[derive(Debug, Deserialize)]
pub struct StupidRound {
    data: Round,
}

#[derive(Debug, Deserialize)]
struct TempPlayer {
    #[serde(rename = "FirstName")]
    first_name: String,
    #[serde(rename = "LastName")]
    last_name: String,
    #[serde(rename = "PDGANum")]
    pdga_num: Option<i32>,
    #[serde(rename = "Rating")]
    rating: Option<i32>,
    #[serde(rename = "AvatarURL")]
    avatar_url: Option<String>,
}

impl TempPlayer {
    fn make_good(&self) -> crate::models::Player {
        crate::models::Player {
            pdga_number: self.pdga_num.unwrap_or_default(),
            first_name: self.first_name.clone(),
            last_name: Some(self.last_name.clone()),
            rating: self.rating,
            avatar: Some("https://www.pdga.com".to_string() + &self.avatar_url.clone().unwrap_or_default()),
        }
    }
}


pub async fn get_tournament(tour_id: i32, div_name: &str, round_id: i32) -> Result<Vec<crate::models::Player>, reqwest::Error>{
    let res: StupidRound = reqwest::get(format!("https://www.pdga.com/apps/tournament/live-api/live_results_fetch_round.php?TournID={}&Division={}&Round={}", tour_id, div_name, round_id)).await?.json().await?;


    Ok(res.data.players.into_iter().map(|p| p.make_good()).collect())
}