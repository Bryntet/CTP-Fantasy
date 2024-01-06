use rocket::serde::json::serde_json::json;
use rocket::serde::json::Json;
use rocket::State;

use rocket_okapi::okapi::schemars;
use rocket_okapi::okapi::schemars::JsonSchema;
use sea_orm::RuntimeErr::SqlxError;
use sea_orm::{ActiveModelTrait, DatabaseConnection, DbErr, EntityTrait};

use crate::authenticate;
use crate::error;
use crate::error::UserError;
use error::Error;
use rocket_okapi::openapi;
use serde::Deserialize;


use rocket::http::Cookie;
use rocket::http::CookieJar;

/// # Create a fantasy tournament
///
/// # Parameters
///
/// - `name` - The name of the tournament
///
/// - `auth` Cookie - The cookie of the user creating the tournament
///
/// # Returns
///
/// A string indicating success
///
/// # Errors
///
/// - `UserError::InvalidUserId` - The user ID in the cookie is invalid
///
/// - `UserError::UsernameConflict` - The username is already taken
///
/// - `Error::Other` - An unknown error occurred
///
/// - `Error::PlayerError` - The player does not exist
///
/// - `Error::DbErr` - A database error occurred
///
/// - `Error::CookieAuthError` - The cookie is invalid
#[openapi(tag = "Fantasy Tournament")]
#[post("/create-fantasy-tournament", format = "json", data = "<tournament>")]
pub(crate) async fn create_tournament(
    tournament: Json<service::CreateTournamentInput>,
    db: &State<DatabaseConnection>,
    user: authenticate::CookieAuth,
) -> Result<(), Error> {
    dbg!("hi");

    let user_model = user.to_user_model(db.inner()).await?;
    let res = tournament
        .into_inner()
        .insert(db.inner(), user_model.id)
        .await;
    match res {
        Ok(_) => Ok(()),
        Err(DbErr::Query(SqlxError(sqlx::Error::Database(error)))) => {
            let msg = error.message();
            if msg.contains("violates foreign key constraint \"fantasy_tournament_owner_fkey\"") {
                Err(UserError::InvalidUserId.into())
            } else if msg.contains("violates unique constraint") {
                Err(UserError::UsernameConflict.into())
            } else {
                Err(Error::Other("".to_string()))
            }
        }
        Err(e) => Err(Error::Other(e.to_string())),
    }
}
/// # Create a user
///
/// # Parameters
///
/// - `username` - The username of the user to create
///
/// - `password` - The password of the user to create
#[openapi(tag = "User")]
#[post("/create-user", format = "json", data = "<user>")]
pub(crate) async fn create_user(
    user: Json<service::CreateUserInput>,
    db: &State<DatabaseConnection>,
    cookies: &CookieJar<'_>
) -> Result<&'static str, Error> {
    let res = user.into_inner().insert(db.inner()).await;
    match res {
        Ok(e) => {
            cookies.add(Cookie::new("auth", e.to_string()));
            Ok("Successfully created user")
        },
        Err(DbErr::Query(SqlxError(sqlx::Error::Database(error)))) => {
            let msg = error.message();
            if msg.contains("violates unique constraint") {
                Err(UserError::UsernameConflict.into())
            } else {
                Err(Error::Other("".to_string()))
            }
        }
        Err(e) => Err(Error::Other(e.to_string())),
    }
}


#[derive(Deserialize, JsonSchema, Debug)]
struct FantasyPick {
    slot: i32,
    pdga_number: i32,
    fantasy_tournament_id: i32
}

impl FantasyPick {
    async fn insert_or_change(&self, db: &DatabaseConnection, user_id: i32) -> Result<(), Error> {
        use sea_orm::{NotSet, Set, QueryFilter, ColumnTrait};
        use entity::prelude::FantasyPick as FantasyPickEntity;



        let existing_pick = FantasyPickEntity::find()
            .filter(entity::fantasy_pick::Column::PickNumber.eq(self.slot))
            .filter(entity::fantasy_pick::Column::User.eq(user_id))
            .filter(entity::fantasy_pick::Column::FantasyTournamentId.eq(self.fantasy_tournament_id))
            .one(db)
            .await?;


        if !service::player_exists(db, self.pdga_number).await {
            Err::<(), Error>(error::PlayerError::PlayerNotFound.into())?;
        }
        match existing_pick {
            Some(pick) => {
                let mut pick: entity::fantasy_pick::ActiveModel = pick.into();
                pick.player = Set(self.pdga_number);
                pick.update(db).await?;
            }
            None => {
                let new_pick = entity::fantasy_pick::ActiveModel {
                    id: NotSet,
                    user: Set(user_id),
                    pick_number: Set(self.slot),
                    player: Set(self.pdga_number),
                    fantasy_tournament_id: Set(self.fantasy_tournament_id),
                    division: Set(service::get_player_division(db, self.pdga_number).await?.first().unwrap().to_owned()),
                };
                new_pick.insert(db).await?;
            }
        }
        Ok(())
    }
}

/// # Add a pick to a fantasy tournament
///
/// # Parameters
///
/// - `slot` - The slot to add the pick to
///
/// - `pdga_number` - The PDGA number of the player to add
///
/// - `fantasy_tournament_id` - The ID of the fantasy tournament to add the pick to
///
/// # Returns
///
/// A string indicating success
#[openapi(tag="Fantasy Tournament")]
#[put("/fantasy-pick", format = "json", data = "<pick>")]
pub async fn add_pick(
    user: authenticate::CookieAuth,
    db: &State<DatabaseConnection>,
    pick: Json<FantasyPick>
) -> Result<String, Error> {
    let user = user.to_user_model(db).await?;
    let pick = pick.into_inner();
    pick.insert_or_change(db, user.id).await?;
    Ok("Successfully added pick".to_string())
}