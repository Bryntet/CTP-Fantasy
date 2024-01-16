
use entity::prelude::User;
use entity::{user, user_cookies};
use rocket::http::{CookieJar, Status};
use rocket::outcome::IntoOutcome;
use rocket::serde::json::Json;
use rocket::{
    get,
    request::{self, FromRequest},
    Request, State,
};
use rocket_okapi::okapi::openapi3::{Object, Parameter, ParameterValue};
use rocket_okapi::{
    gen::OpenApiGenerator,
    openapi,
    request::{OpenApiFromRequest, RequestHeaderInput},
};
use sea_orm::{DatabaseConnection, DbErr, EntityTrait, ModelTrait, TransactionTrait};

use crate::error;
use crate::error::{GenericError, UserError};
use error::AuthError;
use sea_orm::ColumnTrait;
use sea_orm::QueryFilter;
use user::Model as UserModel;
use user_cookies::Model as CookieModel;
pub struct CookieAuth(String);

impl CookieAuth {
    async fn get_cookie(&self, db: &DatabaseConnection) -> Result<Option<CookieModel>, DbErr> {
        entity::prelude::UserCookies::find_by_id(self.0.to_owned())
            .one(db)
            .await
    }

    pub(crate) async fn get_user(
        &self,
        db: &DatabaseConnection,
    ) -> Result<Option<UserModel>, GenericError> {
        let cookie = self.get_cookie(db).await?;
        if let Some(cookie) = cookie {
            return User::find_by_id(cookie.user_id)
                .one(db)
                .await
                .map_err(|_| UserError::InvalidUserId("User not found").into());
        }
        Err(AuthError::Invalid("You do not have permission to do this.").into())
    }

    pub async fn to_user_model(&self, db: &DatabaseConnection) -> Result<UserModel, GenericError> {
        if let Ok(Some(user)) = self.get_user(db).await {
            Ok(user)
        } else {
            Err(AuthError::Invalid("You do not have permission to do this.").into())
        }
    }

    fn remove_from_jar(cookies: &CookieJar<'_>) {
        cookies.remove(rocket::http::Cookie::named("auth"));
    }

    pub async fn remove_cookie(
        self,
        db: &DatabaseConnection,
        cookies: &CookieJar<'_>,
    ) -> Result<&'static str, GenericError> {
        if let Ok(Some(cookie)) = self.get_cookie(db).await {
            cookie.delete(db).await?;
            Self::remove_from_jar(cookies);
        }
        Ok("Successfully logged out")
    }

    pub async fn remove_all_cookies(
        self,
        db: &DatabaseConnection,
        cookies: &CookieJar<'_>,
    ) -> Result<&'static str, GenericError> {
        if let Ok(Some(user)) = self.get_user(db).await {
            let txn = db.begin().await?;
            for cookie in user.find_related(user_cookies::Entity).all(&txn).await? {
                cookie.delete(&txn).await?;
            }
            txn.commit().await?;
            Self::remove_from_jar(cookies);
        }

        Ok("Successfully logged out")
    }

    async fn is_valid(&self, db: &DatabaseConnection) -> bool {
        if let Ok(c) = self.get_cookie(db).await {
            c.is_some()
        } else {
            false
        }
    }
    pub async fn new_checked(cookie: String, db: &DatabaseConnection) -> Option<Self> {
        let cookie = Self(cookie);
        if cookie.is_valid(db).await {
            Some(cookie)
        } else {
            None
        }
    }
}

// Implement the actual checks for the authentication
#[rocket::async_trait]
impl<'a> FromRequest<'a> for CookieAuth {
    type Error = AuthError;
    async fn from_request(request: &'a Request<'_>) -> request::Outcome<Self, Self::Error> {
        request
            .cookies()
            .get("auth")
            .and_then(|cookie| cookie.value().parse().ok())
            .map(CookieAuth)
            .or_forward(Status::Unauthorized)
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
pub async fn check_cookie(
    db: &State<DatabaseConnection>,
    user_cookie: CookieAuth,
) -> Result<&'static str, GenericError> {
    match user_cookie.is_valid(db.inner()).await {
        true => Ok("Cookie is valid"),
        false => Err(AuthError::Invalid("Cookie is invalid").into()),
    }
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
) -> Result<String, GenericError> {
    let login_data = login_data.into_inner();
    let auth_result = service::query::authenticate(
        db.inner(),
        login_data.username.clone(),
        service::query::Auth::Password(login_data.password),
    )
    .await;
    let generic_error_response = "Wrong username or password";

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
                Ok(None) | Err(_) => Err(AuthError::Invalid(generic_error_response).into()),
            }
        }
        Ok(false) => Err(AuthError::WrongPassword(generic_error_response).into()),
        Err(_) => Err(AuthError::UnknownError(generic_error_response).into()),
    }
}

#[openapi(tag = "User")]
#[post("/logout")]
pub(crate) async fn logout(
    db: &State<DatabaseConnection>,
    cookies: &CookieJar<'_>,
    user: CookieAuth,
) -> Result<&'static str, GenericError> {
    user.remove_cookie(db.inner(), cookies).await?;

    Ok("Successfully logged out")
}

#[openapi(tag = "User")]
#[post("/logout-all")]
pub(crate) async fn logout_all(
    db: &State<DatabaseConnection>,
    cookies: &CookieJar<'_>,
    user: CookieAuth,
) -> Result<&'static str, GenericError> {
    user.remove_all_cookies(db.inner(), cookies).await?;

    Ok("Successfully logged out")
}
