mod mutation;
mod query;
use entity::*;
use rocket::serde::{Deserialize, Serialize};
use rocket_okapi::okapi::schemars::{self, JsonSchema};


#[derive(Deserialize, JsonSchema)]
pub struct CreateTournament {
    pub name: String,
    pub max_picks_per_user: Option<i32>,
}

#[derive(Deserialize, Serialize, JsonSchema, Debug)]
pub struct FantasyPick {
    pub slot: i32,
    pub pdga_number: i32,
    pub fantasy_tournament_id: i32,
}

#[derive(Deserialize, JsonSchema, Debug, Clone)]
pub struct UserLogin {
    pub username: String,
    pub password: String,
}


#[derive(Deserialize, JsonSchema, Debug)]
pub struct UserScore {
    pub user: i32,
    pub score: i32,
    pub ranking: i32,
    pub fantasy_tournament_id: i32,
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
    pub score: i32
}

#[derive(Deserialize, JsonSchema, Debug)]
pub struct LoginInput {
    pub username: String,
    pub password: String,
}

