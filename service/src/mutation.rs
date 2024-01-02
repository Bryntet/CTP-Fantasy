use entity::prelude::*;
use entity::*;
use fantasy_tournament::Entity as FantasyTournament;
use sea_orm::ActiveValue::*;
use sea_orm::{DatabaseConnection, EntityTrait};
use serde::Deserialize;

#[derive(Deserialize)]
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
