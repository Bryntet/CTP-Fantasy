pub mod forms;
mod mutation;
mod pdga;
mod query;

use rocket::request::FromParam;
use rocket::FromFormField;

use entity::*;

use crate::error::GenericError;
pub use pdga::{CompetitionInfo, RoundInformation};
use rocket::serde::{Deserialize, Serialize};
use rocket_okapi::okapi::schemars::{self, JsonSchema};
use strum_macros::EnumIter;

pub mod traits {
    pub use super::mutation::InsertCompetition;
}
pub struct PhantomCompetition {
    name: String,
    competition_id: Option<u32>,
    start_date: chrono::NaiveDate,
}
#[derive(Serialize, Deserialize, JsonSchema, Clone)]
pub struct CreateTournament {
    pub name: String,
    pub max_picks_per_user: Option<i32>,
    pub divisions: Vec<Division>,
    pub amount_in_bench: Option<i32>,
}

#[derive(Deserialize, Serialize, JsonSchema, Debug)]
pub struct FantasyPick {
    pub slot: i32,
    pub pdga_number: i32,
    pub name: Option<String>,
    pub avatar: Option<String>,
    pub benched: bool,
}
#[derive(serde::Serialize, serde::Deserialize, JsonSchema, Debug)]
pub struct FantasyPicks {
    pub picks: Vec<FantasyPick>,
    pub(crate) owner: bool,
    pub(crate) fantasy_tournament_id: i32,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
pub struct UserLogin {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize, JsonSchema, Debug)]
pub struct UserScore {
    pub user: i32,
    pub score: i32,
    pub competition_id: u32,
    pub pdga_num: u32,
    pub fantasy_tournament_id: u32,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub enum InvitationStatus {
    Accepted,
    Pending,
    Declined,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub struct User {
    pub id: i32,
    pub name: String,
    pub score: i32,
}

#[derive(Deserialize, JsonSchema, Debug)]
pub struct LoginInput {
    pub username: String,
    pub password: String,
}

#[derive(
    Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, FromFormField, EnumIter, Default,
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
    pub(crate) division77146s: Vec<Division>,
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
        serde_json::from_str(param)
            .map_err(|_| GenericError::BadRequest("Invalid competition level"))
    }
}
