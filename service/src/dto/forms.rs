use super::CompetitionLevel;
use super::{schemars, JsonSchema};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, JsonSchema, Serialize, Deserialize)]
pub struct AddCompetition {
    pub competition_id: u32,
    pub level: CompetitionLevel,
}
