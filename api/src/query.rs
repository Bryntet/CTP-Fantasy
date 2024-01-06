use rocket::http::{Cookie, CookieJar};
use crate::error::Error;
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
                    let cookie_result =
                        service::mutation::generate_cookie(db.inner(), user.id).await;
                    cookies.add(cookie_result?);
                    Ok("Successfully logged in".to_string())
                }
                Ok(None) => Err(UserError::InvalidUserId.into()),
                Err(_) => Err(Error::Other("Failed to find user".to_string())),
            }
        }
        Ok(false) => Err(Error::Other("Invalid password".to_string())),
        Err(_) => Err(Error::Other("Failed to authenticate".to_string())),
    }
}


#[derive(serde::Serialize, serde::Deserialize, JsonSchema, Debug)]
pub struct SimpleFantasyTournament {
    id: i32,
    name: String,
}
#[openapi(tag = "Fantasy Tournament")]
#[get("/my-tournaments")]
pub(crate) async fn see_tournaments(
    db: &State<DatabaseConnection>,
    user: authenticate::CookieAuth,
) -> Result<Json<Vec<SimpleFantasyTournament>>, Error> {
    let user_model = user.to_user_model(db.inner()).await?;
    match service::get_fantasy_tournaments(db.inner(), user_model.id).await {

        Ok(tournaments) => {
            let tours = tournaments.iter().map(|x| SimpleFantasyTournament{id: x.id, name: x.name.to_string()}).collect();
            dbg!(&tours);
            Ok(Json(tours))
        },
        Err(_) => Err(Error::Other("Failed to get tournaments".to_string())),
    }
}