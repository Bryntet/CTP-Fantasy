//! ------ Just Cookies (for just 1 route/endpoint) ------

use rocket::http::Status;
use rocket::outcome::IntoOutcome;
use rocket::response::Responder;
use rocket::serde::json::Json;
use rocket::{
    get,
    request::{self, FromRequest},
    response, Request, Response, State,
};
use rocket_okapi::okapi::openapi3::{Object, Parameter, ParameterValue, Responses};
use rocket_okapi::okapi::schemars;
use rocket_okapi::okapi::schemars::{JsonSchema, Map};
use rocket_okapi::response::OpenApiResponderInner;
use rocket_okapi::{
    gen::OpenApiGenerator,
    openapi,
    request::{OpenApiFromRequest, RequestHeaderInput},
    OpenApiError,
};
use sea_orm::{DatabaseConnection, DbErr, EntityTrait};

use crate::error;
use error::CookieAuthError;

pub struct CookieAuth(String);

impl CookieAuth {
    pub async fn to_user_model(
        self,
        db: &DatabaseConnection,
    ) -> Result<entity::user::Model, error::Error> {
        dbg!(&self.0);
        if let Ok(Some(x)) = entity::prelude::UserCookies::find_by_id(self.0)
            .one(db)
            .await
        {
            dbg!(&x);
            if let Ok(Some(x)) = entity::prelude::User::find_by_id(x.user_id).one(db).await {
                return Ok(x);
            }
        }
        Err(CookieAuthError::Invalid.into())
    }
}

// Implement the actual checks for the authentication
#[rocket::async_trait]
impl<'a> FromRequest<'a> for CookieAuth {
    type Error = error::Error;
    async fn from_request(
        request: &'a request::Request<'_>,
    ) -> request::Outcome<Self, Self::Error> {
        request
            .cookies()
            .get("auth")
            .and_then(|cookie| cookie.value().parse().ok())
            .map(CookieAuth)
            .or_forward(())
    }
}

impl<'a> OpenApiFromRequest<'a> for CookieAuth {
    fn from_request_input(
        gen: &mut OpenApiGenerator,
        _name: String,
        required: bool,
    ) -> rocket_okapi::Result<RequestHeaderInput> {
        let schema = gen.json_schema::<String>();
        Ok(RequestHeaderInput::Parameter(Parameter {
            name: "auth".to_owned(),
            location: "cookie".to_owned(),
            description: Some("Authentication cookie".to_owned()),
            required,
            deprecated: false,
            allow_empty_value: false,
            value: ParameterValue::Schema {
                style: None,
                explode: None,
                allow_reserved: false,
                schema,
                example: None,
                examples: None,
            },
            extensions: Object::default(),
        }))
    }
}
