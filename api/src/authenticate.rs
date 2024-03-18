use entity::prelude::User;
use entity::{user, user_cookies};

use rocket::http::{CookieJar, Status};
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
pub struct UserAuthentication(Authentication);

#[derive(OpenApiFromRequest, Debug)]
pub struct TournamentAuthentication {
    pub user: UserAuthentication,
    is_owner: bool,
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
            service::exchange_windows::has_exchange_begun(db, tournament_id)
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
            let user = user.to_user_model();

            match user {
                Ok(user) => {
                    let user = user.id;
                    match service::exchange_windows::is_user_allowed_to_exchange(
                        db,
                        user,
                        tournament_id as i32,
                    )
                    .await
                    {
                        Ok(allowed) => Outcome::Success(AllowedToExchangeGuard(allowed)),
                        Err(e) => Outcome::Error((Status::InternalServerError, e)),
                    }
                }
                Err(e) => Outcome::Error((Status::Unauthorized, e)),
            }
        } else {
            Outcome::Error((
                Status::NoContent,
                Self::Error::NotFound("Tournament id not found"),
            ))
        }
    }
}

#[derive(Debug)]
enum Authentication {
    Authenticated { cookie: CookieModel, user: UserModel },
    NoCookie,
    InvalidCookie,
}

impl Authentication {
    pub fn is_authenticated(&self, user_id: i32) -> bool {
        match self {
            Self::Authenticated { user, .. } => user.admin || user.id == user_id,
            _ => false,
        }
    }

    pub fn is_admin(&self) -> bool {
        match self {
            Self::Authenticated { user, .. } => user.admin,
            _ => false,
        }
    }
}

impl TournamentAuthentication {
    async fn new(
        user_auth: UserAuthentication,
        db: &DatabaseConnection,
        tournament_id: i32,
    ) -> Result<Self, GenericError> {
        Ok(Self {
            is_owner: Self::get_internal_authentication(&user_auth, db, tournament_id).await?,
            user: user_auth,
        })
    }

    pub fn assure_ownership(&self) -> Result<(), GenericError> {
        if self.is_owner || self.user.0.is_admin() {
            Ok(())
        } else {
            Err(AuthError::Invalid("You are not the owner of this tournament").into())
        }
    }

    pub async fn is_authenticated(&self) -> bool {
        self.user.0.is_admin() || matches!(self.user.0, Authentication::Authenticated { .. })
    }

    async fn get_internal_authentication(
        user: &UserAuthentication,
        db: &DatabaseConnection,
        tournament_id: i32,
    ) -> Result<bool, GenericError> {
        Ok(user.0.is_admin()
            | user
                .0
                .is_authenticated(Self::get_owner_id(db, tournament_id).await?))
    }

    async fn get_owner_id(db: &DatabaseConnection, tournament_id: i32) -> Result<i32, GenericError> {
        entity::fantasy_tournament::Entity::find_by_id(tournament_id)
            .one(db)
            .await
            .map_err(|_| GenericError::UnknownError("Unable to find tournament by id"))?
            .map(|c| c.owner)
            .ok_or(UserError::InvalidUserId("User not found").into())
    }
}
#[rocket::async_trait]
impl<'r> FromRequest<'r> for TournamentAuthentication {
    type Error = GenericError;

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let db = request
            .rocket()
            .state::<DatabaseConnection>()
            .expect("Database not found");
        let cookie: request::Outcome<UserAuthentication, _> = request.guard::<UserAuthentication>().await;

        if let Some(cookie) = cookie.succeeded() {
            if let Some(Ok(t_id)) = request.param::<i32>(1) {
                match TournamentAuthentication::new(cookie, db, t_id).await {
                    Ok(success) => Outcome::Success(success),
                    Err(e) => Outcome::Error((Status::BadRequest, e)),
                }
            } else {
                None.or_error((
                    Status::BadRequest,
                    AuthError::Invalid("Invalid tournament id").into(),
                ))
            }
        } else {
            None.or_error((Status::Unauthorized, AuthError::Missing("No cookie found").into()))
        }
    }
}

impl UserAuthentication {
    fn new_invalid_cookie() -> Self {
        Self(Authentication::InvalidCookie)
    }

    fn new_no_cookie() -> Self {
        Self(Authentication::NoCookie)
    }

