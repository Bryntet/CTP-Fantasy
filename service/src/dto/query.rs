use chrono::{DateTime, TimeZone, Utc};
use itertools::Itertools;
use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::ActiveValue::Set;
use sea_orm::ColumnTrait;
use sea_orm::QueryFilter;
use sea_orm::{ConnectionTrait, EntityTrait, NotSet};

use entity::{user, user_authentication};
//use entity::prelude::Round;

use crate::dto::pdga::{CompetitionInfo, PlayerScore, RoundStatus};
use crate::error::GenericError;

use super::*;

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
            .map_err(|_| {
                GenericError::UnknownError("Unknown error while trying to find pick in database")
            })?;
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
            status: Set(self.is_active().into()),
            name: Set(self.name.clone()),
            rounds: Set(self.date_range.len() as i32),
            level: Set(level),
            ended_at: Set(self.status_to_finished()),
            start_date: Set(self.date_range.start_date()),
        }
    }

    fn status_to_finished(&self) -> Option<DateTimeWithTimeZone> {
        match self.is_active() {
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

    pub(crate) fn round_active_model(
        &self,
        round_number: usize,
        date: DateTime<chrono_tz::Tz>,
    ) -> round::ActiveModel {
        round::ActiveModel {
            id: NotSet,
            round_number: sea_orm::Set(round_number as i32),
            competition_id: sea_orm::Set(self.competition_id as i32),
            date: sea_orm::Set(DateTimeWithTimeZone::from(date.fixed_offset())),
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
            .map(|x| x.is_some())
            .map_err(|_| GenericError::UnknownError("Internal db error"))
    }

    pub(super) fn get_all_player_scores(&self) -> Vec<&PlayerScore> {
        self.rounds
            .iter()
            .flat_map(|r| r.players.iter())
            .collect_vec()
    }

    fn current_round(&self) -> usize {
        if let Some(highest) = self.highest_completed_round {
            match self.is_active() {
                CompetitionStatus::Finished => highest as usize - 1,
                CompetitionStatus::Active(round) => round - 1,
                CompetitionStatus::Pending => 0,
            }
        } else {
            0
        }
    }

    pub(super) fn get_current_player_scores(&self) -> &Vec<PlayerScore> {
        let current_round = self.current_round();
        //dbg!(current_round);
        &self.rounds[current_round].players
    }

    pub(super) async fn get_user_scores(
        &self,
        db: &impl ConnectionTrait,
        fantasy_tournament_id: u32,
    ) -> Result<Vec<UserScore>, GenericError> {
        let mut user_scores: Vec<UserScore> = Vec::new();
        let players = self.get_current_player_scores();
        for player in players {
            let score = player
                .get_user_fantasy_score(db, fantasy_tournament_id, self.competition_id)
                .await?;
            if let Some(score) = score {
                user_scores.push(score);
            }
        }
        Ok(user_scores)
    }

    fn is_active(&self) -> CompetitionStatus {
        let mut rounds = self.rounds.iter();
        if rounds.all(|r| r.status() == RoundStatus::Finished) {
            CompetitionStatus::Finished
        } else if let Some(r) = rounds
            .filter(|r| !(r.status() == RoundStatus::Started))
            .map(|r| r.round_number)
            .max()
        {
            CompetitionStatus::Active(r)
        } else {
            CompetitionStatus::Pending
        }
    }
}

enum CompetitionStatus {
    Pending,
    Active(usize),
    Finished,
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
