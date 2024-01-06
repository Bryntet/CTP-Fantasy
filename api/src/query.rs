use crate::error::Error;
use crate::error::UserError;
use entity::prelude::User;
use entity::user;
use rocket::http::private::cookie;
use rocket::serde::json::serde_json::json;
use rocket::serde::json::Json;
use rocket::State;
use rocket_okapi::openapi;
use sea_orm::ColumnTrait;
use sea_orm::QueryFilter;
use sea_orm::RuntimeErr::SqlxError;
use sea_orm::{DatabaseConnection, DbErr, EntityTrait};


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
                    match cookie_result {
                        Ok(cookie) => Ok(cookie.to_string()),
                        Err(_) => Err(Error::Other("Failed to generate cookie".to_string())),
                    }
                }
                Ok(None) => Err(UserError::InvalidUserId.into()),
                Err(_) => Err(Error::Other("Failed to find user".to_string())),
            }
        }
        Ok(false) => Err(Error::Other("Invalid password".to_string())),
        Err(_) => Err(Error::Other("Failed to authenticate".to_string())),
    }
}
