use rocket::serde::json::serde_json::json;
use rocket::serde::json::Json;
use rocket::State;

use rocket_okapi::okapi::schemars;
use rocket_okapi::okapi::schemars::JsonSchema;
use sea_orm::RuntimeErr::SqlxError;
use sea_orm::{DatabaseConnection, DbErr};

use crate::authenticate;
use crate::error;
use crate::error::UserError;
use error::Error;
use rocket_okapi::openapi;
use serde::Deserialize;

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

#[openapi(tag = "User")]
#[post("/create-user", format = "json", data = "<user>")]
pub(crate) async fn create_user(
    user: Json<service::CreateUserInput>,
    db: &State<DatabaseConnection>,
) -> Result<String, Error> {
    let res = user.into_inner().insert(db.inner()).await;
    match res {
        Ok(e) => Ok(e.to_string()),
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
