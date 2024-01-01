use sea_orm::{DatabaseConnection, EntityTrait};
use sea_orm::ActiveValue::*;
use entity::*;
use fantasy_tournament::Entity as FantasyTournament;

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
                None => NotSet
            },
        }
    }
    pub async fn insert(self, db: &DatabaseConnection) -> Result<(), sea_orm::error::DbErr> {
        FantasyTournament::insert(self.into_active_model()).exec(db).await?;
        Ok(())
    }
}


