use crate::dto::{Division, RoundInformation};
use crate::error::GenericError;
use itertools::Itertools;

use chrono::{DateTime, NaiveDate, NaiveTime, TimeZone, Utc};
use rocket::error;
use sea_orm::DbErr;
use serde_derive::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub(super) struct CompetitionInfoResponse {
    pub(self) data: ApiCompetitionInfo,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct ApiCompetitionInfo {
    #[serde(rename = "SimpleName")]
    name: String,
    divisions: Vec<DivisionWrapper>,
    rounds: u8,
    highest_completed_round: Option<u8>,
    start_date: String,
    end_date: String,
    location: String,
    country: String,
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
use cached::proc_macro::cached;

impl DateRange {
    pub async fn new(start: &str, end: &str, location: String, country: String) -> Option<Self> {
        let tz = parse_tz_from_location(location, country)
            .await
            .unwrap_or(chrono_tz::Tz::US__Hawaii);

        let start =
            dateparser::parse_with(start, &tz, NaiveTime::from_hms_opt(7, 0, 0).unwrap()).ok()?;
        let end =
            dateparser::parse_with(end, &tz, NaiveTime::from_hms_opt(22, 0, 0).unwrap()).ok()?;

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

    pub fn len(&self) -> i64 {
        self.end.signed_duration_since(self.start).num_days()
    }

    pub fn date_times(&self) -> Vec<DateTime<chrono_tz::Tz>> {
        (0..self.len()).filter_map(|x| self.date_time(x)).collect()
    }
    fn date_time(&self, day: i64) -> Option<DateTime<chrono_tz::Tz>> {
        if self.len() >= day {
            Some(self.day(day)?)
        } else {
            None
        }
    }

    fn day(&self, day: i64) -> Option<DateTime<chrono_tz::Tz>> {
        self.start
            .checked_add_days(chrono::Days::new((day - 1) as u64))
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
}

impl CompetitionInfo {
    pub async fn from_web(competition_id: u32) -> Result<Self, GenericError> {
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

        let info = resp.data;
        let mut rounds = Vec::new();
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

        for round_number in 1..=info.rounds {
            rounds.push(
                RoundInformation::new(competition_id as usize, round_number as usize, divs.clone())
                    .await?,
            )
        }
        let begin = std::time::Instant::now();
        let duration = begin.elapsed();
        dbg!(duration);

        let out = Self {
            name: info.name,
            competition_id,
            rounds,
            highest_completed_round: info.highest_completed_round,
            divisions: divs,
            date_range,
        };
        Ok(out)
    }
}

mod blocking {
    use geocoding::{Forward, Openstreetmap, Point};
    use rtzlib::{CanPerformGeoLookup, NedTimezone, OsmTimezone};
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
    pub(super) fn parse_tz_from_location(
        location: String,
        country: String,
    ) -> Option<chrono_tz::Tz> {
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
    use chrono::NaiveTime;

    #[tokio::test]
    async fn test_parse_date() {
        let url = "https://www.pdga.com/apps/tournament/live-api/live_results_fetch_event.php?TournID=73836";

        let resp: CompetitionInfoResponse = reqwest::get(url).await.unwrap().json().await.unwrap();
        dbg!(&resp.data.start_date, &resp.data.end_date);
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
        let info = CompetitionInfo::from_web(77583).await.unwrap();
    }
}
