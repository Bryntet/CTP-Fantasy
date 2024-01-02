use entity::prelude::*;
use entity::*;
use fantasy_tournament::Entity as FantasyTournament;
use sea_orm::ActiveValue::*;
use sea_orm::{DatabaseConnection, EntityTrait};
use serde::Deserialize;
use bcrypt::{DEFAULT_COST, hash};

use rocket_okapi::okapi::schemars;
use rocket_okapi::okapi::schemars::JsonSchema;
#[derive(Deserialize, JsonSchema)]
pub struct CreateTournamentInput {
    pub owner: i32,
    pub max_picks_per_user: Option<i32>,
}

impl CreateTournamentInput {
    pub fn into_active_model(self) -> fantasy_tournament::ActiveModel {
        fantasy_tournament::ActiveModel {
            id: NotSet,
            owner: Set(self.owner),
            max_picks_per_user: match self.max_picks_per_user {
                Some(v) => Set(v),
                None => NotSet,
            },
        }
    }
    pub async fn insert(self, db: &DatabaseConnection) -> Result<(), sea_orm::error::DbErr> {
        FantasyTournament::insert(self.into_active_model())
            .exec(db)
            .await?;
        Ok(())
    }
}

pub struct CreatePickInput {
    pub user: i32,
    pub player: i32,
    pub fantasy_tournament_id: i32,
}

impl CreatePickInput {
    pub fn into_active_model(self) -> fantasy_pick::ActiveModel {
        fantasy_pick::ActiveModel {
            id: NotSet,
            user: Set(self.user),
            player: Set(self.player),
            fantasy_tournament_id: Set(self.fantasy_tournament_id),
        }
    }
    pub async fn insert(self, db: &DatabaseConnection) -> Result<(), sea_orm::error::DbErr> {
        FantasyPick::insert(self.into_active_model())
            .exec(db)
            .await?;
        Ok(())
    }
}

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

#[derive(Deserialize, JsonSchema)]
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
    fn active_authentication(&self, hashed_password: String, user_id: i32) -> user_authentication::ActiveModel {
        user_authentication::ActiveModel {
            id: NotSet,
            user_id: Set(user_id),
            hashed_password: Set(hashed_password),
        }
    }
    pub async fn insert(self, db: &DatabaseConnection) -> Result<(), sea_orm::error::DbErr> {
        let user = self.active_user();
        let user_id = User::insert(user).exec(db).await?.last_insert_id;
        let hashed_password = hash(&self.password, DEFAULT_COST).unwrap();
        let authentication = self.active_authentication(hashed_password, user_id);
        UserAuthentication::insert(authentication).exec(db).await?;
        Ok(())
    }
}
