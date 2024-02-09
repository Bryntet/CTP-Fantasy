
use rocket_okapi::okapi::schemars;
use rocket_okapi::okapi::schemars::JsonSchema;

use sea_orm::{sea_query, ConnectionTrait, EntityTrait};

use serde::Deserialize;

use crate::dto::pdga::player_scoring::{PlayerScore, RoundStatus};
use crate::dto::Division;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompetitionInfoInput {
    pub id: u32,
    pub division: Division,
}


#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct ApiPlayer {
    #[serde(rename = "PDGANum")]
    pub pdga_number: u32,
    pub first_name: String,
    pub last_name: String,
    pub name: String,
    #[serde(rename = "AvatarURL")]
    pub avatar: Option<String>,
    pub division: Division,
    #[serde(rename = "RoundtoPar")]
    pub round_to_par: i16,
    #[serde(deserialize_with = "bool_from_int", rename = "RoundStarted")]
    pub player_started_round: bool,
    #[serde(deserialize_with = "bool_from_int", rename = "Completed")]
    pub player_finished_round: bool,
    pub running_place: u16,
    #[serde(rename = "RoundScore", deserialize_with = "flexible_number")]
    pub throws: u8,
    pub hole_scores: Vec<String>,
}

mod serde_things {
    use serde::de::{Visitor};
    use serde::{de, Deserializer};
    use std::fmt;

    pub(super) fn flexible_number<'de, D>(deserializer: D) -> Result<u8, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct FlexibleNumberVisitor;

        impl<'de> Visitor<'de> for FlexibleNumberVisitor {
            type Value = u8;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a number or a string containing a number")
            }

            fn visit_i64<E>(self, value: i64) -> Result<u8, E>
            where
                E: de::Error,
            {
                Ok(value.try_into().unwrap_or(u8::MAX))
            }

            fn visit_u64<E>(self, value: u64) -> Result<u8, E>
            where
                E: de::Error,
            {
                Ok(value.try_into().unwrap_or(u8::MAX))
            }

            fn visit_str<E>(self, value: &str) -> Result<u8, E>
            where
                E: de::Error,
            {
                Ok(value.parse::<u8>().unwrap_or(u8::MAX))
            }
        }

        deserializer.deserialize_any(FlexibleNumberVisitor)
    }

    pub(super) fn bool_from_int<'de, D>(deserializer: D) -> Result<bool, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct BoolFromInt;

        impl<'de> Visitor<'de> for BoolFromInt {
            type Value = bool;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a number or a string containing a number")
            }

            fn visit_i64<E>(self, value: i64) -> Result<bool, E>
                where
                    E: de::Error,
            {
                Ok(value==1)
            }

            fn visit_u64<E>(self, value: u64) -> Result<bool, E>
                where
                    E: de::Error,
            {
                Ok(value==1)
            }
        }
        deserializer.deserialize_any(BoolFromInt)
    }
}

use crate::error::GenericError;
use serde_things::{bool_from_int, flexible_number};
use crate::dto::pdga::get_competition::CompetitionInfoResponse;

impl From<ApiPlayer> for PlayerScore {
    fn from(p: ApiPlayer) -> Self {
        let round_status = if p.player_started_round {
            if p.player_finished_round {
                RoundStatus::Finished
            } else {
                RoundStatus::Started
            }
        } else if p.player_finished_round || p.throws == u8::MAX {
            RoundStatus::DNF
        } else {
            RoundStatus::Pending
        };

        Self {
            pdga_number: p.pdga_number,
            throws: p.throws,
            round_to_par: p.round_to_par,
            hole_scores: p
                .hole_scores
                .iter()
                .filter_map(|s| s.parse::<u8>().ok())
                .collect(),
            placement: p.running_place,
            started: round_status,
            division: p.division,
            name: p.name,
            first_name: p.first_name,
            last_name: p.last_name,
            avatar: p.avatar.map(|s| "https://www.pdga.com".to_string() + &s),
        }
    }
}


impl PartialEq for ApiPlayer {
    fn eq(&self, other: &Self) -> bool {
        self.pdga_number == other.pdga_number
    }
}

pub async fn add_players(
    db: &impl ConnectionTrait,
    players: Vec<&PlayerScore>,
    fantasy_tournament_id: Option<i32>,
) -> Result<(), GenericError> {
    entity::player::Entity::insert_many(players.iter().map(|p| p.to_active_model()))
        .on_conflict(
            sea_query::OnConflict::column(entity::player::Column::PdgaNumber)
                .do_nothing()
                .to_owned(),
        )
        .do_nothing()
        .exec(db)
        .await
        .map_err(|_| GenericError::UnknownError("Unable to insert players into database"))?;
    if let Some(fantasy_tournament_id) = fantasy_tournament_id {
        entity::player_division_in_fantasy_tournament::Entity::insert_many(
            players
                .into_iter()
                .map(|p| p.to_division_active_model(fantasy_tournament_id)),
        )
        .on_conflict(
            sea_query::OnConflict::column(
                entity::player_division_in_fantasy_tournament::Column::PlayerPdgaNumber,
            )
            .do_nothing()
            .to_owned(),
        )
        .do_nothing()
        .exec(db)
        .await
        .map_err(|_| {
            GenericError::UnknownError("Unable to insert player divisions into fantasy tournament")
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::fs::File;
    use std::io::Read;

}
