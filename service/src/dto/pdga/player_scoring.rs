use crate::dto::{CompetitionLevel, Division, UserScore};
use entity::player_round_score::ActiveModel;
use entity::{fantasy_pick, player_round_score, user};

use crate::dto::pdga::ApiPlayer;
use crate::error::GenericError;
use entity::prelude::{FantasyPick, User};
use itertools::Itertools;
use log::warn;
use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::ActiveValue::Set;
use sea_orm::ModelTrait;
use sea_orm::{ColumnTrait, QueryFilter};
use sea_orm::{ConnectionTrait, DbErr, EntityTrait, IntoActiveModel, NotSet};

use serde::Deserialize;
use serde_with::serde_as;

#[derive(Deserialize, PartialEq, Debug, Clone, Default, Copy)]
enum Unit {
    #[default]
    Meters,
    Feet,
}
#[derive(Deserialize, Debug)]
struct ApiRes {
    data: RoundFromApi,
}

#[derive(Deserialize, PartialEq, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct Layout {
    #[serde(rename = "Detail")]
    holes: Vec<Hole>,
    length: Option<u32>,
    #[serde(rename = "Units")]
    unit: Option<Unit>,
}
/*
impl Layout {
    pub fn phantom() -> Self {
        Layout {
            holes: vec![],
            length: Some(0),
            unit: Some(Unit::Meters),
        }
    }
}*/

#[derive(Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct Hole {
    pub par: u32,
    #[serde(rename = "Ordinal")]
    pub hole_number: u32,
    pub length: Option<u32>,
}
#[derive(Debug, PartialEq)]
pub enum RoundStatus {
    Pending,
    Started,
    Finished,
}

#[derive(Debug, PartialEq, Clone)]
pub enum PlayerStatus {
    Pending,
    Started,
    Finished,
    DidNotFinish,
    DidNotStart,
}

impl PlayerStatus {
    pub fn is_troubled(&self) -> bool {
        matches!(
            self,
            PlayerStatus::DidNotFinish | PlayerStatus::DidNotStart | PlayerStatus::Pending
        )
    }
}
#[derive(Debug, PartialEq, Clone)]
enum Tied {
    Tied(usize),
    NotTied,
}

#[derive(Debug, PartialEq, Clone)]
pub struct PlayerScore {
    pub pdga_number: u32,
    pub hole_scores: Vec<u8>,
    pub throws: u8,
    pub round_to_par: i16,
    pub placement: u16,
    pub started: PlayerStatus,
    pub division: Division,
    pub(crate) name: String,
    pub(crate) first_name: String,
    pub(crate) last_name: String,
    pub(crate) avatar: Option<String>,
    tied: Tied,
}

fn placement_to_score(placement: u16) -> u16 {
    match placement {
        1 => 100,
        2 => 85,
        3 => 75,
        4 => 69,
        5 => 64,
        6 => 60,
        7 => 57,
        8..=20 => 54 - (placement - 8) * 2,
        21..=48 => 50 - placement,
        49..=50 => 2,
        _ => 0,
    }
}

impl PlayerScore {
    fn from_api(api: &ApiPlayer, tied: Tied) -> Self {
        let status: PlayerStatus = PlayerStatus::from(api);
        Self {
            pdga_number: api.pdga_number,
            throws: api.throws,
            round_to_par: api.round_to_par,
            hole_scores: api
                .hole_scores
                .iter()
                .filter_map(|s| s.parse::<u8>().ok())
                .collect(),
            placement: api.running_place.unwrap_or(0),
            started: status,
            division: api.division,
            name: api.name.clone(),
            first_name: api.first_name.clone(),
            last_name: api.last_name.clone(),
            avatar: api
                .avatar
                .clone()
                .map(|s| "https://www.pdga.com".to_string() + &s),
            tied,
        }
    }

    pub(crate) fn to_active_model(&self) -> entity::player::ActiveModel {
        entity::player::Model {
            pdga_number: self.pdga_number as i32,
            first_name: self.first_name.to_owned(),
            last_name: self.last_name.to_owned(),
            avatar: self.avatar.to_owned(),
        }
        .into_active_model()
    }
    pub(crate) fn to_division_active_model(
        &self,
        fantasy_tournament_id: i32,
    ) -> entity::player_division_in_fantasy_tournament::ActiveModel {
        entity::player_division_in_fantasy_tournament::Model {
            player_pdga_number: self.pdga_number as i32,
            fantasy_tournament_id,
            division: (&self.division).into(),
        }
        .into_active_model()
    }
    /// Returns ActiveModel if score is changed, otherwise None
    pub(crate) fn round_score_active_model(
        &self,
        round: i32,
        competition_id: i32,
        division: &Division,
    ) -> Option<ActiveModel> {
        if matches!(self.started, PlayerStatus::Started | PlayerStatus::Finished) {
            Some(ActiveModel {
                id: NotSet,
                pdga_number: Set(self.pdga_number as i32),
                competition_id: Set(competition_id),
                round: Set(round),
                throws: Set(self.throws as i32),
                division: Set(division.into()),
                placement: Set(self.placement as i32),
            })
        } else {
            None
        }
    }

    fn get_user_score(&self, level: CompetitionLevel) -> u16 {
        let score = match self.tied {
            Tied::Tied(tied) => {
                let mut tied_score: u32 = 0;
                for i in 0..=tied {
                    tied_score += placement_to_score(self.placement + i as u16) as u32;
                }
                tied_score /= (tied + 1) as u32;
                tied_score
            }
            Tied::NotTied => placement_to_score(self.placement) as u32,
        };
        (score as f64 * level.multiplier()).round() as u16
    }

    pub(crate) async fn get_user_fantasy_score(
        &self,
        db: &impl ConnectionTrait,
        fantasy_tournament_id: u32,
        competition_id: u32,
    ) -> Result<Option<UserScore>, GenericError> {
        let competition_level = if let Some(competition) = entity::competition::Entity::find()
            .filter(entity::competition::Column::Id.eq(competition_id as i32))
            .one(db)
            .await
            .map_err(|_| GenericError::UnknownError("Unable to find competition"))?
        {
            competition.level.into()
        } else {
            return Err(GenericError::NotFound("Competition not found"));
        };
        let score = self.get_user_score(competition_level) as i32;
        if let Ok(Some((user, benched, slot, division))) = self.get_user(db, fantasy_tournament_id).await {
            Ok(Some(UserScore {
                user: user.id,
                score,
                competition_id,
                pdga_num: self.pdga_number,
                fantasy_tournament_id,
                benched,
                slot,
                division,
            }))
        } else {
            Ok(None)
        }
    }

