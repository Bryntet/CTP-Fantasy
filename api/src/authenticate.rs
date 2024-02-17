use entity::prelude::User;
use entity::{user, user_cookies};

use rocket::http::{Cookie, CookieJar, Status};
use rocket::outcome::{IntoOutcome, Outcome};
use rocket::serde::json::Json;
use rocket::{
    get,
    request::{self, FromRequest},
    Request, State,
};

use rocket_okapi::{openapi, request::OpenApiFromRequest};
use sea_orm::{ConnectionTrait, DatabaseConnection, EntityTrait, ModelTrait, TransactionTrait};

use crate::error;
use crate::error::{GenericError, UserError};
use error::AuthError;
use sea_orm::ColumnTrait;
use sea_orm::QueryFilter;
use user::Model as UserModel;
use user_cookies::Model as CookieModel;
#[derive(OpenApiFromRequest, Debug)]
pub struct UserAuthentication(String);

#[derive(OpenApiFromRequest, Debug)]
pub struct TournamentOwner {
    pub user: UserAuthentication,
    pub tournament_id: u32,
}

#[derive(OpenApiFromRequest, Debug)]
pub struct AllowedToExchangeGuard(bool);

impl AllowedToExchangeGuard {
    pub fn is_allowed(&self) -> bool {
        self.0
    }

    pub async fn is_move_allowed(&self, db: &impl ConnectionTrait, tournament_id: i32) -> bool {
        if self.0 {
            true
        } else {
            service::query::has_exchange_begun(db, tournament_id)
                .await
                .unwrap_or(false)
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AllowedToExchangeGuard {
    type Error = GenericError;

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let db = request
            .rocket()
            .state::<DatabaseConnection>()
            .expect("Database not found");
        let user = request.guard::<UserAuthentication>().await;
        let tournament_id = request.param::<u32>(1);
        if !user.is_success() {
            Outcome::Error((
                Status::Unauthorized,
                Self::Error::NotFound("You are not authorized to do this for this user."),
            ))
        } else if let (Some(user), Some(Ok(tournament_id))) = (user.succeeded(), tournament_id) {
            let user = user.get_user(db).await;

            if let Ok(Some(user)) = user {
                let user = user.id;
                match service::query::is_user_allowed_to_exchange(db, user, tournament_id as i32)
                    .await
                {
                    Ok(allowed) => Outcome::Success(AllowedToExchangeGuard(allowed)),
                    Err(e) => Outcome::Error((Status::InternalServerError, e)),
                }
            } else {
                Outcome::Error((Status::NoContent, Self::Error::NotFound("User not found")))
            }
        } else {
            Outcome::Error((
                Status::NoContent,
                Self::Error::NotFound("Tournament id not found"),
            ))
        }
    }
}

impl TournamentOwner {
    async fn is_authenticated(&self, db: &DatabaseConnection) -> bool {
        if let Ok(Some(c)) = self.user.get_cookie(db).await {
            c.user_id == self.get_owner_id(db).await.unwrap_or(-1)
        } else {
            false
        }
    }

    async fn get_owner_id(&self, db: &DatabaseConnection) -> Result<i32, GenericError> {
        entity::fantasy_tournament::Entity::find_by_id(self.tournament_id as i32)
            .one(db)
            .await
            .map_err(|_| GenericError::UnknownError("Unable to find tournament by id"))?
            .map(|c| c.owner)
            .ok_or(UserError::InvalidUserId("User not found").into())
    }
}
#[rocket::async_trait]
impl<'r> FromRequest<'r> for TournamentOwner {
    type Error = AuthError;

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let db = request
            .rocket()
            .state::<DatabaseConnection>()
            .expect("Database not found");
        let cookie: request::Outcome<UserAuthentication, Json<Self::Error>> =
            request.guard::<UserAuthentication>().await;

        let t: Self = if let Some(cookie) = cookie.succeeded() {
            if let Some(Ok(t_id)) = request.param::<u32>(1) {
                Self {
                    user: cookie,
                    tournament_id: t_id,
                }
            } else {
                return None.or_error((
                    Status::BadRequest,
                    AuthError::Invalid("Invalid tournament id"),
                ));
            }
        } else {
            return None.or_error((Status::Unauthorized, AuthError::Missing("No cookie found")));
        };

        t.is_authenticated(db)
            .await
            .then_some(t)
            .or_error((Status::Unauthorized, AuthError::WrongPassword("")))
    }
}

impl UserAuthentication {
    async fn get_cookie(
        &self,
        db: &DatabaseConnection,
    ) -> Result<Option<CookieModel>, GenericError> {
        entity::prelude::UserCookies::find_by_id(self.0.to_owned())
            .one(db)
            .await
            .map_err(|_| GenericError::UnknownError("db error while finding cookie"))
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
        cookies.remove_private("auth");
    }

    pub async fn remove_cookie(
        self,
        db: &DatabaseConnection,
        cookies: &CookieJar<'_>,
    ) -> Result<&'static str, GenericError> {
        if let Ok(Some(cookie)) = self.get_cookie(db).await {
            cookie
                .delete(db)
                .await
                .map_err(|_| GenericError::UnknownError("Error while trying to delete cookie"))?;
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
            let txn = db
                .begin()
                .await
                .map_err(|_| GenericError::UnknownError("Unable to begin txn"))?;
            for cookie in user
                .find_related(user_cookies::Entity)
                .all(&txn)
                .await
                .map_err(|_| GenericError::UnknownError("Error while trying to find cookie"))?
            {
                cookie
                    .delete(&txn)
                    .await
                    .map_err(|_| GenericError::UnknownError("Unable to delete cookie."))?;
            }
            txn.commit()
                .await
                .map_err(|_| GenericError::UnknownError("Unable to commit txn"))?;
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
impl<'a> FromRequest<'a> for UserAuthentication {
    type Error = Json<AuthError>;
    async fn from_request(request: &'a Request<'_>) -> request::Outcome<Self, Self::Error> {
        let db = request
            .rocket()
            .state::<DatabaseConnection>()
            .expect("Database not found");

        let cookie: Cookie = if let Some(cookie) = request.cookies().get_private("auth") {
            cookie
        } else {
            return None.or_error((
                Status::Unauthorized,
                AuthError::Missing("No cookie found").into(),
            ));
        };

        UserAuthentication::new_checked(cookie.value().to_string(), db)
            .await
            .or_error((
                Status::Forbidden,
                AuthError::WrongPassword("You do not have permission to do that").into(),
            ))
    }
}

/*impl<'a> OpenApiFromRequest<'a> for CookieAuth {
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
}*/

#[openapi(tag = "User")]
#[get("/check-cookie")]
pub async fn check_cookie(_user_cookie: UserAuthentication) -> &'static str {
    "Authenticated"
}

#[openapi(tag = "User")]
#[get("/check-cookie", rank = 2)]
pub fn check_cookie_failed() -> GenericError {
    AuthError::Invalid("Cookie is invalid").into()
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
    login_data: Json<service::dto::LoginInput>,
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
    user: UserAuthentication,
) -> Result<&'static str, GenericError> {
    user.remove_cookie(db.inner(), cookies).await?;

    Ok("Successfully logged out")
}

#[openapi(tag = "User")]
#[post("/logout-all")]
pub(crate) async fn logout_all(
    db: &State<DatabaseConnection>,
    cookies: &CookieJar<'_>,
    user: UserAuthentication,
) -> Result<&'static str, GenericError> {
    user.remove_all_cookies(db.inner(), cookies).await?;

    Ok("Successfully logged out")
}
