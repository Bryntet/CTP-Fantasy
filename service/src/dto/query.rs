use chrono::{TimeZone, Utc};
use sea_orm::ActiveValue::Set;
use sea_orm::{EntityTrait, IntoActiveModel, NotSet};

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

impl CompetitionInfo {
    pub(crate) async fn active_model(
        &self,
        db: &impl ConnectionTrait,
        level: Option<sea_orm_active_enums::CompetitionLevel>,
    ) -> Result<Option<competition::ActiveModel>, GenericError> {
        if let Ok(Some(model)) = competition::Entity::find_by_id(self.competition_id as i32)
            .one(db)
            .await
        {
            let status = model.status.clone();
            let mut model = model.into_active_model();

            Ok(
                if status != entity::sea_orm_active_enums::CompetitionStatus::from(self.status()) {
                    model.status = Set(self.status().into());
                    Some(model)
                } else {
                    None
                },
            )
        } else if let Some(level) = level {
            Ok(Some(competition::ActiveModel {
                id: Set(self.competition_id as i32),
                status: Set(self.status().into()),
                name: Set(self.name.clone()),
                rounds: Set(self.amount_of_rounds as i32),
                level: Set(level),
                ended_at: Set(self.status_to_finished()),
                start_date: Set(self.date_range.start_date()),
            }))
        } else {
            Err(GenericError::UnknownError(
                "Somehow, called active model without level when model not in DB",
            ))
        }
    }

    fn status_to_finished(&self) -> Option<DateTimeWithTimeZone> {
        match self.status() {
            CompetitionStatus::Finished => {
                let tz = self.date_range.timezone();
                let now = Utc::now();
                let local_time = tz.from_utc_datetime(&now.naive_utc());
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
        self.rounds
            .iter()
            .flat_map(|round| round.players.iter().map(|player| player.to_active_model()))
            .collect_vec()
    }

    pub(crate) fn get_all_player_divisions(
        &self,
        fantasy_tournament_id: i32,
    ) -> Vec<player_division_in_fantasy_tournament::ActiveModel> {
        self.rounds
            .iter()
            .flat_map(|round| {
                round
                    .players
                    .iter()
                    .map(|player| player.to_division_active_model(fantasy_tournament_id))
            })
            .collect_vec()
    }

    /*fn get_all_round_score_models(&self) -> Vec<entity::player_round_score::ActiveModel> {
        self.rounds
            .iter()
            .flat_map(|r| r.all_player_active_models(r.round_number as i32, self.competition_id as i32))
            .collect()
    }*/

    fn current_round(&self) -> usize {
        let highest = self
            .rounds
            .iter()
            .filter(|round| round.status() == RoundStatus::Finished)
            .count();
        match self.status() {
            CompetitionStatus::Finished => highest - 1,
            CompetitionStatus::Active(round) => round,
            CompetitionStatus::Pending => 0,
        }
    }

    pub(super) fn get_current_player_scores(&self) -> Result<Vec<&PlayerScore>, GenericError> {
        let current_round = self.current_round();

        let round = self
            .rounds
            .get(current_round)
            .ok_or(GenericError::NotFound("Unable to find current round"))?;
        if round.label == RoundLabel::Playoff {
            let mut round_to_return = self.rounds[current_round - 1]
                .players
                .iter()
                .dedup()
                .collect_vec();
            for player in &mut round_to_return {
                for replacing_player in round.players.iter() {
                    if replacing_player.pdga_number == player.pdga_number {
                        *player = replacing_player;
                    }
                }
            }
            Ok(round_to_return)
        } else {
            Ok(round.players.iter().dedup().collect_vec())
        }
    }

    pub(super) async fn get_user_scores(
        &self,
        db: &impl ConnectionTrait,
        fantasy_tournament_id: u32,
    ) -> Result<Vec<UserScore>, GenericError> {
        let mut user_scores: Vec<UserScore> = Vec::new();
        let players = self.get_current_player_scores()?;

        for player in players {
            let score = player
                .get_user_fantasy_score(db, fantasy_tournament_id, self.competition_id)
                .await?;
            if let Some(score) = score {
                user_scores.push(score);
            }
        }
        let max_picks = super::super::query::max_picks(db, fantasy_tournament_id as i32).await?;
        let bench_size =
            super::super::query::get_tournament_bench_size(db, fantasy_tournament_id as i32).await?;
        let mut filtered_scores = Vec::new();
        let divs = user_scores
            .iter()
            .map(|score| score.division)
            .dedup()
            .collect_vec();

        let mut user_ids = user_scores.iter().map(|score| score.user).collect_vec();
        user_ids.dedup();

        for user_id in user_ids {
            for div in &divs {
                let played_picks = user_scores
                    .clone()
                    .into_iter()
                    .filter(|score| score.user == user_id && &score.division == div)
                    .collect_vec();
                let amount_of_played_picks = played_picks.len();
                for pick in played_picks {
                    if (pick.slot as isize)
                        <= max_picks as isize
                            - (bench_size as isize - (max_picks as isize - amount_of_played_picks as isize))
                    {
                        filtered_scores.push(pick);
                    }
                }
            }
        }

        Ok(filtered_scores)
    }

    pub fn status(&self) -> CompetitionStatus {
        // Add so that it has to be 6 in the morning, day after competition should end, to count as finished.
        if self.rounds.iter().all(|r| r.status() == RoundStatus::Finished)
            && self.date_range.competition_allowed_to_end()
        {
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
    pub(crate) fn multiplier(&self) -> f64 {
        match self {
            CompetitionLevel::Major => 2.0,
            CompetitionLevel::Playoff => 1.5,
            CompetitionLevel::ElitePlus => 1.25,
            CompetitionLevel::Elite => 1.0,
            CompetitionLevel::Silver => 0.5,
        }
    }
}
