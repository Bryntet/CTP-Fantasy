use crate::dto::{CompetitionLevel, Division, UserScore};
use entity::player_round_score::ActiveModel;
use entity::{fantasy_pick, player_round_score, user};

use crate::dto::pdga::ApiPlayer;
use crate::error::GenericError;
use entity::prelude::{FantasyPick, User};
use itertools::Itertools;
use sea_orm::ActiveValue::Set;
use sea_orm::{sea_query, ModelTrait};
use sea_orm::{ActiveModelTrait, ConnectionTrait, DbErr, EntityTrait, IntoActiveModel, NotSet};
use sea_orm::{ColumnTrait, QueryFilter};

use serde::Deserialize;

#[derive(Deserialize, PartialEq, Debug, Clone)]
enum Unit {
    Meters,
    Feet,
}
#[derive(Deserialize)]
struct ApiRes {
    data: RoundFromApi,
}

#[derive(Deserialize, PartialEq, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
struct Layout {
    #[serde(rename = "Detail")]
    holes: Vec<Hole>,
    length: u32,
    #[serde(rename = "Units")]
    unit: Unit,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct Hole {
    pub par: u32,
    #[serde(rename = "HoleOrdinal")]
    pub hole_number: u32,
    pub length: Option<u32>,
}
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, PartialEq)]
pub enum RoundStatus {
    Pending,
    Started,
    Finished,
    DNF,
}
#[derive(Debug, PartialEq)]
pub struct PlayerScore {
    pub pdga_number: u32,
    pub hole_scores: Vec<u8>,
    pub throws: u8,
    pub round_to_par: i16,
    pub placement: u16,
    pub started: RoundStatus,
    pub division: Division,
    pub(crate) name: String,
    pub(crate) first_name: String,
    pub(crate) last_name: String,
    pub(crate) avatar: Option<String>,
}

