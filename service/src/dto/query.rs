use sea_orm::{ConnectionTrait, EntityTrait, NotSet};
use sea_orm::ActiveValue::Set;
use sea_orm::ColumnTrait;
use sea_orm::prelude::Date;
use sea_orm::QueryFilter;

use entity::{user, user_authentication};
//use entity::prelude::Round;

use crate::dto::pdga::{ApiPlayer, CompetitionInfo, PlayerScore};
use crate::error::GenericError;

use super::*;

trait ToModel {
    type Model;
    fn to_model(&self) -> Self::Model;
}

impl UserLogin {
    pub(super) fn active_user(&self) -> user::ActiveModel {
        user::ActiveModel {
            id: NotSet,
            name: Set(self.username.clone()),
        }
    }
    pub(super) fn active_authentication(
        &self,
        hashed_password: String,
        user_id: i32,
    ) -> user_authentication::ActiveModel {
        user_authentication::ActiveModel {
            user_id: Set(user_id),
            hashed_password: Set(hashed_password),
        }
    }
}

impl UserScore {
    pub fn into_active_model(
        self,
        competition_id: i32,
    ) -> user_competition_score_in_fantasy_tournament::ActiveModel {
        user_competition_score_in_fantasy_tournament::ActiveModel {
            id: NotSet,
            user: Set(self.user),
            score: Set(self.score),
            fantasy_tournament_id: Set(self.fantasy_tournament_id as i32),
            competition_id: Set(competition_id),
        }
    }
}

impl CreateTournament {
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
}

impl FantasyPick {
    pub(super) async fn player_in_slot<C>(
        db: &C,
        user_id: i32,
        fantasy_tournament_id: i32,
        slot: i32,
        division: sea_orm_active_enums::Division,
    ) -> Result<Option<fantasy_pick::Model>, GenericError>
    where
        C: ConnectionTrait,
    {
        use entity::prelude::FantasyPick as FantasyPickEntity;
        let existing_pick = FantasyPickEntity::find()
            .filter(
                fantasy_pick::Column::PickNumber
                    .eq(slot)
                    .and(fantasy_pick::Column::FantasyTournamentId.eq(fantasy_tournament_id))
                    .and(fantasy_pick::Column::User.eq(user_id))
                    .and(fantasy_pick::Column::Division.eq(division)),
            )
            .one(db)
            .await.map_err(|_|GenericError::UnknownError("database error while trying to find pick"))?;
        Ok(existing_pick)
    }

    pub(super) async fn player_already_chosen<C>(
        db: &C,
        user_id: i32,
        fantasy_tournament_id: i32,
        pdga_number: i32,
    ) -> Result<Option<fantasy_pick::Model>, GenericError>
    where
        C: ConnectionTrait,
    {
        use entity::prelude::FantasyPick as FantasyPickEntity;
        let existing_pick = FantasyPickEntity::find()
            .filter(
                fantasy_pick::Column::Player
                    .eq(pdga_number)
                    .and(fantasy_pick::Column::FantasyTournamentId.eq(fantasy_tournament_id))
                    .and(fantasy_pick::Column::User.eq(user_id)),
            )
            .one(db)
            .await.map_err(|_|GenericError::UnknownError("Unknown error while trying to find pick in database"))?;
        Ok(existing_pick)
    }
}

impl CompetitionInfo {
    pub(crate) fn active_model(
        &self,
        level: entity::sea_orm_active_enums::CompetitionLevel,
    ) -> competition::ActiveModel {
        competition::ActiveModel {
            id: Set(self.competition_id as i32),
            status: Set(sea_orm_active_enums::CompetitionStatus::NotStarted),
            name: Set(self.name.clone()),
            rounds: Set(self.date_range.len() as i32),
            level: Set(level),
        }
    }

    pub(crate) fn round_active_model(&self, round_number: usize, date: Date) -> round::ActiveModel {
        round::ActiveModel {
            id: NotSet,
            round_number: sea_orm::Set(round_number as i32),
            competition_id: sea_orm::Set(self.competition_id as i32),
            date: sea_orm::Set(date),
        }
    }

    /*pub(crate) async fn get_round(
        &self,
        db: &impl ConnectionTrait,
        date: Date,
    ) -> Result<Option<round::Model>, DbErr> {
        Round::find()
            .filter(
                round::Column::Date
                    .eq::<Date>(date)
                    .and(round::Column::CompetitionId.eq(self.competition_id as i32)),
            )
            .one(db)
            .await
    }*/

