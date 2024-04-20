use super::User;
use crate::error::GenericError;
use rocket::{error, warn};
use rocket_okapi::okapi::schemars::JsonSchema;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, ModelTrait, QueryFilter};
use serde_derive::Serialize;

#[derive(Debug, Serialize, JsonSchema)]
struct Player {
    name: String,
    id: u32,
}

impl From<entity::player::Model> for Player {
    fn from(model: entity::player::Model) -> Self {
        Self {
            name: format!("{} {}", model.first_name, model.last_name),
            id: model.pdga_number as u32,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
struct PlayerCompetitionScore {
    player: Player,
    score: u8,
    placement: u32,
}

impl PlayerCompetitionScore {
    async fn from_db(
        db: &impl ConnectionTrait,
        score_model: entity::user_competition_score_in_fantasy_tournament::Model,
    ) -> Result<Option<Self>, GenericError> {
        if let Some(player_model) = score_model
            .find_related(entity::player::Entity)
            .one(db)
            .await
            .map_err(|e| {
                error!(
                    "Unknown fatal error while getting player from db in competition score {:#?}",
                    e
                );
                GenericError::UnknownError(
                    "Unknown internal db error while trying to get player from competition score",
                )
            })?
        {
            let mut players: Vec<entity::player_round_score::Model> =
                entity::player_round_score::Entity::find()
                    .filter(
                        entity::player_round_score::Column::PdgaNumber
                            .eq(score_model.pdga_number)
                            .and(
                                entity::player_round_score::Column::CompetitionId
                                    .eq(score_model.competition_id),
                            ),
                    )
                    .all(db)
                    .await
                    .map_err(|e| {
                        error!("Unable to get player round scores from db {:#?}", e);
                        GenericError::UnknownError("Unable to get player round scores from db")
                    })?;
            players.sort_by(|a, b| a.round.cmp(&b.round));

            if players.is_empty() {
                warn!("Player does not have any round scores");
                return Ok(None);
            }
            Ok(Some(Self {
                player: Player::from(player_model),
                score: score_model.score as u8,
                placement: players.last().unwrap().placement as u32,
            }))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CompetitionScores(Vec<PlayerCompetitionScore>);

impl CompetitionScores {
    pub async fn new(
        db: &impl ConnectionTrait,
        competition_id: i32,
        user_id: i32,
        tournament_id: i32,
    ) -> Result<Self, GenericError> {
        use entity::user_competition_score_in_fantasy_tournament as CompScore;
        use CompScore::Entity as CompScoreEnt;
        let score_models = CompScoreEnt::find()
            .filter(
                CompScore::Column::User
                    .eq(user_id)
                    .and(CompScore::Column::CompetitionId.eq(competition_id))
                    .and(CompScore::Column::FantasyTournamentId.eq(tournament_id)),
            )
            .all(db)
            .await
            .map_err(|e| {
                error!("Unable to get user scores from competition {:#?}", e);
                GenericError::UnknownError("Unable to get user scores from competition")
            })?;

        let mut scores = Vec::new();
        for score in score_models {
            match PlayerCompetitionScore::from_db(db, score).await? {
                None => warn!("Player does not have a score in competition"),
                Some(score) => scores.push(score),
            }
        }
        scores.sort_by(|a, b| a.placement.cmp(&b.placement));
        Ok(Self(scores))
    }

    pub fn total_score(&self) -> u32 {
        self.0.iter().map(|x| x.score as u32).sum()
    }
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct UserWithCompetitionScores {
    pub user: User,
    pub competition_scores: CompetitionScores,
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

impl UserWithCompetitionScores {
    async fn new(
        db: &impl ConnectionTrait,
        user: entity::user::Model,
        tournament_id: i32,
        competition_id: i32,
    ) -> Result<Self, GenericError> {
        let user = User::from(user);
        let competition_score = CompetitionScores::new(db, competition_id, user.id, tournament_id).await?;

        Ok(Self {
            user,
            total_score: competition_score.total_score(),
            competition_scores: competition_score,
        })
    }
}

pub async fn user_competition_scores(
    db: &impl ConnectionTrait,
    tournament_id: i32,
    competition_id: i32,
) -> Result<Vec<UserWithCompetitionScores>, GenericError> {
    use entity::user::Entity as UserEnt;
    let user_fantasy_models = entity::user_in_fantasy_tournament::Entity::find()
        .filter(entity::user_in_fantasy_tournament::Column::FantasyTournamentId.eq(tournament_id))
        .all(db)
        .await
        .map_err(|e| {
            error!("Unable to get users in tournament from db {:#?}", e);
            GenericError::UnknownError("Unable to get users in tournament from db")
        })?;
    let mut user_models = Vec::new();
    for model in user_fantasy_models {
        user_models.push(model.find_related(UserEnt).one(db).await.map_err(|e| {
            error!("Unable to get user from db {:#?}", e);
            GenericError::UnknownError("Unable to get user from db")
        })?);
    }

    let mut users = Vec::new();
    for user in user_models.into_iter().flatten() {
        users.push(UserWithCompetitionScores::new(db, user, tournament_id, competition_id).await?);
    }
    Ok(users)
}
