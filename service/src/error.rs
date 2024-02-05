use rocket::response::Responder;
use rocket::serde::{Deserialize, Serialize};
use rocket_okapi::gen::OpenApiGenerator;
use rocket_okapi::okapi::openapi3::Responses;
use rocket_okapi::okapi::schemars;
use rocket_okapi::okapi::schemars::{JsonSchema, Map};
use rocket_okapi::response::OpenApiResponderInner;
use std::fmt::{Debug};

#[derive(Serialize, Deserialize, JsonSchema, Debug, Responder)]
pub enum GenericError {
    #[response(status = 404)]
    UnknownCompetition(&'static str),
    #[response(status = 500)]
    UnknownError(&'static str),
    TournamentError(TournamentError),
    UserError(UserError),
    CookieError(AuthError),
    AuthError(AuthError),
    #[response(status = 403)]
    ViolatesForeignKey(&'static str),
    #[response(status = 409)]
    UniqueError(&'static str),
    #[response(status = 422)]
    CheckError(&'static str),
    #[response(status = 404)]
    NotFound(&'static str),
    #[response(status = 409)]
    Conflict(&'static str),
    #[response(status = 400)]
    BadRequest(&'static str),
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Responder)]
pub enum TournamentError {
    #[response(status = 403)]
    TooManyTournaments(&'static str),
    #[response(status = 409)]
    TournamentNameConflict(&'static str),
    #[response(status = 403)]
    NotPermitted(&'static str),
    #[response(status = 404)]
    NotFound(&'static str),
}

impl From<TournamentError> for GenericError {
    fn from(e: TournamentError) -> Self {
        Self::TournamentError(e)
    }
}

impl From<AuthError> for GenericError {
    fn from(e: AuthError) -> Self {
        Self::CookieError(e)
    }
}
impl From<UserError> for GenericError {
    fn from(e: UserError) -> Self {
        Self::UserError(e)
    }
}
impl From<PlayerError> for GenericError {
    fn from(e: PlayerError) -> Self {
        match e {
            PlayerError::NotFound => Self::NotFound("Player not found in tournament"),
            PlayerError::WrongDivision => Self::NotFound("Player not in specified division"),
        }
    }
}

#[derive(Debug, JsonSchema, Deserialize, Serialize, Responder)]
pub enum AuthError {
    #[response(status = 401)]
    Missing(&'static str),
    #[response(status = 403)]
    Invalid(&'static str),
    #[response(status = 403)]
    WrongPassword(&'static str),
    #[response(status = 403)]
    UnknownError(&'static str),
}

#[derive(Debug, JsonSchema, Deserialize, Serialize, Responder)]
pub enum UserError {
    #[response(status = 409)]
    UsernameConflict(&'static str),
    #[response(status = 404)]
    InvalidUserId(&'static str),
    #[response(status = 403)]
    NotPermitted(&'static str),
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub enum PlayerError {
    NotFound,
    WrongDivision,
}

pub enum InviteError {
    TournamentNotFound,
    UserNotFound,
    NotOwner,
}
impl From<InviteError> for GenericError {
    fn from(e: InviteError) -> Self {
        use InviteError::*;
        match e {
            TournamentNotFound => {
                GenericError::TournamentError(TournamentError::NotFound("Tournament not found"))
            }
            UserNotFound => GenericError::UserError(UserError::InvalidUserId("User not found")),
            NotOwner => GenericError::TournamentError(TournamentError::NotPermitted(
                "You are not the owner of this tournament",
            )),
        }
    }
}

pub struct ResultResponder(Result<(), GenericError>);

impl OpenApiResponderInner for GenericError {
    fn responses(_: &mut OpenApiGenerator) -> rocket_okapi::Result<Responses> {
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
