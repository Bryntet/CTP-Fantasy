use rocket::serde::json::Json;
use rocket::State;

use sea_orm::RuntimeErr::SqlxError;
use sea_orm::{DatabaseConnection, DbErr};

use crate::authenticate;
use crate::error;
use crate::error::{TournamentError, UserError};
use error::GenericError;
use rocket_okapi::openapi;

use rocket::http::CookieJar;
use sea_orm::TransactionTrait;

use service::dto::FantasyPick;
use service::dto::UserLogin;

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
#[post("/fantasy-tournament", format = "json", data = "<tournament>")]
pub(crate) async fn create_tournament(
    tournament: Json<service::dto::CreateTournament>,
    db: &State<DatabaseConnection>,
    user: authenticate::CookieAuth,
) -> Result<(), GenericError> {
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
                Err(UserError::InvalidUserId("Your user id seems to be invalid?").into())
            } else if msg.contains("violates unique constraint") {
                Err(TournamentError::TournamentNameConflict("Username already taken").into())
            } else {
                Err(GenericError::UnknownError("Unknown error").into())
            }
        }
        Err(e) => {
            dbg!(e);
            Err(GenericError::UnknownError("Unknown error").into())
        }
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
    user: Json<UserLogin>,
    db: &State<DatabaseConnection>,
    cookies: &CookieJar<'_>,
) -> Result<String, GenericError> {
    let res = user.0.insert(db, cookies).await;
    match res {
        Ok(()) => Ok("Successfully created user".to_string()),
        Err(DbErr::Query(SqlxError(sqlx::Error::Database(error)))) => {
            let msg = error.message();
            if msg.contains("violates unique constraint") {
                Err(UserError::UsernameConflict("Username already taken").into())
            } else {
                Err(GenericError::UnknownError("Unknown error"))
            }
        }
        Err(_) => Err(GenericError::UnknownError("Unknown error")),
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
#[openapi(tag = "Fantasy Tournament")]
#[put("/fantasy-tournament/<fantasy_tournament_id>/pick/<slot>/player/<pdga_number>")]
pub(crate) async fn add_pick(
    user: authenticate::CookieAuth,
    db: &State<DatabaseConnection>,
    fantasy_tournament_id: i32,
    slot: i32,
    pdga_number: i32,
) -> Result<String, GenericError> {
    let db = db.inner();
    let user = user.to_user_model(db).await?;
    let pick = FantasyPick { slot, pdga_number, name: None};
    // TODO: ADD AUTH FOR FANTASY_TOURNAMENT_ID
    pick.change_or_insert(db, user.id, fantasy_tournament_id)
        .await?;
    Ok("Successfully added pick".to_string())
}

#[openapi(tag = "Fantasy Tournament")]
#[post(
    "/fantasy-tournament/<fantasy_tournament_id>/pick",
    format = "json",
    data = "<picks>"
)]
pub(crate) async fn add_picks(
    user: authenticate::CookieAuth,
    db: &State<DatabaseConnection>,
    fantasy_tournament_id: i32,
    picks: Json<Vec<FantasyPick>>,
) -> Result<String, GenericError> {
    let user = user.to_user_model(db).await?;

    let txn = db.inner().begin().await?;

    for pick in picks.into_inner() {
        pick.change_or_insert(&txn, user.id, fantasy_tournament_id)
            .await?;
    }
    txn.commit().await?;
    Ok("Successfully added picks".to_string())
}

#[openapi(tag = "Fantasy Tournament")]
#[post("/fantasy-tournament/<fantasy_tournament_id>/invite/<invited_user>")]
pub(crate) async fn invite_user(
    user: authenticate::CookieAuth,
    db: &State<DatabaseConnection>,
    fantasy_tournament_id: i32,
    invited_user: String,
) -> Result<String, GenericError> {
    let user = user.to_user_model(db).await?;
    match service::create_invite(db, user, invited_user, fantasy_tournament_id).await {
        Ok(_) => Ok("Successfully invited user".to_string()),
        Err(e) => Err(e.into()),
    }
}

#[openapi(tag = "Fantasy Tournament")]
#[post("/fantasy-tournament/<fantasy_tournament_id>/answer-invite/<accepted>")]
pub(crate) async fn answer_invite(
    user: authenticate::CookieAuth,
    db: &State<DatabaseConnection>,
    fantasy_tournament_id: i32,
    accepted: bool,
) -> Result<String, GenericError> {
    let user = user.to_user_model(db).await?;
    match service::answer_invite(db, user, fantasy_tournament_id, accepted).await {
        Ok(()) => Ok("Successfully answered invite".to_string()),
        Err(e) => Err(e.into()),
    }
}
