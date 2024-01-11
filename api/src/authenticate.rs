//! ------ Just Cookies (for just 1 route/endpoint) ------

use rocket::http::{CookieJar, Status};
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
use sea_orm::{DatabaseConnection, DbErr, EntityTrait, ModelTrait, TransactionTrait};
use entity::prelude::{User};
use entity::{user,user_cookies};

use crate::error;
use error::AuthError;
use crate::error::{Error, UserError};
use sea_orm::ColumnTrait;
use sea_orm::QueryFilter;
use user_cookies::Model as CookieModel;
use user::Model as UserModel;
pub struct CookieAuth(String);

impl CookieAuth {

    async fn get_cookie(&self, db: &DatabaseConnection) -> Result<Option<CookieModel>, DbErr> {
        entity::prelude::UserCookies::find_by_id(self.0.to_owned()).one(db).await
    }

    pub(crate) async fn get_user(&self, db: &DatabaseConnection) -> Result<Option<UserModel>, Error> {
        let cookie = self.get_cookie(db).await?;
        if let Some(cookie) = cookie {
            return User::find_by_id(cookie.user_id).one(db).await.map_err(|_| UserError::InvalidUserId.into());
        }
        Err(AuthError::Invalid.into())
    }

    pub async fn to_user_model(
        &self,
        db: &DatabaseConnection,
    ) -> Result<UserModel, Error> {
        if let Ok(Some(user)) = self.get_user(db).await {
            return Ok(user);
        }
        Err(AuthError::Invalid.into())
    }

    fn remove_from_jar(cookies: &CookieJar<'_>) {
        cookies.remove(rocket::http::Cookie::named("auth"));
    }

    pub async fn remove_cookie(
        self,
        db: &DatabaseConnection,
        cookies: &CookieJar<'_>,
    ) -> Result<&'static str, Error> {
        if let Ok(Some(cookie)) = self.get_cookie(db).await
        {
            cookie.delete(db).await?;
            Self::remove_from_jar(cookies);
        }
        Ok("Successfully logged out")
    }

    pub async fn remove_all_cookies(
        self,
        db: &DatabaseConnection,
        cookies: &CookieJar<'_>,
    ) -> Result<&'static str, Error> {

        if let Ok(Some(user)) = self.get_user(db).await
        {
            let txn = db.begin().await?;
            for cookie in user.find_related(user_cookies::Entity).all(&txn).await? {
                cookie.delete(&txn).await?;
            }
            txn.commit().await?;
            Self::remove_from_jar(cookies);
        }

        Ok("Successfully logged out")
    }


}

// Implement the actual checks for the authentication
#[rocket::async_trait]
impl<'a> FromRequest<'a> for CookieAuth {
    type Error = AuthError;
    async fn from_request(
        request: &'a Request<'_>,
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



#[openapi(tag = "User")]
#[get("/check-cookie")]
pub(crate) async fn check_cookie(
    db: &State<DatabaseConnection>,
    user: CookieAuth,
) -> Result<&'static str, Error> {
    let a = user.to_user_model(db.inner()).await?;
    Ok("Cookie is valid")
}


/// # Login
///
/// # Parameters
///
/// - `username` - The username of the user
///
/// - `password` - The password of the user
///
/// # Returns
///
/// A cookie indicating success
#[openapi(tag = "User")]
#[post("/login", format = "json", data = "<login_data>")]
pub(crate) async fn login(
    login_data: Json<service::LoginInput>,
    db: &State<DatabaseConnection>,
    cookies: &CookieJar<'_>,
) -> Result<String, Error> {
    let login_data = login_data.into_inner();
    let auth_result = service::query::authenticate(
        db.inner(),
        login_data.username.clone(),
        service::query::Auth::Password(login_data.password),
    )
        .await;

    match auth_result {
        Ok(true) => {
            let user = User::find()
                .filter(user::Column::Name.eq(login_data.username))
                .one(db.inner())
                .await;
            match user {
                Ok(Some(user)) => {
                    service::generate_cookie(db.inner(), user.id, cookies).await?;
                    Ok("Successfully logged in".to_string())
                }
                Ok(None) => Err(UserError::InvalidUserId.into()),
                Err(_) => Err(AuthError::Invalid.into()),
            }
        }
        Ok(false) => Err(AuthError::WrongPassword.into()),
        Err(_) => Err(AuthError::UnknownError.into()),
    }
}


#[openapi(tag = "User")]
#[post("/logout")]
pub(crate) async fn logout(
    db: &State<DatabaseConnection>,
    cookies: &CookieJar<'_>,
    user: CookieAuth,
) -> Result<&'static str, Error> {
    user.remove_cookie(db.inner(), cookies).await?;

    Ok("Successfully logged out")
}


#[openapi(tag = "User")]
#[post("/logout-all")]
pub(crate) async fn logout_all(
    db: &State<DatabaseConnection>,
    cookies: &CookieJar<'_>,
    user: CookieAuth,
) -> Result<&'static str, Error> {
    user.remove_all_cookies(db.inner(), cookies).await?;

    Ok("Successfully logged out")
}