    /// Returns user and if the player is benched and slot number
    async fn get_user(
        &self,
        db: &impl ConnectionTrait,
        fantasy_id: u32,
    ) -> Result<Option<(user::Model, bool, u8, Division)>, GenericError> {
        if let Some(pick) = FantasyPick::find()
            .filter(
                fantasy_pick::Column::Player
                    .eq(self.pdga_number)
                    .and(fantasy_pick::Column::FantasyTournamentId.eq(fantasy_id)),
            )
            .one(db)
            .await
            .map_err(|_| GenericError::UnknownError("Pick not found due to unknown database error"))?
        {
            pick.find_related(User)
                .one(db)
                .await
                .map_err(|_| GenericError::UnknownError("User not found due to unknown database error"))
                .map(|u| u.map(|u| (u, pick.benched, pick.pick_number as u8, pick.division.into())))
        } else {
            Ok(None)
        }
    }
}

use crate::dto::pdga::get_competition::{RoundLabel, RoundLabelInfo};
use entity::sea_orm_active_enums::RoundTypeEnum;
use serde_with::VecSkipError;

#[serde_as]
#[derive(Deserialize, Debug)]
struct RoundFromApi {
    layouts: Vec<Layout>,
    #[serde_as(as = "VecSkipError<_>")]
    scores: Vec<ApiPlayer>,
    #[serde(skip)]
    div: Division,
}

fn fix_length(length: Option<u32>, unit: Option<Unit>) -> Option<u32> {
    match (unit, length) {
        (Some(Unit::Feet), Some(length)) => Some((length as f64 * 0.3048).round() as u32),
        (Some(Unit::Meters), Some(length)) => Some(length),
        _ => None,
    }
}

#[derive(Debug, PartialEq)]
pub struct RoundInformation {
    pub holes: Vec<Hole>,
    pub players: Vec<PlayerScore>,
    pub course_length: u32,
    pub round_number: usize,
    pub competition_id: usize,
    pub divs: Vec<Division>,
    pub label: RoundLabelInfo,
    phantom: bool,
}
fn count_ties(player_score: &ApiPlayer, scores: &[ApiPlayer]) -> Tied {
    let tied_counter = scores
        .iter()
        .filter(|s| {
            s.running_place == player_score.running_place && s.pdga_number != player_score.pdga_number
        })
        .count();
    if tied_counter == 0 {
        Tied::NotTied
    } else {
        Tied::Tied(tied_counter)
    }
}

impl RoundInformation {
    pub async fn new(
        competition_id: usize,
        given_divs: Vec<Division>,
        round_label: &RoundLabelInfo,
        all_round_labels: &[RoundLabelInfo],
    ) -> Result<Self, GenericError> {
        let mut divs: Vec<RoundFromApi> = vec![];
        let mut maybe_error: Result<(), GenericError> = Ok(());
        for div in given_divs {
            let new_div = Self::get_one_div(competition_id, round_label.round_number, div).await;

            if let Ok(new_div) = new_div {
                divs.push(new_div);
            } else if let Err(e) = new_div {
                warn!("Unable to get round and div from PDGA: {:#?}", e);
                maybe_error = Err(e);
            }
        }
        if divs.is_empty() {
            maybe_error?;
        }

        if !divs.is_empty() {
            let layout: Layout = divs
                .iter()
                .map(|d| d.layouts.first().unwrap())
                .next()
                .unwrap()
                .to_owned();
            let divisions = divs.iter().map(|d| d.div.to_owned()).collect();

            let player_scores: Vec<PlayerScore> = divs
                .into_iter()
                .flat_map(|d| {
                    d.scores
                        .iter()
                        .map(|player_score| {
                            let tied = count_ties(player_score, &d.scores);
                            PlayerScore::from_api(player_score, tied)
                        })
                        .collect_vec()
                })
                .collect_vec();
            Ok(Self::make_self(
                player_scores,
                layout,
                competition_id,
                round_label.get_round_number_from_label(all_round_labels),
                divisions,
                round_label.to_owned(),
            ))
        } else {
            Err(GenericError::NotFound(
                "No round found containing any divisions supported by Rustling Chains",
            ))
        }
    }

    pub fn phantom(
        round_label_info: &RoundLabelInfo,
        competition_id: usize,
        round_labels: &[RoundLabelInfo],
    ) -> Self {
        RoundInformation {
            holes: vec![],
            players: vec![],
            course_length: 0,
            round_number: round_label_info.get_round_number_from_label(round_labels),
            label: round_label_info.to_owned(),
            competition_id,
            divs: vec![],
            phantom: true,
        }
    }

    fn make_self(
        player_scores: Vec<PlayerScore>,
        layout: Layout,
        competition_id: usize,
        round_number: usize,
        divs: Vec<Division>,
        label: RoundLabelInfo,
    ) -> Self {
        let holes = layout
            .holes
            .iter()
            .map(|h| Hole {
                par: h.par,
                hole_number: h.hole_number,
                length: fix_length(h.length, layout.unit),
            })
            .collect_vec();

        let length = match layout.unit {
            Some(Unit::Feet) => (layout.length.unwrap_or_default() as f64 * 0.3048).round() as u32,
            Some(Unit::Meters) => (layout.length.unwrap_or_default() as f64).round() as u32,
            None => 0,
        };

        RoundInformation {
            holes,
            players: player_scores
                .into_iter()
                .filter_map(|p| {
                    if p.started != PlayerStatus::DidNotStart || p.started != PlayerStatus::DidNotFinish {
                        Some(p)
                    } else {
                        None
                    }
                })
                .collect(),
            course_length: length,
            round_number,
            competition_id,
            divs,
            phantom: false,
            label,
        }
    }

    async fn get_one_div(
        competition_id: usize,
        round: usize,
        div: Division,
    ) -> Result<RoundFromApi, GenericError> {
        let div_str = div.to_string().to_uppercase();
        let url = format!("https://www.pdga.com/apps/tournament/live-api/live_results_fetch_round.php?TournID={competition_id}&Round={round}&Division={div_str}");
        //dbg!(&url);
        //tokio::time::sleep(std::time::Duration::from_millis(250)).await;

        let mut resp: ApiRes = reqwest::get(url)
            .await
            .map_err(|_| GenericError::UnknownError("Internal error while fetching round from PDGA"))?
            .json()
            .await
            .map_err(|e| {
                warn!("Unable to parse PDGA round response: {}", e);
                GenericError::UnknownError("Internal error while converting PDGA round to internal format")
            })?;
        resp.data.div = div;
        Ok(resp.data)
    }

    pub fn all_player_scores(&self) -> Vec<PlayerScore> {
        self.players.clone()
    }

    pub fn all_player_round_score_active_models(&self, round: i32, competition_id: i32) -> Vec<ActiveModel> {
        self.players
            .iter()
            .filter_map(|p| p.round_score_active_model(round, competition_id, &p.division))
            .collect()
    }

