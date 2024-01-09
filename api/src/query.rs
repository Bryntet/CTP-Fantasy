use rocket::http::{Cookie, CookieJar};
use crate::error::{Error, TournamentError, AuthError};
use crate::error::UserError;
use entity::prelude::User;
use entity::user;
use rocket::http::private::cookie;
use rocket::serde::json::serde_json::json;
use rocket::serde::json::Json;
use rocket::State;
use rocket_okapi::{openapi};
use sea_orm::ColumnTrait;
use sea_orm::QueryFilter;
use sea_orm::RuntimeErr::SqlxError;
use sea_orm::{DatabaseConnection, DbErr, EntityTrait};
use crate::authenticate;
use rocket_okapi::okapi::schemars;
use rocket_okapi::okapi::schemars::JsonSchema;

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



#[openapi(tag = "Fantasy Tournament")]
#[get("/my-tournaments")]
pub(crate) async fn see_tournaments(
    db: &State<DatabaseConnection>,
    user: authenticate::CookieAuth,
) -> Result<Json<Vec<service::SimpleFantasyTournament>>, Error> {
    let user_model = user.to_user_model(db.inner()).await?;
    match service::get_fantasy_tournaments(db.inner(), user_model.id).await {

        Ok(tournaments) => {
            Ok(Json(tournaments))
        },
        Err(_) => Err(TournamentError::NotFound.into()),
    }
}

#[openapi(tag = "Fantasy Tournament")]
#[get("/fantasy-tournament/<id>/participants")]
pub(crate) async fn see_participants(
    db: &State<DatabaseConnection>,
    id: i32,
) -> Result<Json<Vec<service::SimpleUser>>, Error> {
    match service::get_participants(db.inner(), id).await {
        Ok(participants) => Ok(Json(participants)),
        Err(_) => Err(UserError::InvalidUserId.into()),
    }
}



#[openapi(tag = "Fantasy Tournament")]
#[get("/fantasy-tournament/<tournament_id>/user_picks/<user_id>")]
pub(crate) async fn get_user_picks(
    db: &State<DatabaseConnection>,
    requester: authenticate::CookieAuth,
    tournament_id: i32,
    user_id: i32,
) -> Result<Json<service::SimpleFantasyPicks>, Error> {
    match service::get_user_picks_in_tournament(db.inner(), requester.to_user_model(db.inner()).await?, user_id, tournament_id).await {
        Ok(picks) => Ok(Json(picks)),
        Err(_) => Err(UserError::InvalidUserId.into()),
    }
}
