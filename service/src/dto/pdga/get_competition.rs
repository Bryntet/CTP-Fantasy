use itertools::Itertools;
use sea_orm::DbErr;
use serde_derive::Deserialize;
use std::collections::HashMap;
use crate::dto::{Division, RoundInformation};
use crate::error::GenericError;
use log::{warn,debug};
use rocket::error;

#[derive(Deserialize, Debug)]
pub(super) struct CompetitionInfoResponse {
    pub(self) data: ApiCompetitionInfo,
}


#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct ApiCompetitionInfo {
    rounds_list: HashMap<String, Round>,
    #[serde(rename = "SimpleName")]
    name: String,
    divisions: Vec<DivisionWrapper>,
    rounds: u8,
    highest_completed_round: Option<u8>,
}

#[derive(Deserialize, Debug)]
struct DivisionWrapper {
    #[serde(rename = "Division")]
    division: Division,
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
    pub(crate) divisions: Vec<Division>,
    pub(crate) rounds: Vec<RoundInformation>,
    pub(crate) highest_completed_round: Option<u8>,
}

impl CompetitionInfo {
    pub async fn from_web(competition_id: u32) -> Result<Self, GenericError> {
        let url = format!("https://www.pdga.com/apps/tournament/live-api/live_results_fetch_event.php?TournID={competition_id}");
        let resp: CompetitionInfoResponse =
            reqwest::get(url).await.map_err(|e|{
                error!("Unable to fetch competition from PDGA: {}", e);
                GenericError::PdgaGaveUp("Internal error while fetching competition from PDGA")
            })?.json().await.map_err(|e|{
                error!("PDGA issue while converting to json:{:#?}", e);
                GenericError::PdgaGaveUp("Internal error while converting PDGA competition to internal format")
            })?;

        let dates = parse_date_range(&resp).unwrap();
        let info = resp.data;
        let mut rounds = Vec::new();

        let divs = info.divisions.into_iter().filter_map(|d| {
            let div= d.division;
            if div == Division::Unknown {
                None
            } else {
                Some(div)
            }
        }).dedup().collect_vec();

        for round_number in 1..=info.rounds {
            rounds.push(RoundInformation::new(competition_id as usize, round_number as usize, divs.clone()).await?)
        }


        let out = Self {
            name: info.name,
            date_range: dates,
            competition_id,
            rounds,
            highest_completed_round: info.highest_completed_round,
            divisions: divs
        };
        Ok(out)



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

    #[tokio::test]
    async fn test_parse_date_range() {
        let url = "https://www.pdga.com/apps/tournament/live-api/live_results_fetch_event.php?TournID=77583";

        let resp: CompetitionInfoResponse = reqwest::get(url).await.unwrap().json().await.unwrap();
        let mut dates = parse_date_range(&resp).unwrap();
        dates.sort();
        assert_eq!(dates.len(), 1);
    }

    #[tokio::test]
    async fn test_competition_info() {
        let info = CompetitionInfo::from_web(77583).await.unwrap();
    }
}
