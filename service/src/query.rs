use bcrypt::verify;
use cookie::Cookie;
use entity::prelude::*;
use entity::prelude::*;
use entity::sea_orm_active_enums::Division;
use entity::*;
use entity::*;
use rand::distributions::Alphanumeric;
use rand::Rng;
use rocket_okapi::okapi::schemars;
use rocket_okapi::okapi::schemars::JsonSchema;
use sea_orm::entity::prelude::*;
use sea_orm::ActiveValue::*;
use sea_orm::{DatabaseConnection, DbErr, EntityTrait};
use serde::Deserialize;
pub async fn get_user_picks_for_tournament(
    db: &DatabaseConnection,
    user_id: i32,
    tournament_id: i32,
) -> Result<Vec<fantasy_pick::Model>, DbErr> {
    let picks = FantasyPick::find()
        .filter(fantasy_pick::Column::User.eq(user_id))
        .filter(fantasy_pick::Column::FantasyTournamentId.eq(tournament_id))
        .all(db)
        .await?;

    Ok(picks)
}

#[derive(Deserialize, JsonSchema)]
pub struct LoginInput {
    pub username: String,
    pub password: String,
}

pub enum Auth {
    Password(String),
    Cookie(String),
}
pub async fn authenticate(
    db: &DatabaseConnection,
    username: String,
    auth: Auth,
) -> Result<bool, DbErr> {
    let user = User::find()
        .filter(user::Column::Name.eq(username))
        .one(db)
        .await?;

    if let Some(user) = user {
        match auth {
            Auth::Password(password) => {
                let user_auth = UserAuthentication::find()
                    .filter(user_authentication::Column::UserId.eq(user.id))
                    .one(db)
                    .await?;
                if let Some(user_auth) = user_auth {
                    Ok(verify(&password, &user_auth.hashed_password).is_ok())
                } else {
                    Ok(false)
                }
            }
            Auth::Cookie(cookie_value) => {
                let user_cookie = UserCookies::find()
                    .filter(user_cookies::Column::UserId.eq(user.id))
                    .filter(user_cookies::Column::Cookie.eq(cookie_value))
                    .one(db)
                    .await?;
                Ok(user_cookie.is_some())
            }
        }
    } else {
        Ok(false)
    }
}