impl PlayerScore {
    pub async fn update_and_save(
        &self,
        db: &impl ConnectionTrait,
        round: i32,
        competition_id: i32,
        div: &Division,
    ) -> Result<(), GenericError> {
        if let Some(score_update) = self
            .round_score_active_model(db, round, competition_id, div.into())
            .await
        {
            score_update
                .save(db)
                .await
                .map_err(|_| GenericError::UnknownError("unable to save score to database"))?;
        }
        self.make_sure_player_in_competition(db, competition_id, div.into())
            .await?;
        Ok(())
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
    async fn round_score_active_model(
        &self,
        db: &impl ConnectionTrait,
        round: i32,
        competition_id: i32,
        division: entity::sea_orm_active_enums::Division,
    ) -> Option<ActiveModel> {
        let existing_score = player_round_score::Entity::find()
            .filter(player_round_score::Column::PdgaNumber.eq(self.pdga_number))
            .filter(player_round_score::Column::Round.eq(round))
            .one(db)
            .await
            .map_err(|e| {
                dbg!(&e);
                e
            });

        match existing_score {
            Ok(Some(score)) => {
                if score.throws != self.throws as i32 {
                    let mut score = score.into_active_model();
                    score.throws = Set(self.throws as i32);
                    Some(score)
                } else {
                    None
                }
            }
            Err(_) | Ok(None) => Some(ActiveModel {
                id: NotSet,
                pdga_number: Set(self.pdga_number as i32),
                competition_id: Set(competition_id),
                round: Set(round),
                throws: Set(self.throws as i32),
                division: Set(division),
                placement: Set(self.placement as i32),
            }),
        }
    }

    async fn make_sure_player_in_competition(
        &self,
        db: &impl ConnectionTrait,
        competition_id: i32,
        div: &entity::sea_orm_active_enums::Division,
    ) -> Result<(), GenericError> {
        entity::player_in_competition::Entity::insert(entity::player_in_competition::ActiveModel {
            id: NotSet,
            pdga_number: Set(self.pdga_number as i32),
            competition_id: Set(competition_id),
            division: Set(div.clone()),
        })
        .on_conflict(
            sea_query::OnConflict::columns(vec![
                entity::player_in_competition::Column::PdgaNumber,
                entity::player_in_competition::Column::CompetitionId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .do_nothing()
        .exec(db)
        .await
        .map_err(|_| GenericError::UnknownError("Unable to add player in competition"))?;

        Ok(())
    }

    fn get_user_score(&self, level: CompetitionLevel) -> u8 {
        ((match self.placement {
            1 => 100,
            2 => 85,
            3 => 75,
            4 => 69,
            5 => 64,
            6 => 60,
            7 => 57,
            8..=20 => 54 - (self.placement - 8) * 2,
            21..=48 => 50 - self.placement,
            49..=50 => 2,
            _ => 0,
        } as f32)
            * level.multiplier())
        .round() as u8
    }

    pub(crate) async fn get_user_fantasy_score(
        &self,
        db: &impl ConnectionTrait,
        fantasy_tournament_id: u32,
        competition_id: u32,
    ) -> Result<Option<UserScore>, GenericError> {
        let competition_level = entity::competition::Entity::find()
            .filter(entity::competition::Column::Id.eq(competition_id as i32))
            .one(db)
            .await
            .unwrap()
            .unwrap()
            .level
            .into();
        let score = self.get_user_score(competition_level) as i32;
        if score > 0 {
            if let Ok(Some(user)) = self.get_user(db, fantasy_tournament_id).await {
                Ok(Some(UserScore {
                    user: user.id,
                    score,
                    competition_id,
                    pdga_num: self.pdga_number,
                    fantasy_tournament_id,
                }))
            } else {
                //                dbg!(self);
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    async fn get_user(
        &self,
        db: &impl ConnectionTrait,
        fantasy_id: u32,
    ) -> Result<Option<user::Model>, GenericError> {
        if let Some(pick) = FantasyPick::find()
            .filter(
                fantasy_pick::Column::Player.eq(self.pdga_number).and(
                    fantasy_pick::Column::FantasyTournamentId
                        .eq(fantasy_id)
                        .and(fantasy_pick::Column::Benched.eq(false)),
                ),
            )
            .one(db)
            .await
            .map_err(|_| {
                GenericError::UnknownError("Pick not found due to unknown database error")
            })?
        {
            pick.find_related(User).one(db).await.map_err(|_| {
                GenericError::UnknownError("User not found due to unknown database error")
            })
        } else {
            Ok(None)
        }
    }
}

#[derive(Deserialize)]
struct RoundFromApi {
    layouts: Vec<Layout>,
    scores: Vec<ApiPlayer>,
    #[serde(skip)]
    div: Division,
}

fn fix_length(length: u32, unit: &Unit) -> u32 {
    match unit {
        Unit::Feet => (length as f64 * 0.3048).round() as u32,
        Unit::Meters => length,
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
}

impl RoundInformation {
    pub async fn new(
        competition_id: usize,
        round: usize,
        given_divs: Vec<Division>,
    ) -> Result<Self, GenericError> {
        let mut divs: Vec<RoundFromApi> = vec![];
        for div in given_divs {
            let new_div = Self::get_one_div(competition_id, round, div).await?;
            divs.push(new_div);
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
                        .into_iter()
                        .map(|p| {
                            let p: PlayerScore = p.into();
                            p
                        })
                        .collect_vec()
                })
                .collect_vec();
            Ok(Self::make_self(
                player_scores,
                layout,
                competition_id,
                round,
                divisions,
            ))
        } else {
            Err(GenericError::NotFound(
                "No round found containing any divisions supported by Rustling Chains",
            ))
        }
    }

    fn make_self(
        player_scores: Vec<PlayerScore>,
        layout: Layout,
        competition_id: usize,
        round_number: usize,
        divs: Vec<Division>,
    ) -> Self {
        let holes = layout
            .holes
            .iter()
            .map(|h| Hole {
                par: h.par,
                hole_number: h.hole_number,
                length: h.length.map(|l| fix_length(l, &layout.unit)),
            })
            .collect_vec();

        let length = match layout.unit {
            Unit::Feet => (layout.length as f64 * 0.3048).round() as u32,
            Unit::Meters => (layout.length as f64).round() as u32,
        };

        RoundInformation {
            holes,
            players: player_scores
                .into_iter()
                .filter_map(|p| {
                    let p: PlayerScore = p;
                    if p.started != RoundStatus::DNF {
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
        let mut resp: ApiRes = reqwest::get(url)
            .await
            .map_err(|_| {
                GenericError::UnknownError("Internal error while fetching round from PDGA")
            })?
            .json()
            .await
            .map_err(|_| {

                GenericError::UnknownError(
                    "Internal error while converting PDGA round to internal format",
                )
            })?;

        resp.data.div = div;
        Ok(resp.data)
    }

    // TODO: Refactor to do one mass update instead of many small
    pub async fn update_all(&self, db: &impl ConnectionTrait) -> Result<(), GenericError> {
        for player in &self.players {
            player
                .update_and_save(
                    db,
                    self.round_number as i32,
                    self.competition_id as i32,
                    &player.division,
                )
                .await?;
        }
        Ok(())
    }

    pub async fn all_player_scores_exist_in_db(
        &self,
        db: &impl ConnectionTrait,
    ) -> Result<bool, DbErr> {
        player_round_score::Entity::find()
            .filter(player_round_score::Column::Round.eq(self.round_number as i32))
            .all(db)
            .await
            .map(|x| x.len() == self.players.len())
    }

    pub(crate) fn status(&self) -> RoundStatus {
        let mut players = self
            .players
            .iter()
            .filter(|p| p.started != RoundStatus::DNF);
        if players.all(|p| p.started == RoundStatus::Finished) {
            RoundStatus::Finished
        } else if players.any(|p| p.started == RoundStatus::Started) {
            RoundStatus::Started
        } else {
            RoundStatus::Pending
        }
    }

    pub fn combine(&mut self, other: Self) {
        self.divs.extend(other.divs);
        self.divs.dedup();
        self.players.extend(other.players);
    }
}
