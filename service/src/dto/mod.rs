use chrono::{FixedOffset, NaiveDate};
use itertools::Itertools;
use rocket::request::FromParam;
use rocket::serde::{Deserialize, Serialize};
use rocket::FromFormField;
use rocket_okapi::okapi::schemars::JsonSchema;
use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::ConnectionTrait;
use std::fmt::Debug;
use strum_macros::EnumIter;

use entity::*;
pub use pdga::{CompetitionInfo, RoundInformation};
pub use scoring_visualisation::{user_competition_scores, AttributeCompetitionScores, CompetitionScores};

use crate::error::GenericError;

pub mod forms;
mod mutation;
mod pdga;
mod player_trading;
mod query;
mod scoring_visualisation;
mod user_attribute;
pub use pdga::RoundLabel;
pub use user_attribute::{AttributeName, UserDataCombination};
pub mod traits {
    pub use super::mutation::InsertCompetition;
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub enum ExchangeWindowStatus {
    AllowedToExchange,
    AllowedToReorder {
        window_opens_at: chrono::DateTime<FixedOffset>,
    },
    Closed,
}
impl ExchangeWindowStatus {
    pub async fn new(db: &impl ConnectionTrait, user_id: u32, tournament: u32) -> Result<Self, GenericError> {
        let allowed_to_exchange =
            super::exchange_windows::is_user_allowed_to_exchange(db, user_id as i32, tournament as i32)
                .await?;
        if allowed_to_exchange {
            Ok(Self::AllowedToExchange)
        } else if !super::exchange_windows::any_competitions_running(db, tournament as i32).await? {
            if let Some(time) = super::exchange_windows::see_when_users_can_exchange(db, tournament as i32)
                .await?
                .into_iter()
                .find(|(user, _)| user.user.id == user_id as i32)
                .map(|(_, time)| time)
            {
                Ok(Self::AllowedToReorder {
                    window_opens_at: time,
                })
            } else {
                unreachable!()
            }
        } else {
            Ok(Self::Closed)
        }
    }
}

pub struct PhantomCompetition {
    name: String,
    competition_id: Option<u32>,
    start_date: NaiveDate,
}
#[derive(Serialize, Deserialize, JsonSchema, Clone)]
pub struct CreateTournament {
    pub name: String,
    pub max_picks_per_user: Option<i32>,
    pub divisions: Vec<Division>,
    pub amount_in_bench: Option<i32>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub struct Competition {
    pub name: String,
    pub competition_id: i32,
    pub level: CompetitionLevel,
    pub start_date: NaiveDate,
}
impl Competition {
    pub async fn all_in_fantasy_tournament(
        db: &impl ConnectionTrait,
        tournament_id: i32,
    ) -> Result<Vec<Self>, GenericError> {
        Ok(super::get_competitions_in_fantasy_tournament(db, tournament_id)
            .await?
            .into_iter()
            .sorted_by(|a, b| a.start_date.cmp(&b.start_date))
            .map(|c| Self {
                level: c.level.into(),
                name: c.name,
                competition_id: c.id,
                start_date: c.start_date,
            })
            .collect())
    }
}

pub use player_trading::{FantasyPick, FantasyPicks, PlayerTradesLog};

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
pub struct UserLogin {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize, JsonSchema, Debug, Clone)]
pub struct UserScore {
    pub user: i32,
    pub score: i32,
    pub competition_id: u32,
    pub pdga_num: u32,
    pub fantasy_tournament_id: u32,
    // Required for later filtering of scores
    pub benched: bool,
    pub slot: u8,
    pub division: Division,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub enum InvitationStatus {
    Accepted,
    Pending,
    Declined,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Hash, Eq)]
pub struct User {
    pub id: i32,
    pub username: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub struct UserWithScore {
    pub user: User,
    pub score: i32,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub struct UserWithPicks {
    pub user: User,
    pub picks: Vec<FantasyPick>,
}

#[derive(Deserialize, JsonSchema, Debug)]
pub struct LoginInput {
    pub username: String,
    pub password: String,
}

#[derive(
    Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, FromFormField, EnumIter, Default, Copy,
)]
pub enum Division {
    MPO,
    FPO,
    #[default]
    #[serde(other)]
    Unknown,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub struct FantasyTournamentDivs {
    pub(crate) divisions: Vec<Division>,
}

#[derive(Deserialize, JsonSchema, Debug)]
pub struct PlayerInCompetition {
    pub pdga_number: i32,
    pub division: Division,
    pub competition_id: i32,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, FromFormField)]
pub enum CompetitionLevel {
    Major,
    Playoff,
    ElitePlus,
    Elite,
    Silver,
}

impl From<sea_orm_active_enums::CompetitionLevel> for CompetitionLevel {
    fn from(level: sea_orm_active_enums::CompetitionLevel) -> Self {
        match level {
            sea_orm_active_enums::CompetitionLevel::Major => CompetitionLevel::Major,
            sea_orm_active_enums::CompetitionLevel::Playoff => CompetitionLevel::Playoff,
            sea_orm_active_enums::CompetitionLevel::ElitePlus => CompetitionLevel::ElitePlus,
            sea_orm_active_enums::CompetitionLevel::Elite => CompetitionLevel::Elite,
            sea_orm_active_enums::CompetitionLevel::Silver => CompetitionLevel::Silver,
            _ => panic!("Invalid competition level"),
        }
    }
}

impl FromParam<'_> for CompetitionLevel {
    type Error = GenericError;

    fn from_param(param: &'_ str) -> Result<Self, Self::Error> {
        serde_json::from_str(param).map_err(|_| GenericError::BadRequest("Invalid competition level"))
    }
}