    pub fn all_player_active_models(&self) -> Vec<entity::player::ActiveModel> {
        self.players.iter().map(|p| p.to_active_model()).collect()
    }

    pub async fn all_player_scores_exist_in_db(&self, db: &impl ConnectionTrait) -> Result<bool, DbErr> {
        player_round_score::Entity::find()
            .filter(player_round_score::Column::Round.eq(self.round_number as i32))
            .all(db)
            .await
            .map(|x| x.len() == self.players.len())
    }

    pub(crate) fn status(&self) -> RoundStatus {
        let players = self
            .players
            .iter()
            .filter(|p| !p.started.is_troubled())
            .collect_vec();

        let is_majority_finished = players
            .iter()
            .filter(|p| p.started == PlayerStatus::Finished)
            .count()
            >= (players.len() / 2);

        if self.phantom || players.is_empty() {
            RoundStatus::Pending
        } else if players.iter().all(|p| p.started == PlayerStatus::Finished) || is_majority_finished {
            RoundStatus::Finished
        } else if players
            .iter()
            .any(|p| p.started == PlayerStatus::Started || p.started == PlayerStatus::Finished)
        {
            RoundStatus::Started
        } else {
            RoundStatus::Pending
        }
    }

    pub fn active_model(&self, date: DateTimeWithTimeZone) -> entity::round::ActiveModel {
        entity::round::ActiveModel {
            id: NotSet,
            round_number: Set(self.round_number as i32),
            competition_id: Set(self.competition_id as i32),
            status: Set(self.status().into()),
            date: Set(date),
            round_type: Set(Some(RoundTypeEnum::from(&self.label))),
        }
    }
}
impl From<&RoundLabelInfo> for RoundTypeEnum {
    fn from(value: &RoundLabelInfo) -> Self {
        match value.label {
            RoundLabel::Final => RoundTypeEnum::Final,
            RoundLabel::Playoff => RoundTypeEnum::Playoff,
            RoundLabel::Round(_) => RoundTypeEnum::Round,
            RoundLabel::Other => RoundTypeEnum::Unknown,
        }
    }
}

