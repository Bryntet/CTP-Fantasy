#[macro_use]
extern crate rocket;

use std::fmt::Debug;
use dotenvy::dotenv;
use rocket::http::Status;
use rocket::response::Responder;
use rocket::serde::json::Json;
use rocket::{response, response::status, Request, Response, State};
use sea_orm::{DatabaseConnection, DbErr, RuntimeErr};
use sea_orm::RuntimeErr::SqlxError;
use sqlx::postgres::PgDatabaseError;
enum Error {
    TooManyTournaments,
    InvalidUserId,
    TournamentNameConflict,
    Other(Box<dyn Debug>),
}

impl Error {
    fn to_rocket_status(&self) -> Status {
        match self {
            Self::InvalidUserId => Status::BadRequest,
            Self::Other(e) => {
                dbg!(e);
                Status::InternalServerError
            },
            Self::TooManyTournaments => Status::Forbidden,
            Self::TournamentNameConflict => Status::Conflict,
        }
    }
    fn to_err_message(&self) -> Option<String> {
        match self {
            Self::InvalidUserId => Some("Invalid User Id".to_string()),
            Self::TooManyTournaments => Some("User has reached max amount of tournaments".to_string()),
            Self::TournamentNameConflict => Some("Tournament name already used".to_string()),
            Self::Other(_) => None,
        }
    }
    fn other<T: Debug + 'static>(value: T) -> Self {
        Self::Other(Box::new(value))
    }
}

impl<'r> Responder<'r, 'static> for Error {
    fn respond_to(self, _: &'r Request<'_>) -> response::Result<'static> {
        let mut builder = Response::build();
        builder.status(self.to_rocket_status());
        if let Some(msg) = self.to_err_message() {
            builder.sized_body(msg.len(), std::io::Cursor::new(msg));
        }
        builder.ok()
    }
}

#[post("/tournaments", format = "json", data = "<tournament>")]
async fn create_tournament(
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
                Err(Error::other(""))
            }
        }
        Err(e) => {Err(Error::other(e))}
    }
}

#[post("/competition")]


#[launch]
async fn rocket() -> _ {
    dotenv().ok();
    let db = sea_orm::Database::connect(std::env::var("DATABASE_URL").expect("DATABASE_URL not set"))
        .await
        .unwrap();
    rocket::build()
        .manage(db)
        .mount("/", routes![create_tournament])
}
