use chrono::{TimeZone, Utc};
use itertools::Itertools;
use log::error;
use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::ActiveValue::Set;
use sea_orm::ColumnTrait;
use sea_orm::QueryFilter;
use sea_orm::{EntityTrait, NotSet};

//use entity::prelude::Round;

use crate::dto::pdga::{PlayerScore, RoundStatus};

use super::*;

impl UserLogin {
    pub(super) fn active_user(&self) -> user::ActiveModel {
        user::ActiveModel {
            id: NotSet,
            name: Set(self.username.clone()),
            admin: Set(false),
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
            pdga_number: Set(self.pdga_num as i32),
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
            bench_size: match self.amount_in_bench {
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
            .await
            .map_err(|_| GenericError::UnknownError("database error while trying to find pick"))?;
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
            .await
            .map_err(|_| GenericError::UnknownError("Unknown error while trying to find pick in database"))?;
        Ok(existing_pick)
    }
}

impl CompetitionInfo {
    pub(crate) fn active_model(
        &self,
        level: sea_orm_active_enums::CompetitionLevel,
    ) -> competition::ActiveModel {
        competition::ActiveModel {
            id: Set(self.competition_id as i32),
            status: Set(self.status().into()),
            name: Set(self.name.clone()),
            rounds: Set(self.amount_of_rounds as i32),
            level: Set(level),
            ended_at: Set(self.status_to_finished()),
            start_date: Set(self.date_range.start_date()),
        }
    }

    fn status_to_finished(&self) -> Option<DateTimeWithTimeZone> {
        match self.status() {
            CompetitionStatus::Finished => {
                let tz = self.date_range.timezone();
                let now = Utc::now().naive_utc();
                let local_time = tz.from_utc_datetime(&now);
                /*let minute = local_time.minute();
                let rounded_time = if minute < 30 {
                    local_time.with_minute(30)?.with_second(0)?
                } else {
                    local_time.with_hour(local_time.hour() + 1)?.with_minute(0)?.with_second(0)?
                };*/

                Some(DateTimeWithTimeZone::from(local_time.fixed_offset()))
            }
            _ => None,
        }
    }

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
            .map(|x| x.is_some())
            .map_err(|_| GenericError::UnknownError("Internal db error"))
    }

    pub(super) fn get_all_player_active_models(&self) -> Vec<player::ActiveModel> {
        self.rounds.iter().flat_map(|round| round.players.iter().map(|player|player.to_active_model())).collect_vec()
    }
    
    pub(crate) fn get_all_player_divisions(&self, fantasy_tournament_id: i32) -> Vec<player_division_in_fantasy_tournament::ActiveModel> {
        self.rounds.iter().flat_map(|round| round.players.iter().map(|player|player.to_division_active_model(fantasy_tournament_id))).collect_vec()
    }

    /*fn get_all_round_score_models(&self) -> Vec<entity::player_round_score::ActiveModel> {
        self.rounds
            .iter()
            .flat_map(|r| r.all_player_active_models(r.round_number as i32, self.competition_id as i32))
            .collect()
    }*/

    fn current_round(&self) -> usize {
        if let Some(highest) = self.highest_completed_round {
            match self.status() {
                CompetitionStatus::Finished => highest as usize - 1,
                CompetitionStatus::Active(round) => round,
                CompetitionStatus::Pending => 0,
            }
        } else {
            0
        }
    }

    pub(super) fn get_current_player_scores(&self) -> Vec<&PlayerScore> {
        let current_round = self.current_round();
        //dbg!(current_round);
        if current_round >= self.rounds.len() {
            error!("Current round is higher than rounds length");
        }
        self.rounds[current_round].players.iter().collect_vec()
    }

    pub(super) async fn get_user_scores(
        &self,
        db: &impl ConnectionTrait,
        fantasy_tournament_id: u32,
    ) -> Result<Vec<UserScore>, GenericError> {
        let mut user_scores: Vec<UserScore> = Vec::new();
        let players = self.get_current_player_scores();
        
        for player in players {
            let score = player.get_user_fantasy_score(db, fantasy_tournament_id, self.competition_id)
                .await?;
            if let Some(score) = score {
                user_scores.push(score);
            }
        }
        Ok(user_scores)
    }

    pub fn status(&self) -> CompetitionStatus {
        if self.rounds.len() < self.amount_of_rounds {
            CompetitionStatus::Active(self.rounds.len())
        } else if self.rounds.iter().all(|r| r.status() == RoundStatus::Finished) {
            CompetitionStatus::Finished
        } else if let Some(round) = self.rounds.iter().find(|r| r.status() == RoundStatus::Started) {
            CompetitionStatus::Active(round.round_number - 1)
        } else if let Some((round, _)) = self
            .rounds
            .iter()
            .enumerate()
            .filter(|(_, round)| round.status() == RoundStatus::Finished)
            .max_by(|(idx, _), (other, _)| idx.cmp(other))
        {
            CompetitionStatus::Active(round)
        } else {
            CompetitionStatus::Pending
        }
    }
}

pub enum CompetitionStatus {
    Pending,
    Active(usize),
    Finished,
}

impl From<RoundStatus> for sea_orm_active_enums::CompetitionStatus {
    fn from(status: RoundStatus) -> Self {
        match status {
            RoundStatus::Pending => sea_orm_active_enums::CompetitionStatus::NotStarted,
            RoundStatus::Started => sea_orm_active_enums::CompetitionStatus::Running,
            RoundStatus::Finished => sea_orm_active_enums::CompetitionStatus::Finished,
        }
    }
}

impl From<CompetitionStatus> for sea_orm_active_enums::CompetitionStatus {
    fn from(status: CompetitionStatus) -> Self {
        match status {
            CompetitionStatus::Pending => sea_orm_active_enums::CompetitionStatus::NotStarted,
            CompetitionStatus::Active(_) => sea_orm_active_enums::CompetitionStatus::Running,
            CompetitionStatus::Finished => sea_orm_active_enums::CompetitionStatus::Finished,
        }
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
