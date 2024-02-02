use super::CompetitionLevel;
use super::FromForm;
use super::{schemars, JsonSchema};
use rocket::request::FromParam;
use rocket::FromFormField;
use serde_derive::Deserialize;

#[derive(Debug, JsonSchema, Deserialize)]
pub struct AddCompetition {
    pub competition_id: u32,
    pub level: CompetitionLevel,
}
