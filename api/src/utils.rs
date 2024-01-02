use std::fmt::Debug;
use rocket::http::Status;
use rocket::{Request, response, Response};
use rocket::response::Responder;
use serde::{Serialize, Deserialize};
use rocket_okapi::{openapi_get_routes, rapidoc::*, swagger_ui::*};
use rocket_okapi::gen::OpenApiGenerator;
use rocket_okapi::okapi::openapi3::Responses;
use rocket_okapi::okapi::schemars;
use rocket_okapi::okapi::schemars::{JsonSchema, Map};
use rocket_okapi::response::{OpenApiResponder, OpenApiResponderInner};

#[derive(Serialize, Deserialize, JsonSchema)]
pub(crate) enum Error {
    TooManyTournaments,
    InvalidUserId,
    TournamentNameConflict,
    UnknownCompetition,
    Other(String),
}




impl Error {
    pub(crate) fn to_rocket_status(&self) -> Status {
        match self {
            Self::InvalidUserId => Status::BadRequest,
            Self::Other(e) => {
                dbg!(e);
                Status::InternalServerError
            },
            Self::TooManyTournaments => Status::Forbidden,
            Self::TournamentNameConflict => Status::Conflict,
            Self::UnknownCompetition => Status::NotFound,
        }
    }
    pub(crate) fn to_err_message(&self) -> Option<String> {
        match self {
            Self::InvalidUserId => Some("Invalid User Id".to_string()),
            Self::TooManyTournaments => Some("User has reached max amount of tournaments".to_string()),
            Self::TournamentNameConflict => Some("Tournament name already used".to_string()),
            Self::UnknownCompetition => Some("Unknown competition".to_string()),
            Self::Other(_) => None,
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