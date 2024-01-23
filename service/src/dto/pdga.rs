use sea_orm::ActiveValue::Set;
use sea_orm::{DbErr, EntityTrait, IntoActiveModel, QueryFilter};
use serde::Deserialize;
use std::collections::HashMap;
use sea_orm::ColumnTrait;
use sea_orm::prelude::Date;

#[derive(Deserialize)]
struct CompetitionInfoResponse {
    data: ApiCompetitionInfo,
}

#[derive(Deserialize)]
struct ApiCompetitionInfo {
    #[serde(rename = "RoundsList")]
    rounds_list: HashMap<String, Round>,
    #[serde(rename = "SimpleName")]
    name: String,
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
}



fn parse_date_range(res: &CompetitionInfoResponse) -> Result<Vec<Date>, DbErr> {
    let mut dates = Vec::new();
    for round in res.data.rounds_list.values() {
        dates.push(round.date);
    }
    Ok(dates)
}

async fn get_competition_information(
    competition_id: u32,
) -> Result<CompetitionInfo, reqwest::Error> {
    let url = format!("https://www.pdga.com/apps/tournament/live-api/live_results_fetch_event.php?TournID={competition_id}");

    let resp: CompetitionInfoResponse = reqwest::get(url).await?.json().await?;
    let dates = parse_date_range(&resp).unwrap();
    let info = resp.data;
    Ok(CompetitionInfo {
        name: info.name,
        date_range: dates,
        competition_id,
    })
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
        let info = get_competition_information(77583).await.unwrap();
        dbg!(&info);
        let c_info = CompetitionInfo {
            name: "Winter Warriors at Järva DGP 1-27".to_string(),
            date_range: vec![Date::from_ymd_opt(2024, 1, 28).unwrap()],
            competition_id: 77583,
        };
        assert_eq!(info, c_info);
    }
}