    pub(crate) fn fantasy_model(
        &self,
        fantasy_tournament_id: u32,
    ) -> competition_in_fantasy_tournament::ActiveModel {
        competition_in_fantasy_tournament::ActiveModel {
            id: NotSet,
            competition_id: Set(self.competition_id as i32),
            fantasy_tournament_id: Set(fantasy_tournament_id as i32),
        }
    }

    pub async fn is_in_db(&self, db: &impl ConnectionTrait) -> Result<bool, GenericError> {
        competition::Entity::find_by_id(self.competition_id as i32)
            .one(db)
            .await
            .map(|x| x.is_some()).map_err(|_|GenericError::UnknownError("Internal db error"))
    }

    pub(super) async fn get_all_player_scores(&self) -> Result<Vec<ApiPlayer>, GenericError> {
        let mut players: Vec<ApiPlayer> = Vec::new();
        for div in &self.divisions {
            for round in 1..=self.rounds {
                players.extend(self.get_players_wrapper(round, div).await);
            }
        }
        Ok(players)
    }

    async fn get_players_wrapper(&self, round: u8, div: &Division) -> Vec<ApiPlayer> {
        pdga::get_players_from_api(self.competition_id, div, round as i32)
            .await
            .map_err(|e| {
                #[cfg(debug_assertions)]
                dbg!(&e);
                e
            })
            .unwrap_or_default()
    }

    pub(super) async fn get_current_player_scores(&self) -> Result<Vec<ApiPlayer>, GenericError> {
        let mut players: Vec<ApiPlayer> = Vec::new();
        for div in &self.divisions {
            let round = if let Some(highest) = self.highest_completed_round {
                if highest == self.rounds {
                    highest
                } else {
                    let temp = self.get_players_wrapper(highest + 1, div).await;
                    if temp.iter().any(|p| p.round_started) {
                        highest + 1
                    } else {
                        highest
                    }
                }
            } else {
                self.highest_completed_round.unwrap_or(1)
            };
            players.extend(self.get_players_wrapper(round, div).await);
        }
        Ok(players)
    }

    pub(super) async fn get_user_scores(
        &self,
        db: &impl ConnectionTrait,
        fantasy_tournament_id: u32,
    ) -> Result<Vec<UserScore>, GenericError> {
        let mut user_scores: Vec<UserScore> = Vec::new();
        if let Ok(players) = self.get_current_player_scores().await {
            for player in players {
                let a = PlayerScore::from(player)
                    .get_user_fantasy_score(db, fantasy_tournament_id, self.competition_id)
                    .await;
                if let Ok(Some(score)) = a {
                    user_scores.push(score);
                } else {
                    //                    dbg!(a);
                }
            }
        }
        Ok(user_scores)
    }
}
impl PhantomCompetition {
    pub(crate) fn active_model(
        &self,
        level: sea_orm_active_enums::CompetitionLevel,
    ) -> phantom_competition::ActiveModel {
        phantom_competition::ActiveModel {
            id: NotSet,
            name: Set(self.name.clone()),
            date: Set(self.start_date),
            level: Set(level),
        }
    }
}

impl From<CompetitionLevel> for sea_orm_active_enums::CompetitionLevel {
    fn from(level: CompetitionLevel) -> Self {
        match level {
            CompetitionLevel::Major => sea_orm_active_enums::CompetitionLevel::Major,
            CompetitionLevel::Playoff => sea_orm_active_enums::CompetitionLevel::Playoff,
            CompetitionLevel::ElitePlus => sea_orm_active_enums::CompetitionLevel::ElitePlus,
            CompetitionLevel::Elite => sea_orm_active_enums::CompetitionLevel::Elite,
            CompetitionLevel::Silver => sea_orm_active_enums::CompetitionLevel::Silver,
        }
    }
}

impl CompetitionLevel {
    pub(crate) fn multiplier(&self) -> f32 {
        match self {
            CompetitionLevel::Major => 2.0,
            CompetitionLevel::Playoff => 1.5,
            CompetitionLevel::ElitePlus => 1.25,
            CompetitionLevel::Elite => 1.0,
            CompetitionLevel::Silver => 0.5,
        }
    }
}
