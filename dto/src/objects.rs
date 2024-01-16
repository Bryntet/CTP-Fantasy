use entity::*;
use rocket::serde::{Deserialize, Serialize};
use rocket_okapi::okapi::schemars::{self, JsonSchema};


#[derive(Deserialize, Serialize, JsonSchema, Debug)]
#[serde(crate = "rocket::serde")]
pub struct FantasyPick {
    slot: i32,
    pdga_number: i32,
    fantasy_tournament_id: i32,
}

