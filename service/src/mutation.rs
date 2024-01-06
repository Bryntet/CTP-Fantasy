use bcrypt::{hash, DEFAULT_COST};
use cookie::Cookie;
use entity::prelude::*;
use entity::*;
use fantasy_tournament::Entity as FantasyTournament;
use rand::distributions::Alphanumeric;
use rand::Rng;
use sea_orm::ActiveValue::*;
use sea_orm::{DatabaseConnection, DbErr, EntityTrait, TransactionTrait};
use serde::Deserialize;

use rocket_okapi::okapi::schemars;
use rocket_okapi::okapi::schemars::JsonSchema;
#[derive(Deserialize, JsonSchema)]
pub struct CreateTournamentInput {
    pub name: String,
    pub max_picks_per_user: Option<i32>,
}

impl CreateTournamentInput {
    pub fn into_active_model(self, owner_id: i32) -> fantasy_tournament::ActiveModel {
        fantasy_tournament::ActiveModel {
            id: NotSet,
            name: Set(self.name),
            owner: Set(owner_id),
            max_picks_per_user: match self.max_picks_per_user {
                Some(v) => Set(v),
                None => NotSet,
            },
        }
    }
    pub async fn insert(
        self,
        db: &DatabaseConnection,
        owner_id: i32,
    ) -> Result<(), sea_orm::error::DbErr> {
        FantasyTournament::insert(self.into_active_model(owner_id))
            .exec(db)
            .await?;
        Ok(())
    }
}

/*pub struct CreatePickInput {
    pub user: i32,
    pub player: i32,
    pub fantasy_tournament_id: i32,
    pub
}

impl CreatePickInput {
    pub fn into_active_model(self) -> fantasy_pick::ActiveModel {
        fantasy_pick::ActiveModel {
            id: NotSet,
            user: Set(self.user),
            player: Set(self.player),
            fantasy_tournament_id: Set(self.fantasy_tournament_id),
            create
        }
    }
    pub async fn insert(self, db: &DatabaseConnection) -> Result<(), sea_orm::error::DbErr> {
        FantasyPick::insert(self.into_active_model())
            .exec(db)
            .await?;
        Ok(())
    }
}*/

pub struct CreateUserScoreInput {
    pub user: i32,
    pub score: i32,
    pub ranking: i32,
    pub fantasy_tournament_id: i32,
}

impl CreateUserScoreInput {
    pub fn into_active_model(self) -> fantasy_scores::ActiveModel {
        fantasy_scores::ActiveModel {
            id: NotSet,
            user: Set(self.user),
            score: Set(self.score),
            ranking: Set(self.ranking),
            fantasy_tournament_id: Set(self.fantasy_tournament_id),
        }
    }
    pub async fn insert(self, db: &DatabaseConnection) -> Result<(), sea_orm::error::DbErr> {
        FantasyScores::insert(self.into_active_model())
            .exec(db)
            .await?;
        Ok(())
    }
}

#[derive(Deserialize, JsonSchema, Debug)]
pub struct CreateUserInput {
    pub username: String,
    pub password: String,
}

impl CreateUserInput {
    fn active_user(&self) -> user::ActiveModel {
        user::ActiveModel {
            id: NotSet,
            name: Set(self.username.clone()),
        }
    }
    fn active_authentication(
        &self,
        hashed_password: String,
        user_id: i32,
    ) -> user_authentication::ActiveModel {
        user_authentication::ActiveModel {
            user_id: Set(user_id),
            hashed_password: Set(hashed_password),
        }
    }
    pub async fn insert(self, db: &DatabaseConnection) -> Result<Cookie, sea_orm::error::DbErr> {
        dbg!(&self);
        let txn = db.begin().await?;
        let user = self.active_user();
        let user_id = User::insert(user).exec(&txn).await?.last_insert_id;
        let hashed_password = hash(&self.password, DEFAULT_COST).unwrap();
        let authentication = self.active_authentication(hashed_password, user_id);
        UserAuthentication::insert(authentication)
            .exec(&txn)
            .await?;
        txn.commit().await?;
        generate_cookie(db, user_id).await
    }
}

pub async fn generate_cookie(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<Cookie<'static>, DbErr> {
    let random_value: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect();

    let cookie = Cookie::build(("auth", random_value.clone()))
        .secure(true)
        .build();

    let user_cookie = user_cookies::ActiveModel {
        user_id: Set(user_id),
        cookie: Set(random_value),
    };

    UserCookies::insert(user_cookie).exec(db).await?;

    Ok(cookie)
}
