use rocket::error;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, ModelTrait, QueryFilter};
use serde_derive::Serialize;
use super::User;
use crate::error::GenericError;
use rocket_okapi::okapi::schemars::{self, JsonSchema};




#[derive(Debug, Serialize, JsonSchema)]
struct Player {
    name: String,
    id: u32,
}

impl From<entity::player::Model> for Player {
    fn from(model: entity::player::Model) -> Self {
        Self {
            name: model.first_name + &model.last_name,
            id: model.pdga_number as u32,
        }
    }

}


#[derive(Debug, Serialize, JsonSchema)]
struct PlayerCompetitionScore {
    player: Player,
    score: u8,
}

impl PlayerCompetitionScore {
    async fn from_db(db: &impl ConnectionTrait, score_model: entity::user_competition_score_in_fantasy_tournament::Model) -> Result<Self, GenericError> {
        let player = Player::from(score_model.find_related(entity::player::Entity).one(db).await.map_err(|e| {
            error!("Unknown fatal error while getting player from db in competition score {:#?}", e);
            GenericError::UnknownError("Unknown internal db error while trying to get player from competition score")
        })?.ok_or({
            error!("Unable to find player in db in competition score");
            GenericError::UnknownError("Unable to find player in db in competition score")
        })?);
        Ok(Self {
            player,
            score: score_model.score as u8,
        })
    }
}


#[derive(Debug, Serialize, JsonSchema)]
pub struct CompetitionScore(Vec<PlayerCompetitionScore>);

impl CompetitionScore {
    pub async fn new(db: &impl ConnectionTrait, competition_id: i32, user_id: i32, tournament_id: i32) -> Result<Self, GenericError> {
        use entity::user_competition_score_in_fantasy_tournament as CompScore;
        use CompScore::Entity as CompScoreEnt;
        let score_models= CompScoreEnt::find()
            .filter(
                CompScore::Column::User.eq(user_id)
                    .and(CompScore::Column::CompetitionId.eq(competition_id))
                    .and(CompScore::Column::FantasyTournamentId.eq(tournament_id))
            )
            .all(db)
            .await
            .map_err(|e| {
                error!("Unable to get user scores from competition {:#?}", e);
                GenericError::UnknownError("Unable to get user scores from competition")
            })?;

        let mut scores = Vec::new();
        for score in score_models {
            scores.push(PlayerCompetitionScore::from_db(db, score).await?);
        }

        Ok(Self(scores))
    }
    
    pub fn total_score(&self) -> u32 {
        self.0.iter().map(|x| x.score as u32).sum()
    }
}
#[derive(Debug, Serialize,JsonSchema)]
pub struct UserWithCompetitionScore {
    pub user: User,
    pub competition_score: CompetitionScore,
    pub total_score: u32,
}



impl From<entity::user::Model> for User {
    fn from(model: entity::user::Model) -> Self {
        Self {
            id: model.id,
            username: model.name,
        }
    }
}



impl UserWithCompetitionScore {
    async fn new(db: &impl ConnectionTrait, user: entity::user::Model, tournament_id: i32, competition_id: i32) -> Result<Self, GenericError> {
        let user = User::from(user);
        let competition_score = CompetitionScore::new(db, competition_id, user.id, tournament_id).await?;

        Ok(Self {
            user,
            total_score: competition_score.total_score(),
            competition_score,
        })
    }
}


pub async fn user_competition_scores(db: &impl ConnectionTrait, tournament_id: i32, competition_id: i32) -> Result<Vec<UserWithCompetitionScore>, GenericError> {
    use entity::user::Entity as UserEnt;
    let user_models = UserEnt::find().all(db).await.map_err(|e| {
        error!("Unable to get users from db {:#?}", e);
        GenericError::UnknownError("Unable to get users from db")
    })?;
    let mut users = Vec::new();
    for user in user_models {
        users.push(UserWithCompetitionScore::new(db, user, tournament_id, competition_id).await?);
    }
    Ok(users)
}
