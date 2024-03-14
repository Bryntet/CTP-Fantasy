use cached::proc_macro::cached;
use chrono::{DateTime, NaiveDate, NaiveTime, TimeZone};
use itertools::Itertools;
use rocket::form::validate::Contains;
use rocket::{error, warn};
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;

use crate::dto::{Division, RoundInformation};
use crate::error::GenericError;

#[derive(Deserialize, Debug)]
pub(super) struct CompetitionInfoResponse {
    pub(self) data: ApiCompetitionInfo,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct ApiRoundLabelInfo {
    #[serde(rename = "Number")]
    round_number: usize,
    label: String,
}

impl From<&ApiRoundLabelInfo> for RoundLabel {
    fn from(info: &ApiRoundLabelInfo) -> Self {
        let label = info.label.to_lowercase();
        if label.contains("round") {
            RoundLabel::Round(info.round_number)
        } else {
            match label.as_str() {
                "finals" | "final" => RoundLabel::Final,
                "playoff" | "playoffs" => RoundLabel::Playoff,
                _ => {
                    warn!("Unknown round label: {}", label);
                    RoundLabel::Other
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum RoundLabel {
    Final,
    Playoff,
    Round(usize),
    Other,
}
/// This struct is used to store the round number and the label of the round
/// round_number is a 1-indexed number of the round, the number that label contains is arbitrary based on PDGA's API
#[derive(Debug, PartialEq, Clone)]
pub struct RoundLabelInfo {
    pub round_number: usize,
    pub label: RoundLabel,
}

impl From<&ApiRoundLabelInfo> for RoundLabelInfo {
    fn from(info: &ApiRoundLabelInfo) -> Self {
        Self {
            round_number: info.round_number,
            label: info.into(),
        }
    }
}

impl RoundLabelInfo {
    pub fn get_round_number_from_label(&self, total_rounds: usize) -> usize {
        match self.label {
            RoundLabel::Final => total_rounds,
            RoundLabel::Playoff => total_rounds - 1,
            RoundLabel::Round(round) => round,
            RoundLabel::Other => {
                warn!("Unknown round label: {:?}", self);
                self.round_number
            }
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct ApiCompetitionInfo {
    #[serde(rename = "SimpleName")]
    name: String,
    divisions: Vec<DivisionWrapper>,
    #[serde(rename = "RoundsList", deserialize_with = "flatten_round_labels")]
    round_labels: Vec<ApiRoundLabelInfo>,
    highest_completed_round: Option<u8>,
    start_date: String,
    end_date: String,
    location: String,
    country: String,
}

fn flatten_round_labels<'de, D>(deserializer: D) -> Result<Vec<ApiRoundLabelInfo>, D::Error>
where
    D: Deserializer<'de>,
{
    let map: HashMap<String, ApiRoundLabelInfo> = Deserialize::deserialize(deserializer)?;
    Ok(map.into_values().collect())
}

#[derive(Deserialize, Debug)]
struct DivisionWrapper {
    #[serde(rename = "Division")]
    division: Division,
}

#[derive(Debug, PartialEq, Clone)]
pub struct DateRange {
    start: DateTime<chrono_tz::Tz>,
    end: DateTime<chrono_tz::Tz>,
}

impl DateRange {
    pub async fn new(start: &str, end: &str, location: String, country: String) -> Option<Self> {
        let tz = parse_tz_from_location(location, country)
            .await
            .unwrap_or(chrono_tz::Tz::US__Hawaii);

        let start = dateparser::parse_with(start, &tz, NaiveTime::from_hms_opt(7, 0, 0).unwrap()).ok()?;
        let end = dateparser::parse_with(end, &tz, NaiveTime::from_hms_opt(22, 0, 0).unwrap()).ok()?;

        let start = tz.from_utc_datetime(&start.naive_utc());
        let end = tz.from_utc_datetime(&end.naive_utc());
        Some(Self { start, end })
    }
    async fn from_api_comp_info(info: &ApiCompetitionInfo) -> Option<Self> {
        Self::new(
            &info.start_date,
            &info.end_date,
            info.location.clone(),
            info.country.clone(),
        )
        .await
    }

    pub fn timezone(&self) -> chrono_tz::Tz {
        self.start.timezone()
    }

    pub fn len(&self) -> i64 {
        self.end.signed_duration_since(self.start).num_days()
    }

    pub fn date_times(&self) -> Vec<DateTime<chrono_tz::Tz>> {
        (0..=self.len())
            .filter_map(|x| self.date_time(x as u64))
            .collect()
    }

    fn date_time(&self, day: u64) -> Option<DateTime<chrono_tz::Tz>> {
        if self.len() >= day as i64 {
            Some(self.get_day(day)?)
        } else {
            None
        }
    }

    pub fn get_day(&self, day: u64) -> Option<DateTime<chrono_tz::Tz>> {
        self.start.checked_add_days(chrono::Days::new(day))
    }

    pub(crate) fn start_date(&self) -> NaiveDate {
        self.start.naive_local().date()
    }
}

#[derive(Debug, PartialEq)]
pub struct CompetitionInfo {
    pub(crate) name: String,
    pub competition_id: u32,
    pub(crate) divisions: Vec<Division>,
    pub(crate) rounds: Vec<RoundInformation>,
    pub(crate) highest_completed_round: Option<u8>,
    pub(crate) date_range: DateRange,
    pub(crate) amount_of_rounds: usize,
}

impl CompetitionInfo {
    pub async fn from_web(competition_id: u32) -> Result<Self, GenericError> {
        let mut info = Self::get_pdga_competition_info(competition_id).await?;
        info.round_labels
            .sort_by(|a, b| a.round_number.cmp(&b.round_number));
        let date_range = DateRange::from_api_comp_info(&info).await.unwrap();

        let divs = info
            .divisions
            .into_iter()
            .filter_map(|d| {
                let div = d.division;
                if div == Division::Unknown {
                    None
                } else {
                    Some(div)
                }
            })
            .dedup()
            .collect_vec();
        let mut rounds = Vec::new();
        let amount_of_rounds = info.round_labels.len();
        for round_label in &info.round_labels {
            let label = RoundLabelInfo::from(round_label);
            if let Ok(round) =
                RoundInformation::new(competition_id as usize, divs.clone(), &label, amount_of_rounds).await
            {
                rounds.push(round);
            } else {
                rounds.push(RoundInformation::phantom(
                    label,
                    competition_id as usize,
                    amount_of_rounds,
                ));
            }
        }
        rounds.sort_by(|a, b| {
            a.label
                .get_round_number_from_label(amount_of_rounds)
                .cmp(&b.label.get_round_number_from_label(amount_of_rounds))
        });

        let out = Self {
            name: info.name,
            competition_id,
            amount_of_rounds: rounds.len(),
            rounds,
            highest_completed_round: info.highest_completed_round,
            divisions: divs,
            date_range,
        };
        Ok(out)
    }

    async fn get_pdga_competition_info(competition_id: u32) -> Result<ApiCompetitionInfo, GenericError> {
        let url = format!("https://www.pdga.com/apps/tournament/live-api/live_results_fetch_event.php?TournID={competition_id}");
        let resp: CompetitionInfoResponse = reqwest::get(url)
            .await
            .map_err(|e| {
                error!("Unable to fetch competition from PDGA: {}", e);
                GenericError::PdgaGaveUp("Internal error while fetching competition from PDGA")
            })?
            .json()
            .await
            .map_err(|e| {
                error!("PDGA issue while converting to json: {:#?}", e);
                GenericError::PdgaGaveUp(
                    "Internal error while converting PDGA competition to internal format",
                )
            })?;
        Ok(resp.data)
    }
}

mod blocking {
    use geocoding::{Forward, Openstreetmap, Point};
    use rtzlib::{CanPerformGeoLookup, NedTimezone};

    fn get_point(loc: &str) -> Option<Point<f32>> {
        if let Ok(v) = Openstreetmap::new().forward(loc) {
            if !v.is_empty() {
                Some(v[0])
            } else {
                None
            }
        } else {
            None
        }
    }

    #[inline(always)]
    pub(super) fn parse_tz_from_location(location: String, country: String) -> Option<chrono_tz::Tz> {
        let address = location.clone() + ", " + &country;
        let point = get_point(&address);
        let point = match point {
            None => get_point(&location),
            Some(p) => Some(p),
        };
        if let Some(point) = point {
            let tz = NedTimezone::lookup(point.0.x, point.0.y);
            if !tz.is_empty() {
                Some(tz[0].identifier.as_ref()?.parse().ok()?)
            } else {
                None
            }
        } else {
            None
        }
    }
}
#[cached]
async fn parse_tz_from_location(location: String, country: String) -> Option<chrono_tz::Tz> {
    tokio::task::spawn_blocking(move || blocking::parse_tz_from_location(location, country))
        .await
        .ok()?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_date() {
        let url = "https://www.pdga.com/apps/tournament/live-api/live_results_fetch_event.php?TournID=73836";

        let resp: CompetitionInfoResponse = reqwest::get(url).await.unwrap().json().await.unwrap();
        if let Some(range) = DateRange::new(
            &resp.data.start_date,
            &resp.data.end_date,
            resp.data.location,
            resp.data.country,
        )
        .await
        {
            dbg!(range);
        }
    }

    #[tokio::test]
    async fn test_competition_info() {
        let info = CompetitionInfo::from_web(77583).await;
        assert!(info.is_ok())
    }
}
