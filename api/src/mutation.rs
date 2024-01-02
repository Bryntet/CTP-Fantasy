use crate::utils::Error;
use rocket::serde::json::serde_json::json;
use rocket::serde::json::Json;
use rocket::State;
use sea_orm::RuntimeErr::SqlxError;
use sea_orm::{DatabaseConnection, DbErr};

use rocket_okapi::openapi;

#[openapi(tag = "Fantasy Tournament")]
#[post("/create-fantasy-tournament", format = "json", data = "<tournament>")]
pub(crate) async fn create_tournament(
    tournament: Json<service::CreateTournamentInput>,
    db: &State<DatabaseConnection>,
) -> Result<(), Error> {
    let res = tournament.into_inner().insert(db.inner()).await;
    match res {
        Ok(_) => Ok(()),
        Err(DbErr::Query(SqlxError(sqlx::Error::Database(error)))) => {
            let msg = error.message();
            if msg.contains("violates foreign key constraint \"fantasy_tournament_owner_fkey\"") {
                Err(Error::InvalidUserId)
            } else if msg.contains("violates unique constraint") {
                Err(Error::TournamentNameConflict)
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
) -> Result<(), Error> {
    let res = user.into_inner().insert(db.inner()).await;
    match res {
        Ok(_) => Ok(()),
        Err(DbErr::Query(SqlxError(sqlx::Error::Database(error)))) => {
            let msg = error.message();
            if msg.contains("violates unique constraint") {
                Err(Error::UsernameConflict)
            } else {
                Err(Error::Other("".to_string()))
            }
        }
        Err(e) => Err(Error::Other(e.to_string())),
    }
}