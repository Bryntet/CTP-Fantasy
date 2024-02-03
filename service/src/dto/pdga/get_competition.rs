use itertools::Itertools;
use sea_orm::DbErr;
use serde_derive::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
struct CompetitionInfoResponse {
    data: ApiCompetitionInfo,
}

#[derive(Deserialize, Debug)]
struct ApiDivision {
    #[serde(rename = "Division")]
    division: super::super::Division,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct ApiCompetitionInfo {
    rounds_list: HashMap<String, Round>,
    #[serde(rename = "SimpleName")]
    name: String,
    divisions: Vec<ApiDivision>,
    rounds: usize,
    highest_completed_round: Option<usize>,
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
    pub competition_id: u32,
    pub(crate) divisions: Vec<super::super::Division>,
    pub(crate) rounds: usize,
    pub(crate) highest_completed_round: Option<usize>,
}

impl CompetitionInfo {
    pub async fn from_web(competition_id: u32) -> Result<Self, reqwest::Error> {
        let url = format!("https://www.pdga.com/apps/tournament/live-api/live_results_fetch_event.php?TournID={competition_id}");
        let resp: Result<CompetitionInfoResponse, reqwest::Error> =
            reqwest::get(url).await?.json().await;
        match resp {
            Ok(resp) => {
                let dates = parse_date_range(&resp).unwrap();
                let info = resp.data;

                let out = Self {
                    name: info.name,
                    date_range: dates,
                    competition_id,
                    rounds: info.rounds,
                    divisions: info
                        .divisions
                        .into_iter()
                        .dedup_by(|a, b| a.division.eq(&b.division))
                        .map(|d| d.division)
                        .collect(),
                    highest_completed_round: info.highest_completed_round,
                };
                dbg!(&out);
                Ok(out)
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
    use super::super::super::Division;
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
            divisions: vec![Division::FPO, Division::MPO],
            rounds: 0,
        };
        assert_eq!(info, c_info);
    }
}