impl From<RoundTypeEnum> for RoundLabel {
    fn from(value: RoundTypeEnum) -> Self {
        match value {
            RoundTypeEnum::Final => RoundLabel::Final,
            RoundTypeEnum::Playoff => RoundLabel::Playoff,
            RoundTypeEnum::Unknown | RoundTypeEnum::Round => RoundLabel::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::dto::pdga::player_scoring::ApiRes;

    #[test]
    fn test_parse_comp() {
        use serde_json::from_str;
        use std::fs;

        // Specify the path to your JSON file
        let path = "/home/brynte/RustroverProjects/CTP-Fantasy/pdga_schema/round_thing.json";

        // Read the file content
        let content = fs::read_to_string(path).expect("Could not read file");

        // Deserialize the content into ApiCompetitionInfo
        let info: Result<ApiRes, _> = from_str(&content);

        assert!(info.is_ok());

        // Now you can use `info` for your assertions
    }

    #[test]
    fn test_pdga_round() {
        let test_string = r#"{
  "data":
  {
    "pool": "",
    "layouts":
    [
      {
        "LayoutID": 654591,
        "CourseID": -1,
        "CourseName": null,
        "TournID": 75961,
        "Name": "Pioneer Open",
        "Holes": 18,
        "Par": 60,
        "Length": 7504,
        "Units": "Feet",
        "Accuracy": "M",
        "H1": 3,
        "H2": 3,
        "H3": 4,
        "H4": 4,
        "H5": 3,
        "H6": 3,
        "H7": 3,
        "H8": 4,
        "H9": 3,
        "H10": 3,
        "H11": 3,
        "H12": 3,
        "H13": 3,
        "H14": 3,
        "H15": 4,
        "H16": 4,
        "H17": 3,
        "H18": 4,
        "H19": 3,
        "H20": 3,
        "H21": 3,
        "H22": 3,
        "H23": 3,
        "H24": 3,
        "H25": 3,
        "H26": 3,
        "H27": 3,
        "H28": 3,
        "H29": 3,
        "H30": 3,
        "H31": 3,
        "H32": 3,
        "H33": 3,
        "H34": 3,
        "H35": 3,
        "H36": 3,
        "SSARd1": "56.028",
        "SSARd2": null,
        "SSARd3": null,
        "SSARd4": null,
        "SSARd5": null,
        "SSARd6": null,
        "SSARd7": null,
        "SSARd8": null,
        "SSARd9": null,
        "SSARd10": null,
        "SSASemis": null,
        "SSAFinals": null,
        "CombinedSSA": null,
        "ProvisionalSSA": null,
        "ChallengeFactor": null,
        "UpdateDate": "2024-02-16 21:03:02",
        "Detail":
        [
          {
            "Hole": "H1",
            "HoleOrdinal": 1,
            "Label": "1",
            "Par": 3,
            "Length": 449,
            "Units": null,
            "Accuracy": null,
            "Ordinal": 1
          },
          {
            "Hole": "H2",
            "HoleOrdinal": 2,
            "Label": "2",
            "Par": 3,
            "Length": 299,
            "Units": null,
            "Accuracy": null,
            "Ordinal": 2
          },
          {
            "Hole": "H3",
            "HoleOrdinal": 3,
            "Label": "3",
            "Par": 4,
            "Length": 482,
            "Units": null,
            "Accuracy": null,
            "Ordinal": 3
          },
          {
            "Hole": "H4",
            "HoleOrdinal": 4,
            "Label": "4",
            "Par": 4,
            "Length": 586,
            "Units": null,
            "Accuracy": null,
            "Ordinal": 4
          },
          {
            "Hole": "H5",
            "HoleOrdinal": 5,
            "Label": "5",
            "Par": 3,
            "Length": 364,
            "Units": null,
            "Accuracy": null,
            "Ordinal": 5
          },
          {
            "Hole": "H6",
            "HoleOrdinal": 6,
            "Label": "6",
            "Par": 3,
            "Length": 395,
            "Units": null,
            "Accuracy": null,
            "Ordinal": 6
          },
          {
            "Hole": "H7",
            "HoleOrdinal": 7,
            "Label": "7",
            "Par": 3,
            "Length": 296,
            "Units": null,
            "Accuracy": null,
            "Ordinal": 7
          },
          {
            "Hole": "H8",
            "HoleOrdinal": 8,
            "Label": "8",
            "Par": 4,
            "Length": 460,
            "Units": null,
            "Accuracy": null,
            "Ordinal": 8
          },
          {
            "Hole": "H9",
            "HoleOrdinal": 9,
            "Label": "9",
            "Par": 3,
            "Length": 359,
            "Units": null,
            "Accuracy": null,
            "Ordinal": 9
          },
          {
            "Hole": "H10",
            "HoleOrdinal": 10,
            "Label": "10",
            "Par": 3,
            "Length": 434,
            "Units": null,
            "Accuracy": null,
            "Ordinal": 10
          },
          {
            "Hole": "H11",
            "HoleOrdinal": 11,
            "Label": "11",
            "Par": 3,
            "Length": 393,
            "Units": null,
            "Accuracy": null,
            "Ordinal": 11
          },
          {
            "Hole": "H12",
            "HoleOrdinal": 12,
            "Label": "12",
            "Par": 3,
            "Length": 342,
            "Units": null,
            "Accuracy": null,
            "Ordinal": 12
          },
          {
            "Hole": "H13",
            "HoleOrdinal": 13,
            "Label": "13",
            "Par": 3,
            "Length": 350,
            "Units": null,
            "Accuracy": null,
            "Ordinal": 13
          },
          {
            "Hole": "H14",
            "HoleOrdinal": 14,
            "Label": "14",
            "Par": 3,
            "Length": 385,
            "Units": null,
            "Accuracy": null,
            "Ordinal": 14
          },
          {
            "Hole": "H15",
            "HoleOrdinal": 15,
            "Label": "15",
            "Par": 4,
            "Length": 434,
            "Units": null,
            "Accuracy": null,
            "Ordinal": 15
          },
          {
            "Hole": "H16",
            "HoleOrdinal": 16,
            "Label": "16",
            "Par": 4,
            "Length": 528,
            "Units": null,
            "Accuracy": null,
            "Ordinal": 16
          },
          {
            "Hole": "H17",
            "HoleOrdinal": 17,
            "Label": "17",
            "Par": 3,
            "Length": 401,
            "Units": null,
            "Accuracy": null,
            "Ordinal": 17
          },
          {
            "Hole": "H18",
            "HoleOrdinal": 18,
            "Label": "18",
            "Par": 4,
            "Length": 547,
            "Units": null,
            "Accuracy": null,
            "Ordinal": 18
          }
        ]
      }
    ],
    "division": "FPO",
    "live_round_id": 122269426,
    "id": 105,
    "shotgun_time": "",
    "tee_times": false,
    "holes":
    [
      {
        "Hole": "H1",
        "HoleOrdinal": 1,
        "Label": "1",
        "Par": 3,
        "Length": 449,
        "Units": null,
        "Accuracy": null,
        "Ordinal": 1
      },
      {
        "Hole": "H2",
        "HoleOrdinal": 2,
        "Label": "2",
        "Par": 3,
        "Length": 299,
        "Units": null,
        "Accuracy": null,
        "Ordinal": 2
      },
      {
        "Hole": "H3",
        "HoleOrdinal": 3,
        "Label": "3",
        "Par": 4,
        "Length": 482,
        "Units": null,
        "Accuracy": null,
        "Ordinal": 3
      },
      {
        "Hole": "H4",
        "HoleOrdinal": 4,
        "Label": "4",
        "Par": 4,
        "Length": 586,
        "Units": null,
        "Accuracy": null,
        "Ordinal": 4
      },
      {
        "Hole": "H5",
        "HoleOrdinal": 5,
        "Label": "5",
        "Par": 3,
        "Length": 364,
        "Units": null,
        "Accuracy": null,
        "Ordinal": 5
      },
      {
        "Hole": "H6",
        "HoleOrdinal": 6,
        "Label": "6",
        "Par": 3,
        "Length": 395,
        "Units": null,
        "Accuracy": null,
        "Ordinal": 6
      },
      {
        "Hole": "H7",
        "HoleOrdinal": 7,
        "Label": "7",
        "Par": 3,
        "Length": 296,
        "Units": null,
        "Accuracy": null,
        "Ordinal": 7
      },
      {
        "Hole": "H8",
        "HoleOrdinal": 8,
        "Label": "8",
        "Par": 4,
        "Length": 460,
        "Units": null,
        "Accuracy": null,
        "Ordinal": 8
      },
      {
        "Hole": "H9",
        "HoleOrdinal": 9,
        "Label": "9",
        "Par": 3,
        "Length": 359,
        "Units": null,
        "Accuracy": null,
        "Ordinal": 9
      },
      {
        "Hole": "H10",
        "HoleOrdinal": 10,
        "Label": "10",
        "Par": 3,
        "Length": 434,
        "Units": null,
        "Accuracy": null,
        "Ordinal": 10
      },
      {
        "Hole": "H11",
        "HoleOrdinal": 11,
        "Label": "11",
        "Par": 3,
        "Length": 393,
        "Units": null,
        "Accuracy": null,
        "Ordinal": 11
      },
      {
        "Hole": "H12",
        "HoleOrdinal": 12,
        "Label": "12",
        "Par": 3,
        "Length": 342,
        "Units": null,
        "Accuracy": null,
        "Ordinal": 12
      },
      {
        "Hole": "H13",
        "HoleOrdinal": 13,
        "Label": "13",
        "Par": 3,
        "Length": 350,
        "Units": null,
        "Accuracy": null,
        "Ordinal": 13
      },
      {
        "Hole": "H14",
        "HoleOrdinal": 14,
        "Label": "14",
        "Par": 3,
        "Length": 385,
        "Units": null,
        "Accuracy": null,
        "Ordinal": 14
      },
      {
        "Hole": "H15",
        "HoleOrdinal": 15,
        "Label": "15",
        "Par": 4,
        "Length": 434,
        "Units": null,
        "Accuracy": null,
        "Ordinal": 15
      },
      {
        "Hole": "H16",
        "HoleOrdinal": 16,
        "Label": "16",
        "Par": 4,
        "Length": 528,
        "Units": null,
        "Accuracy": null,
        "Ordinal": 16
      },
      {
        "Hole": "H17",
        "HoleOrdinal": 17,
        "Label": "17",
        "Par": 3,
        "Length": 401,
        "Units": null,
        "Accuracy": null,
        "Ordinal": 17
      },
      {
        "Hole": "H18",
        "HoleOrdinal": 18,
        "Label": "18",
        "Par": 4,
        "Length": 547,
        "Units": null,
        "Accuracy": null,
        "Ordinal": 18
      }
    ],
    "scores":
    [
      {
        "ResultID": 210710463,
        "RoundID": 122269426,
        "ScoreID": null,
        "FirstName": "Lori",
        "LastName": "Beierle",
        "Name": "Lori Beierle",
        "AvatarURL": "/files/styles/large/public/pictures/picture-297751-1613191208.jpg",
        "City": "Chehalis",
        "Country": "US",
        "Nationality": null,
        "StateProv": "WA",
        "PDGANum": 82950,
        "HasPDGANum": 1,
        "Rating": 867,
        "Division": "FPO",
        "Pool": "",
        "Team": null,
        "TeamName": null,
        "Round": 2,
        "Authoritative": null,
        "ScorecardUpdatedAt": null,
        "WonPlayoff": "no",
        "Prize": null,
        "PrevRounds": 0,
        "RoundStatus": null,
        "Holes": 18,
        "LayoutID": 654591,
        "GrandTotal": 0,
        "CardNum": null,
        "TeeTime": "",
        "TeeStart": "",
        "HasGroupAssignment": 0,
        "PlayedPreviousRound": 0,
        "HasRoundScore": 0,
        "UpdateDate": null,
        "Played": null,
        "Completed": 0,
        "RoundStarted": 0,
        "PrevRndTotal": 0,
        "RoundScore": 0,
        "SubTotal": 0,
        "RoundtoPar": 0,
        "ToPar": null,
        "Scores": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "SortScores": "|||||||||||||||||||||||||||||||||||",
        "Pars": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "Rounds": ",,,,,,,,,,,",
        "SortRounds": "777",
        "RoundRating": null,
        "PreviousPlace": null,
        "FullLocation": "Chehalis, WA",
        "ShortName": "L. Beierle",
        "ProfileURL": "https://www.pdga.com/player/82950",
        "ParThruRound": 0,
        "RoundPool": "",
        "Teammates": [],
        "TeeTimeSort": "",
        "PlayerThrowStatus": null,
        "HoleScores":
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          ""
        ]
      },
      {
        "ResultID": 210700153,
        "RoundID": 122269426,
        "ScoreID": null,
        "FirstName": "Alison",
        "LastName": "Blakeman",
        "Name": "Alison Blakeman",
        "AvatarURL": "/files/styles/large/public/pictures/picture-200566-1623608631.jpg",
        "City": "Kennewick",
        "Country": "US",
        "Nationality": null,
        "StateProv": "WA",
        "PDGANum": 48199,
        "HasPDGANum": 1,
        "Rating": 886,
        "Division": "FPO",
        "Pool": "",
        "Team": null,
        "TeamName": null,
        "Round": 2,
        "Authoritative": null,
        "ScorecardUpdatedAt": null,
        "WonPlayoff": "no",
        "Prize": null,
        "PrevRounds": 0,
        "RoundStatus": null,
        "Holes": 18,
        "LayoutID": 654591,
        "GrandTotal": 0,
        "CardNum": null,
        "TeeTime": "",
        "TeeStart": "",
        "HasGroupAssignment": 0,
        "PlayedPreviousRound": 0,
        "HasRoundScore": 0,
        "UpdateDate": null,
        "Played": null,
        "Completed": 0,
        "RoundStarted": 0,
        "PrevRndTotal": 0,
        "RoundScore": 0,
        "SubTotal": 0,
        "RoundtoPar": 0,
        "ToPar": null,
        "Scores": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "SortScores": "|||||||||||||||||||||||||||||||||||",
        "Pars": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "Rounds": ",,,,,,,,,,,",
        "SortRounds": "777",
        "RoundRating": null,
        "PreviousPlace": null,
        "FullLocation": "Kennewick, WA",
        "ShortName": "A. Blakeman",
        "ProfileURL": "https://www.pdga.com/player/48199",
        "ParThruRound": 0,
        "RoundPool": "",
        "Teammates": [],
        "TeeTimeSort": "",
        "PlayerThrowStatus": null,
        "HoleScores":
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          ""
        ]
      },
      {
        "ResultID": 210726477,
        "RoundID": 122269426,
        "ScoreID": null,
        "FirstName": "Sofia",
        "LastName": "Donnecke",
        "Name": "Sofia Donnecke",
        "AvatarURL": "/files/styles/large/public/pictures/picture-1996996-1691252667.jpg",
        "City": "Victoria",
        "Country": "CA",
        "Nationality": "CA",
        "StateProv": "BC",
        "PDGANum": 185534,
        "HasPDGANum": 1,
        "Rating": 922,
        "Division": "FPO",
        "Pool": "",
        "Team": null,
        "TeamName": null,
        "Round": 2,
        "Authoritative": null,
        "ScorecardUpdatedAt": null,
        "WonPlayoff": "no",
        "Prize": null,
        "PrevRounds": 0,
        "RoundStatus": null,
        "Holes": 18,
        "LayoutID": 654591,
        "GrandTotal": 0,
        "CardNum": null,
        "TeeTime": "",
        "TeeStart": "",
        "HasGroupAssignment": 0,
        "PlayedPreviousRound": 0,
        "HasRoundScore": 0,
        "UpdateDate": null,
        "Played": null,
        "Completed": 0,
        "RoundStarted": 0,
        "PrevRndTotal": 0,
        "RoundScore": 0,
        "SubTotal": 0,
        "RoundtoPar": 0,
        "ToPar": null,
        "Scores": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "SortScores": "|||||||||||||||||||||||||||||||||||",
        "Pars": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "Rounds": ",,,,,,,,,,,",
        "SortRounds": "777",
        "RoundRating": null,
        "PreviousPlace": null,
        "FullLocation": "Victoria, BC",
        "ShortName": "S. Donnecke",
        "ProfileURL": "https://www.pdga.com/player/185534",
        "ParThruRound": 0,
        "RoundPool": "",
        "Teammates": [],
        "TeeTimeSort": "",
        "PlayerThrowStatus": null,
        "HoleScores":
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          ""
        ]
      },
      {
        "ResultID": 210701415,
        "RoundID": 122269426,
        "ScoreID": null,
        "FirstName": "Candace",
        "LastName": "Kennedy",
        "Name": "Candace Kennedy",
        "AvatarURL": "/files/styles/large/public/pictures/picture-1664246-1618961276.jpg",
        "City": "Tacoma",
        "Country": "US",
        "Nationality": null,
        "StateProv": "WA",
        "PDGANum": 154343,
        "HasPDGANum": 1,
        "Rating": 845,
        "Division": "FPO",
        "Pool": "",
        "Team": null,
        "TeamName": null,
        "Round": 2,
        "Authoritative": null,
        "ScorecardUpdatedAt": null,
        "WonPlayoff": "no",
        "Prize": null,
        "PrevRounds": 0,
        "RoundStatus": null,
        "Holes": 18,
        "LayoutID": 654591,
        "GrandTotal": 0,
        "CardNum": null,
        "TeeTime": "",
        "TeeStart": "",
        "HasGroupAssignment": 0,
        "PlayedPreviousRound": 0,
        "HasRoundScore": 0,
        "UpdateDate": null,
        "Played": null,
        "Completed": 0,
        "RoundStarted": 0,
        "PrevRndTotal": 0,
        "RoundScore": 0,
        "SubTotal": 0,
        "RoundtoPar": 0,
        "ToPar": null,
        "Scores": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "SortScores": "|||||||||||||||||||||||||||||||||||",
        "Pars": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "Rounds": ",,,,,,,,,,,",
        "SortRounds": "777",
        "RoundRating": null,
        "PreviousPlace": null,
        "FullLocation": "Tacoma, WA",
        "ShortName": "C. Kennedy",
        "ProfileURL": "https://www.pdga.com/player/154343",
        "ParThruRound": 0,
        "RoundPool": "",
        "Teammates": [],
        "TeeTimeSort": "",
        "PlayerThrowStatus": null,
        "HoleScores":
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          ""
        ]
      },
      {
        "ResultID": 210702073,
        "RoundID": 122269426,
        "ScoreID": null,
        "FirstName": "Brittany",
        "LastName": "Leaman-Snyder",
        "Name": "Brittany Leaman-Snyder",
        "AvatarURL": "/files/styles/large/public/pictures/picture-638961-1695533950.png",
        "City": "Redmond",
        "Country": "US",
        "Nationality": null,
        "StateProv": "OR",
        "PDGANum": 101295,
        "HasPDGANum": 1,
        "Rating": 899,
        "Division": "FPO",
        "Pool": "",
        "Team": null,
        "TeamName": null,
        "Round": 2,
        "Authoritative": null,
        "ScorecardUpdatedAt": null,
        "WonPlayoff": "no",
        "Prize": null,
        "PrevRounds": 0,
        "RoundStatus": null,
        "Holes": 18,
        "LayoutID": 654591,
        "GrandTotal": 0,
        "CardNum": null,
        "TeeTime": "",
        "TeeStart": "",
        "HasGroupAssignment": 0,
        "PlayedPreviousRound": 0,
        "HasRoundScore": 0,
        "UpdateDate": null,
        "Played": null,
        "Completed": 0,
        "RoundStarted": 0,
        "PrevRndTotal": 0,
        "RoundScore": 0,
        "SubTotal": 0,
        "RoundtoPar": 0,
        "ToPar": null,
        "Scores": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "SortScores": "|||||||||||||||||||||||||||||||||||",
        "Pars": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "Rounds": ",,,,,,,,,,,",
        "SortRounds": "777",
        "RoundRating": null,
        "PreviousPlace": null,
        "FullLocation": "Redmond, OR",
        "ShortName": "B. Leaman-Snyder",
        "ProfileURL": "https://www.pdga.com/player/101295",
        "ParThruRound": 0,
        "RoundPool": "",
        "Teammates": [],
        "TeeTimeSort": "",
        "PlayerThrowStatus": null,
        "HoleScores":
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          ""
        ]
      },
      {
        "ResultID": 210720680,
        "RoundID": 122269426,
        "ScoreID": null,
        "FirstName": "Kristy",
        "LastName": "Lee",
        "Name": "Kristy Lee",
        "AvatarURL": null,
        "City": "Victoria",
        "Country": "CA",
        "Nationality": null,
        "StateProv": "BC",
        "PDGANum": 75818,
        "HasPDGANum": 1,
        "Rating": 911,
        "Division": "FPO",
        "Pool": "",
        "Team": null,
        "TeamName": null,
        "Round": 2,
        "Authoritative": null,
        "ScorecardUpdatedAt": null,
        "WonPlayoff": "no",
        "Prize": null,
        "PrevRounds": 0,
        "RoundStatus": null,
        "Holes": 18,
        "LayoutID": 654591,
        "GrandTotal": 0,
        "CardNum": null,
        "TeeTime": "",
        "TeeStart": "",
        "HasGroupAssignment": 0,
        "PlayedPreviousRound": 0,
        "HasRoundScore": 0,
        "UpdateDate": null,
        "Played": null,
        "Completed": 0,
        "RoundStarted": 0,
        "PrevRndTotal": 0,
        "RoundScore": 0,
        "SubTotal": 0,
        "RoundtoPar": 0,
        "ToPar": null,
        "Scores": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "SortScores": "|||||||||||||||||||||||||||||||||||",
        "Pars": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "Rounds": ",,,,,,,,,,,",
        "SortRounds": "777",
        "RoundRating": null,
        "PreviousPlace": null,
        "FullLocation": "Victoria, BC",
        "ShortName": "K. Lee",
        "ProfileURL": "https://www.pdga.com/player/75818",
        "ParThruRound": 0,
        "RoundPool": "",
        "Teammates": [],
        "TeeTimeSort": "",
        "PlayerThrowStatus": null,
        "HoleScores":
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          ""
        ]
      },
      {
        "ResultID": 210700146,
        "RoundID": 122269426,
        "ScoreID": null,
        "FirstName": "Amy",
        "LastName": "Lewis",
        "Name": "Amy Lewis",
        "AvatarURL": "/files/styles/large/public/pictures/picture-177661-1679238941.jpg",
        "City": "Myrtle Creek",
        "Country": "US",
        "Nationality": "US",
        "StateProv": "OR",
        "PDGANum": 61950,
        "HasPDGANum": 1,
        "Rating": 928,
        "Division": "FPO",
        "Pool": "",
        "Team": null,
        "TeamName": null,
        "Round": 2,
        "Authoritative": null,
        "ScorecardUpdatedAt": null,
        "WonPlayoff": "no",
        "Prize": null,
        "PrevRounds": 0,
        "RoundStatus": null,
        "Holes": 18,
        "LayoutID": 654591,
        "GrandTotal": 0,
        "CardNum": null,
        "TeeTime": "",
        "TeeStart": "",
        "HasGroupAssignment": 0,
        "PlayedPreviousRound": 0,
        "HasRoundScore": 0,
        "UpdateDate": null,
        "Played": null,
        "Completed": 0,
        "RoundStarted": 0,
        "PrevRndTotal": 0,
        "RoundScore": 0,
        "SubTotal": 0,
        "RoundtoPar": 0,
        "ToPar": null,
        "Scores": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "SortScores": "|||||||||||||||||||||||||||||||||||",
        "Pars": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "Rounds": ",,,,,,,,,,,",
        "SortRounds": "777",
        "RoundRating": null,
        "PreviousPlace": null,
        "FullLocation": "Myrtle Creek, OR",
        "ShortName": "A. Lewis",
        "ProfileURL": "https://www.pdga.com/player/61950",
        "ParThruRound": 0,
        "RoundPool": "",
        "Teammates": [],
        "TeeTimeSort": "",
        "PlayerThrowStatus": null,
        "HoleScores":
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          ""
        ]
      },
      {
        "ResultID": 210700126,
        "RoundID": 122269426,
        "ScoreID": null,
        "FirstName": "Samii The Tutu",
        "LastName": "Maes",
        "Name": "Samii The Tutu Maes",
        "AvatarURL": "/files/styles/large/public/pictures/picture-299901-1663096418.jpg",
        "City": "Waitsburg",
        "Country": "US",
        "Nationality": "US",
        "StateProv": "WA",
        "PDGANum": 84007,
        "HasPDGANum": 1,
        "Rating": 751,
        "Division": "FPO",
        "Pool": "",
        "Team": null,
        "TeamName": null,
        "Round": 2,
        "Authoritative": null,
        "ScorecardUpdatedAt": null,
        "WonPlayoff": "no",
        "Prize": null,
        "PrevRounds": 0,
        "RoundStatus": null,
        "Holes": 18,
        "LayoutID": 654591,
        "GrandTotal": 0,
        "CardNum": null,
        "TeeTime": "",
        "TeeStart": "",
        "HasGroupAssignment": 0,
        "PlayedPreviousRound": 0,
        "HasRoundScore": 0,
        "UpdateDate": null,
        "Played": null,
        "Completed": 0,
        "RoundStarted": 0,
        "PrevRndTotal": 0,
        "RoundScore": 0,
        "SubTotal": 0,
        "RoundtoPar": 0,
        "ToPar": null,
        "Scores": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "SortScores": "|||||||||||||||||||||||||||||||||||",
        "Pars": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "Rounds": ",,,,,,,,,,,",
        "SortRounds": "777",
        "RoundRating": null,
        "PreviousPlace": null,
        "FullLocation": "Waitsburg, WA",
        "ShortName": "S. Maes",
        "ProfileURL": "https://www.pdga.com/player/84007",
        "ParThruRound": 0,
        "RoundPool": "",
        "Teammates": [],
        "TeeTimeSort": "",
        "PlayerThrowStatus": null,
        "HoleScores":
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          ""
        ]
      },
      {
        "ResultID": 210712106,
        "RoundID": 122269426,
        "ScoreID": null,
        "FirstName": "Katie",
        "LastName": "Maixner",
        "Name": "Katie Maixner",
        "AvatarURL": "/files/styles/large/public/pictures/picture-1097466-1668882631.jpg",
        "City": "Junction City",
        "Country": "US",
        "Nationality": "US",
        "StateProv": "OR",
        "PDGANum": 115863,
        "HasPDGANum": 1,
        "Rating": 838,
        "Division": "FPO",
        "Pool": "",
        "Team": null,
        "TeamName": null,
        "Round": 2,
        "Authoritative": null,
        "ScorecardUpdatedAt": null,
        "WonPlayoff": "no",
        "Prize": null,
        "PrevRounds": 0,
        "RoundStatus": null,
        "Holes": 18,
        "LayoutID": 654591,
        "GrandTotal": 0,
        "CardNum": null,
        "TeeTime": "",
        "TeeStart": "",
        "HasGroupAssignment": 0,
        "PlayedPreviousRound": 0,
        "HasRoundScore": 0,
        "UpdateDate": null,
        "Played": null,
        "Completed": 0,
        "RoundStarted": 0,
        "PrevRndTotal": 0,
        "RoundScore": 0,
        "SubTotal": 0,
        "RoundtoPar": 0,
        "ToPar": null,
        "Scores": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "SortScores": "|||||||||||||||||||||||||||||||||||",
        "Pars": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "Rounds": ",,,,,,,,,,,",
        "SortRounds": "777",
        "RoundRating": null,
        "PreviousPlace": null,
        "FullLocation": "Junction City, OR",
        "ShortName": "K. Maixner",
        "ProfileURL": "https://www.pdga.com/player/115863",
        "ParThruRound": 0,
        "RoundPool": "",
        "Teammates": [],
        "TeeTimeSort": "",
        "PlayerThrowStatus": null,
        "HoleScores":
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          ""
        ]
      },
      {
        "ResultID": 210700123,
        "RoundID": 122269426,
        "ScoreID": null,
        "FirstName": "Jennifer",
        "LastName": "Rice",
        "Name": "Jennifer Rice",
        "AvatarURL": null,
        "City": "Edmonds",
        "Country": "US",
        "Nationality": null,
        "StateProv": "WA",
        "PDGANum": 151072,
        "HasPDGANum": 1,
        "Rating": 881,
        "Division": "FPO",
        "Pool": "",
        "Team": null,
        "TeamName": null,
        "Round": 2,
        "Authoritative": null,
        "ScorecardUpdatedAt": null,
        "WonPlayoff": "no",
        "Prize": null,
        "PrevRounds": 0,
        "RoundStatus": null,
        "Holes": 18,
        "LayoutID": 654591,
        "GrandTotal": 0,
        "CardNum": null,
        "TeeTime": "",
        "TeeStart": "",
        "HasGroupAssignment": 0,
        "PlayedPreviousRound": 0,
        "HasRoundScore": 0,
        "UpdateDate": null,
        "Played": null,
        "Completed": 0,
        "RoundStarted": 0,
        "PrevRndTotal": 0,
        "RoundScore": 0,
        "SubTotal": 0,
        "RoundtoPar": 0,
        "ToPar": null,
        "Scores": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "SortScores": "|||||||||||||||||||||||||||||||||||",
        "Pars": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "Rounds": ",,,,,,,,,,,",
        "SortRounds": "777",
        "RoundRating": null,
        "PreviousPlace": null,
        "FullLocation": "Edmonds, WA",
        "ShortName": "J. Rice",
        "ProfileURL": "https://www.pdga.com/player/151072",
        "ParThruRound": 0,
        "RoundPool": "",
        "Teammates": [],
        "TeeTimeSort": "",
        "PlayerThrowStatus": null,
        "HoleScores":
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          ""
        ]
      },
      {
        "ResultID": 210700335,
        "RoundID": 122269426,
        "ScoreID": null,
        "FirstName": "Roxy",
        "LastName": "Russell",
        "Name": "Roxy Russell",
        "AvatarURL": "/files/styles/large/public/pictures/picture-1894906-1651694753.jpg",
        "City": "Vancouver",
        "Country": "US",
        "Nationality": "US",
        "StateProv": "WA",
        "PDGANum": 176407,
        "HasPDGANum": 1,
        "Rating": 870,
        "Division": "FPO",
        "Pool": "",
        "Team": null,
        "TeamName": null,
        "Round": 2,
        "Authoritative": null,
        "ScorecardUpdatedAt": null,
        "WonPlayoff": "no",
        "Prize": null,
        "PrevRounds": 0,
        "RoundStatus": null,
        "Holes": 18,
        "LayoutID": 654591,
        "GrandTotal": 0,
        "CardNum": null,
        "TeeTime": "",
        "TeeStart": "",
        "HasGroupAssignment": 0,
        "PlayedPreviousRound": 0,
        "HasRoundScore": 0,
        "UpdateDate": null,
        "Played": null,
        "Completed": 0,
        "RoundStarted": 0,
        "PrevRndTotal": 0,
        "RoundScore": 0,
        "SubTotal": 0,
        "RoundtoPar": 0,
        "ToPar": null,
        "Scores": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "SortScores": "|||||||||||||||||||||||||||||||||||",
        "Pars": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "Rounds": ",,,,,,,,,,,",
        "SortRounds": "777",
        "RoundRating": null,
        "PreviousPlace": null,
        "FullLocation": "Vancouver, WA",
        "ShortName": "R. Russell",
        "ProfileURL": "https://www.pdga.com/player/176407",
        "ParThruRound": 0,
        "RoundPool": "",
        "Teammates": [],
        "TeeTimeSort": "",
        "PlayerThrowStatus": null,
        "HoleScores":
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          ""
        ]
      },
      {
        "ResultID": 210700266,
        "RoundID": 122269426,
        "ScoreID": null,
        "FirstName": "Irina",
        "LastName": "Shakhova",
        "Name": "Irina Shakhova",
        "AvatarURL": "/files/styles/large/public/pictures/picture-1906871-1699219522.jpg",
        "City": "Rossport",
        "Country": "CA",
        "Nationality": "CA",
        "StateProv": "ON",
        "PDGANum": 177578,
        "HasPDGANum": 1,
        "Rating": 909,
        "Division": "FPO",
        "Pool": "",
        "Team": null,
        "TeamName": null,
        "Round": 2,
        "Authoritative": null,
        "ScorecardUpdatedAt": null,
        "WonPlayoff": "no",
        "Prize": null,
        "PrevRounds": 0,
        "RoundStatus": null,
        "Holes": 18,
        "LayoutID": 654591,
        "GrandTotal": 0,
        "CardNum": null,
        "TeeTime": "",
        "TeeStart": "",
        "HasGroupAssignment": 0,
        "PlayedPreviousRound": 0,
        "HasRoundScore": 0,
        "UpdateDate": null,
        "Played": null,
        "Completed": 0,
        "RoundStarted": 0,
        "PrevRndTotal": 0,
        "RoundScore": 0,
        "SubTotal": 0,
        "RoundtoPar": 0,
        "ToPar": null,
        "Scores": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "SortScores": "|||||||||||||||||||||||||||||||||||",
        "Pars": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "Rounds": ",,,,,,,,,,,",
        "SortRounds": "777",
        "RoundRating": null,
        "PreviousPlace": null,
        "FullLocation": "Rossport, ON",
        "ShortName": "I. Shakhova",
        "ProfileURL": "https://www.pdga.com/player/177578",
        "ParThruRound": 0,
        "RoundPool": "",
        "Teammates": [],
        "TeeTimeSort": "",
        "PlayerThrowStatus": null,
        "HoleScores":
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          ""
        ]
      },
      {
        "ResultID": 210724955,
        "RoundID": 122269426,
        "ScoreID": null,
        "FirstName": "Ashlyn",
        "LastName": "Tahlier",
        "Name": "Ashlyn Tahlier",
        "AvatarURL": "/files/styles/large/public/pictures/picture-1506206-1634331702.jpg",
        "City": "Eugene",
        "Country": "US",
        "Nationality": null,
        "StateProv": "OR",
        "PDGANum": 141044,
        "HasPDGANum": 1,
        "Rating": 906,
        "Division": "FPO",
        "Pool": "",
        "Team": null,
        "TeamName": null,
        "Round": 2,
        "Authoritative": null,
        "ScorecardUpdatedAt": null,
        "WonPlayoff": "no",
        "Prize": null,
        "PrevRounds": 0,
        "RoundStatus": null,
        "Holes": 18,
        "LayoutID": 654591,
        "GrandTotal": 0,
        "CardNum": null,
        "TeeTime": "",
        "TeeStart": "",
        "HasGroupAssignment": 0,
        "PlayedPreviousRound": 0,
        "HasRoundScore": 0,
        "UpdateDate": null,
        "Played": null,
        "Completed": 0,
        "RoundStarted": 0,
        "PrevRndTotal": 0,
        "RoundScore": 0,
        "SubTotal": 0,
        "RoundtoPar": 0,
        "ToPar": null,
        "Scores": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "SortScores": "|||||||||||||||||||||||||||||||||||",
        "Pars": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "Rounds": ",,,,,,,,,,,",
        "SortRounds": "777",
        "RoundRating": null,
        "PreviousPlace": null,
        "FullLocation": "Eugene, OR",
        "ShortName": "A. Tahlier",
        "ProfileURL": "https://www.pdga.com/player/141044",
        "ParThruRound": 0,
        "RoundPool": "",
        "Teammates": [],
        "TeeTimeSort": "",
        "PlayerThrowStatus": null,
        "HoleScores":
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          ""
        ]
      },
      {
        "ResultID": 210700144,
        "RoundID": 122269426,
        "ScoreID": null,
        "FirstName": "Madison",
        "LastName": "Tomaino",
        "Name": "Madison Tomaino",
        "AvatarURL": "/files/styles/large/public/pictures/picture-169011-1615397304.jpg",
        "City": "Portland",
        "Country": "US",
        "Nationality": null,
        "StateProv": "OR",
        "PDGANum": 60798,
        "HasPDGANum": 1,
        "Rating": 906,
        "Division": "FPO",
        "Pool": "",
        "Team": null,
        "TeamName": null,
        "Round": 2,
        "Authoritative": null,
        "ScorecardUpdatedAt": null,
        "WonPlayoff": "no",
        "Prize": null,
        "PrevRounds": 0,
        "RoundStatus": null,
        "Holes": 18,
        "LayoutID": 654591,
        "GrandTotal": 0,
        "CardNum": null,
        "TeeTime": "",
        "TeeStart": "",
        "HasGroupAssignment": 0,
        "PlayedPreviousRound": 0,
        "HasRoundScore": 0,
        "UpdateDate": null,
        "Played": null,
        "Completed": 0,
        "RoundStarted": 0,
        "PrevRndTotal": 0,
        "RoundScore": 0,
        "SubTotal": 0,
        "RoundtoPar": 0,
        "ToPar": null,
        "Scores": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "SortScores": "|||||||||||||||||||||||||||||||||||",
        "Pars": ",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,",
        "Rounds": ",,,,,,,,,,,",
        "SortRounds": "777",
        "RoundRating": null,
        "PreviousPlace": null,
        "FullLocation": "Portland, OR",
        "ShortName": "M. Tomaino",
        "ProfileURL": "https://www.pdga.com/player/60798",
        "ParThruRound": 0,
        "RoundPool": "",
        "Teammates": [],
        "TeeTimeSort": "",
        "PlayerThrowStatus": null,
        "HoleScores":
        [
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          "",
          ""
        ]
      }
    ]
  },
  "hash": "c52793bfda48fdd0fe3bff587b689e0d"
}"#;
        let resp: ApiRes = serde_json::from_str(test_string).unwrap();
        dbg!(resp);
    }
}