    pub async fn new(db: &impl ConnectionTrait, cookie: &str) -> Result<Self, GenericError> {
        Ok(Self(Self::get_authentication(db, cookie).await?))
    }

    async fn get_db_cookie(db: &impl ConnectionTrait, cookie: &str) -> Result<CookieModel, GenericError> {
        entity::prelude::UserCookies::find_by_id(cookie)
            .one(db)
            .await
            .map_err(|_| GenericError::UnknownError("db error while finding cookie"))?
            .ok_or(AuthError::Invalid("Cookie not found").into())
    }

    async fn get_user_from_db(
        db: &impl ConnectionTrait,
        cookie: &CookieModel,
    ) -> Result<Option<UserModel>, GenericError> {
        cookie.find_related(user::Entity).one(db).await.map_err(|e| {
            error!("Error while trying to find user by cookie: {}", e);
            GenericError::UnknownError("Error while trying to find user by cookie")
        })
    }

    async fn get_authentication(
        db: &impl ConnectionTrait,
        cookie: &str,
    ) -> Result<Authentication, GenericError> {
        let cookie = Self::get_db_cookie(db, cookie).await?;
        if let Some(user) = Self::get_user_from_db(db, &cookie).await? {
            Ok(Authentication::Authenticated { cookie, user })
        } else {
            Ok(Authentication::InvalidCookie)
        }
    }

    pub fn assure_authorized(&self) -> Result<(), GenericError> {
        match self.0 {
            Authentication::Authenticated { .. } => Ok(()),
            _ => Err(AuthError::Missing("You are not authorized to do that").into()),
        }
    }

    pub fn assure_admin(&self) -> Result<(), GenericError> {
        if !self.0.is_admin() {
            Err(GenericError::NotPermitted("You are not authorized to do this"))
        } else {
            Ok(())
        }
    }

    pub fn to_user_model(&self) -> Result<&UserModel, GenericError> {
        match self.0 {
            Authentication::Authenticated { ref user, .. } => Ok(user),
            _ => Err(AuthError::Missing("No user found").into()),
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
        match self.0 {
            Authentication::Authenticated { cookie, .. } => {
                cookie
                    .delete(db)
                    .await
                    .map_err(|_| GenericError::UnknownError("Error while trying to delete cookie"))?;
                Self::remove_from_jar(cookies);
                Ok("Successfully logged out")
            }
            _ => Err(AuthError::Invalid("Cannot remove non-existing cookie").into()),
        }
    }

    pub async fn remove_all_cookies(
        self,
        db: &DatabaseConnection,
        cookies: &CookieJar<'_>,
    ) -> Result<&'static str, GenericError> {
        match self.0 {
            Authentication::Authenticated { user, .. } => {
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
                        .map_err(|_| GenericError::UnknownError("Unable to delete cookie"))?;
                }
                txn.commit()
                    .await
                    .map_err(|_| GenericError::UnknownError("Unable to commit txn"))?;
                Self::remove_from_jar(cookies);
                Ok("Successfully logged out")
            }
            _ => Err(AuthError::Invalid("Cannot remove non-existing cookie").into()),
        }
    }

    // Function that returns an auth error if the user is not authenticated
    pub async fn require_authentication(&self) -> Result<(), GenericError> {
        match self.0 {
            Authentication::Authenticated { .. } => Ok(()),
            _ => Err(AuthError::Invalid("You do not have permission to do that").into()),
        }
    }
}

// Implement the actual checks for the authentication
#[rocket::async_trait]
impl<'a> FromRequest<'a> for UserAuthentication {
    type Error = GenericError;
    async fn from_request(request: &'a Request<'_>) -> request::Outcome<Self, Self::Error> {
        let db = request
            .rocket()
            .state::<DatabaseConnection>()
            .expect("Database not found");

        if let Some(cookie) = request.cookies().get_private("auth") {
            match UserAuthentication::new(db, cookie.value()).await {
                Ok(auth) => Outcome::Success(auth),
                _ => Outcome::Success(UserAuthentication::new_invalid_cookie()),
            }
        } else {
            Outcome::Success(UserAuthentication::new_no_cookie())
        }
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
pub async fn check_cookie(auth: UserAuthentication) -> Result<&'static str, GenericError> {
    auth.assure_authorized()?;
    Ok("ELLO")
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
