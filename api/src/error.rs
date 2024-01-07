use rocket::http::Status;
use rocket::response::Responder;
use rocket::{response, Request, Response};
use rocket_okapi::gen::OpenApiGenerator;
use rocket_okapi::okapi::openapi3::Responses;
use rocket_okapi::okapi::schemars;
use rocket_okapi::okapi::schemars::{JsonSchema, Map};
use rocket_okapi::response::{OpenApiResponder, OpenApiResponderInner};
use rocket_okapi::{openapi_get_routes, rapidoc::*, swagger_ui::*};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub enum Error {
    TournamentError(TournamentError),
    UnknownCompetition,
    CookieError(AuthError),
    UserError(UserError),
    PlayerError(PlayerError),
    UnknownError,
}

impl MyRocketError for Error {
    fn to_rocket_status(&self) -> Status {
        match self {
            Self::TournamentError(e) => e.to_rocket_status(),
            Self::UnknownCompetition => Status::NotFound,
            Self::CookieError(e) => e.to_rocket_status(),
            Self::UserError(e) => e.to_rocket_status(),
            Self::PlayerError(e) => e.to_rocket_status(),
            Self::UnknownError => Status::InternalServerError,
        }
    }
    fn to_err_message(&self) -> Option<String> {
        match self {
            Self::TournamentError(e) => e.to_err_message(),
            Self::UnknownCompetition => Some("Unknown competition".to_string()),
            Self::CookieError(e) => e.to_err_message(),
            Self::UserError(e) => e.to_err_message(),
            Self::PlayerError(e) => e.to_err_message(),
            Self::UnknownError => Some("Unknown error".to_string()),
        }
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

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub(crate) enum TournamentError {
    TooManyTournaments,
    TournamentNameConflict,
    NotPermitted,
    NotFound
}

trait MyRocketError {
    fn to_rocket_status(&self) -> Status;
    fn to_err_message(&self) -> Option<String>;
}

impl From<TournamentError> for Error {
    fn from(e: TournamentError) -> Self {
        Self::TournamentError(e)
    }
}

impl From<AuthError> for Error {
    fn from(e: AuthError) -> Self {
        Self::CookieError(e)
    }
}

impl MyRocketError for TournamentError {
    fn to_rocket_status(&self) -> Status {
        match self {
            Self::TooManyTournaments => Status::Forbidden,
            Self::TournamentNameConflict => Status::Conflict,
            Self::NotPermitted => Status::Forbidden,
            Self::NotFound => Status::NotFound
        }
    }
    fn to_err_message(&self) -> Option<String> {
        match self {
            Self::TooManyTournaments => {
                Some("User has reached max amount of tournaments".to_string())
            }
            Self::TournamentNameConflict => Some("Tournament name already used".to_string()),
            Self::NotPermitted => Some("You do not have permission to do this".to_string()),
            Self::NotFound => Some("Tournament not found".to_string())
        }
    }
}

#[derive(Debug, JsonSchema, Deserialize, Serialize)]
pub enum AuthError {
    Missing,
    Invalid,
    WrongPassword,
    UnknownError
}

impl MyRocketError for AuthError {
    fn to_rocket_status(&self) -> Status {
        match self {
            Self::Missing => Status::Unauthorized,
            Self::Invalid => Status::Forbidden,
            Self::WrongPassword => Status::Forbidden,
            Self::UnknownError => Status::InternalServerError
        }
    }

    fn to_err_message(&self) -> Option<String> {
        match self {
            Self::Missing => Some("Missing auth cookie".to_string()),
            Self::Invalid => Some("Invalid auth cookie".to_string()),
            Self::WrongPassword => Some("Wrong password".to_string()),
            Self::UnknownError => Some("Unknown error".to_string())
        }
    }
}

#[derive(Debug, JsonSchema, Deserialize, Serialize)]
pub enum UserError {
    UsernameConflict,
    InvalidUserId,
    NotPermitted,
}

impl MyRocketError for UserError {
    fn to_rocket_status(&self) -> Status {
        match self {
            Self::UsernameConflict => Status::Conflict,
            Self::InvalidUserId => Status::NotFound,
            Self::NotPermitted => Status::Forbidden,
        }
    }

    fn to_err_message(&self) -> Option<String> {
        match self {
            Self::UsernameConflict => Some("Username already taken".to_string()),
            Self::InvalidUserId => Some("Invalid user".to_string()),
            Self::NotPermitted => Some("You do not have permission to do this".to_string()),
        }
    }
}

impl From<UserError> for Error {
    fn from(e: UserError) -> Self {
        Self::UserError(e)
    }
}

impl From<sea_orm::DbErr> for Error {
    fn from(e: sea_orm::DbErr) -> Self {
        dbg!(e);
        Self::UnknownError
    }
}
#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub enum PlayerError {
    PlayerNotFound,
}
impl MyRocketError for PlayerError {
    fn to_rocket_status(&self) -> Status {
        match self {
            Self::PlayerNotFound => Status::NotFound,
        }
    }
    fn to_err_message(&self) -> Option<String> {
        match self {
            Self::PlayerNotFound => Some("Player not found".to_string()),
        }
    }
}

impl From<PlayerError> for Error {
    fn from(e: PlayerError) -> Self {
        Self::PlayerError(e)
    }
}

pub struct ResultResponder(Result<(), Error>);

impl OpenApiResponderInner for Error {
    fn responses(gen: &mut OpenApiGenerator) -> rocket_okapi::Result<Responses> {
        use rocket_okapi::okapi::openapi3::{RefOr, Response as OpenApiResponse};

        let mut responses = Map::new();
        responses.insert(
            "400".to_string(),
            RefOr::Object(OpenApiResponse {
                description: "\
                # [400 Bad Request](https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/400)\n\
                The request given is wrongly formatted or data asked could not be fulfilled. \
                "
                .to_string(),
                ..Default::default()
            }),
        );
        responses.insert(
            "404".to_string(),
            RefOr::Object(OpenApiResponse {
                description: "\
                # [404 Not Found](https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/404)\n\
                This response is given when you request a page that does not exists.\
                "
                .to_string(),
                ..Default::default()
            }),
        );
        responses.insert(
            "409".to_string(),
            RefOr::Object(OpenApiResponse {
                description: "\
                # [409 Conflict](https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/409)\n\
                This response is given when you try to create a resource that already exists. \
                "
                .to_string(),
                ..Default::default()
            }),
        );
        responses.insert(
            "422".to_string(),
            RefOr::Object(OpenApiResponse {
                description: "\
                # [422 Unprocessable Entity](https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/422)\n\
                This response is given when you request body is not correctly formatted. \
                ".to_string(),
                ..Default::default()
            }),
        );
        responses.insert(
            "500".to_string(),
            RefOr::Object(OpenApiResponse {
                description: "\
                # [500 Internal Server Error](https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/500)\n\
                This response is given when something wend wrong on the server. \
                ".to_string(),
                ..Default::default()
            }),
        );
        Ok(Responses {
            responses,
            ..Default::default()
        })
    }
}
