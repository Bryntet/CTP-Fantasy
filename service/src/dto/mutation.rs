use std::fmt::Display;

use bcrypt::{hash, DEFAULT_COST};
use itertools::Itertools;
use log::error;

use rocket::http::CookieJar;
use rocket::request::FromParam;
use rocket::warn;
use sea_orm::sea_query::OnConflict;
use sea_orm::ActiveValue::Set;
use sea_orm::{sea_query, ActiveModelTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, ModelTrait, NotSet, TransactionTrait, SqlErr};

use entity::fantasy_pick;
use entity::prelude::{
    FantasyTournament, PhantomCompetitionInFantasyTournament, User, UserAuthentication,
    UserCompetitionScoreInFantasyTournament, UserInFantasyTournament,
};
use entity::sea_orm_active_enums::FantasyTournamentInvitationStatus;

use crate::dto::pdga::{add_players, RoundStatus};
use crate::error::GenericError;
use crate::error::PlayerError;
use crate::{generate_cookie, player_exists};

use super::pdga::CompetitionInfo;
use super::*;

impl FantasyPick {
    pub async fn change_or_insert(
        &self,
        db: &impl ConnectionTrait,
        user_id: i32,
        tournament_id: i32,
        div: Division,
    ) -> Result<(), GenericError> {
        if !player_exists(db, self.pdga_number).await {
            Err(PlayerError::NotFound.into())
        } else {
            let actual_player_div =
                super::super::get_player_division_in_tournament(db, self.pdga_number, tournament_id).await;
            let actual_player_div = match actual_player_div {
                Err(_) => Err(GenericError::UnknownError("Unable to get player division")),
                Ok(None) => Err(GenericError::NotFound("Player not in tournament")),
                Ok(Some(v)) => Ok(v),
            }?;

            if !actual_player_div.eq(&div) {
                Err(GenericError::Conflict("Player division does not match division"))?
            }

            let person_in_slot =
                Self::player_in_slot(db, user_id, tournament_id, self.slot, (&div).into()).await?;

            if let Some(player) = person_in_slot {
                let player: fantasy_pick::ActiveModel = player.into();
                player
                    .delete(db)
                    .await
                    .map_err(|_| GenericError::UnknownError("Unable to remove pick"))?;
            }

            if let Some(player) =
                Self::player_already_chosen(db, user_id, tournament_id, self.pdga_number).await?
            {
                let player: fantasy_pick::ActiveModel = player.into();
                player
                    .delete(db)
                    .await
                    .map_err(|_| GenericError::UnknownError("Unable to remove pick"))?;
            }
            self.insert(db, user_id, tournament_id, actual_player_div).await?;
            Ok(())
        }
    }

    async fn is_benched(&self, db: &impl ConnectionTrait, tournament_id: i32) -> Result<bool, GenericError> {
        Ok(self.slot > (super::super::get_tournament_bench_limit(db, tournament_id).await?))
    }

    async fn insert(
        &self,
        db: &impl ConnectionTrait,
        user_id: i32,
        tournament_id: i32,
        division: Division,
    ) -> Result<(), GenericError> {
        let pick = fantasy_pick::ActiveModel {
            id: NotSet,
            user: Set(user_id),
            pick_number: Set(self.slot),
            player: Set(self.pdga_number),
            fantasy_tournament_id: Set(tournament_id),
            division: Set((&division).into()),
            benched: Set(self.is_benched(db, tournament_id).await?),
        };
        pick.save(db)
            .await
            .map_err(|_| GenericError::UnknownError("Unknown error while saving pick"))?;
        Ok(())
    }
}

impl From<&Division> for &sea_orm_active_enums::Division {
    fn from(division: &Division) -> Self {
        match division {
            Division::MPO => &sea_orm_active_enums::Division::Mpo,
            Division::FPO => &sea_orm_active_enums::Division::Fpo,
            Division::Unknown => &sea_orm_active_enums::Division::Mpo,
        }
    }
}

impl From<&Division> for sea_orm_active_enums::Division {
    fn from(division: &Division) -> Self {
        match division {
            Division::MPO => sea_orm_active_enums::Division::Mpo,
            Division::FPO => sea_orm_active_enums::Division::Fpo,
            Division::Unknown => sea_orm_active_enums::Division::Mpo,
        }
    }
}

impl From<sea_orm_active_enums::Division> for Division {
    fn from(division: sea_orm_active_enums::Division) -> Self {
        match division {
            sea_orm_active_enums::Division::Mpo => Division::MPO,
            sea_orm_active_enums::Division::Fpo => Division::FPO,
        }
    }
}

impl<'r> FromParam<'r> for Division {
    type Error = std::convert::Infallible;

    fn from_param(param: &'r str) -> Result<Self, Self::Error> {
        Ok(Division::from(param))
    }
}

impl From<&str> for Division {
    fn from(division: &str) -> Division {
        match division {
            "MPO" => Division::MPO,
            "FPO" => Division::FPO,
            _ => {
                warn!("Unknown division, defaulting to MPO");
                Division::MPO
            }
        }
    }
}

impl Display for Division {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Division::MPO => "MPO".to_string(),
            Division::FPO => "FPO".to_string(),
            Division::Unknown => {
                warn!("Unknown division, defaulting to MPO");
                "MPO".to_string()
            }
        };
        write!(f, "{}", str)
    }
}

impl UserLogin {
    pub async fn insert<'a>(
        &'a self,
        db: &'a DatabaseConnection,
        cookies: &CookieJar<'_>,
    ) -> Result<(), GenericError> {
        let txn = db
            .begin()
            .await
            .map_err(|_| GenericError::UnknownError("Unable to start transaction"))?;
        let user = self.active_user();
        let user_id = User::insert(user)
            .exec(&txn)
            .await
            .map_err(|e| {
                e.sql_err()
                    .map(|_| GenericError::Conflict("Username already taken"))
                    .unwrap_or_else(|| GenericError::UnknownError("Unable to insert user into database"))
            })?
            .last_insert_id;
        let hashed_password = hash(&self.password, DEFAULT_COST).expect("hashing should work");
        let authentication = self.active_authentication(hashed_password, user_id);
        UserAuthentication::insert(authentication)
            .exec(&txn)
            .await
            .map_err(|_| GenericError::UnknownError("Inserting user authentication failed"))?;
        txn.commit()
            .await
            .map_err(|_| GenericError::UnknownError("Transaction commit failed"))?;
        generate_cookie(db, user_id, cookies).await
    }
}

impl UserScore {
    pub async fn insert(self, db: &DatabaseConnection, competition_id: i32) -> Result<(), DbErr> {
        UserCompetitionScoreInFantasyTournament::insert(self.into_active_model(competition_id))
            .exec(db)
            .await?;
        Ok(())
    }
}

impl CreateTournament {
    pub async fn insert(&self, db: &DatabaseConnection, owner_id: i32) -> Result<(), GenericError> {
        let tour = FantasyTournament::insert(self.clone().into_active_model(owner_id))
            .exec(db)
            .await
            .map_err(|e| {
                match e.sql_err() {
                    Some(SqlErr::UniqueConstraintViolation(_)) => GenericError::Conflict("Tournament name already taken"),
                    Some(SqlErr::ForeignKeyConstraintViolation(_)) => GenericError::NotFound("Owner not found"),
                    _ => {
                        error!("Unable to insert fantasy tournament: {:#?}", e.sql_err());
                        GenericError::UnknownError("Unable to insert fantasy tournament")
                    }
                }
            })?;
        UserInFantasyTournament::insert(user_in_fantasy_tournament::ActiveModel {
            id: NotSet,
            user_id: Set(owner_id),
            fantasy_tournament_id: Set(tour.last_insert_id),
            invitation_status: Set(FantasyTournamentInvitationStatus::Accepted),
        })
        .exec(db)
        .await.map_err(|e|{
            match e.sql_err() {
                Some(SqlErr::UniqueConstraintViolation(_)) => GenericError::Conflict("User already in tournament"),
                Some(SqlErr::ForeignKeyConstraintViolation(_)) => GenericError::NotFound("User not found"),
                _ => {
                    error!("Unable to insert user in fantasy tournament: {:#?}", e.sql_err());
                    GenericError::UnknownError("Unable to insert user in fantasy tournament")
                }
            }
        })?;
        FantasyTournamentDivs::insert(self.divisions.clone(), db, tour.last_insert_id).await.map_err(|e| {
            match e.sql_err() {
                Some(SqlErr::ForeignKeyConstraintViolation(_)) => GenericError::NotFound("Division not found"),
                _ => {
                    error!("Unable to insert fantasy tournament divisions: {:#?}", e.sql_err());
                    GenericError::UnknownError("Unable to insert fantasy tournament divisions")
                }
            }
        })?;

        Ok(())
    }
}

impl FantasyTournamentDivs {
    pub async fn insert(
        divisions: Vec<Division>,
        db: &DatabaseConnection,
        tournament_id: i32,
    ) -> Result<(), DbErr> {
        let txn = db.begin().await?;
        for div in divisions {
            let div = fantasy_tournament_division::ActiveModel {
                id: NotSet,
                fantasy_tournament_id: Set(tournament_id),
                division: Set((&div).into()),
            };
            div.save(&txn).await?;
        }
        txn.commit().await?;
        Ok(())
    }
}

impl PlayerInCompetition {
    pub async fn insert(&self, db: &impl ConnectionTrait) -> Result<(), DbErr> {
        let player = self.active_model();
        player.save(db).await?;
        Ok(())
    }

    fn active_model(&self) -> player_in_competition::ActiveModel {
        player_in_competition::ActiveModel {
            id: NotSet,
            pdga_number: Set(self.pdga_number),
            competition_id: Set(self.competition_id),
            division: Set((&self.division).into()),
        }
    }
}
#[allow(async_fn_in_trait)]
pub trait InsertCompetition {
    async fn insert_in_db(
        &self,
        db: &impl ConnectionTrait,
        level: sea_orm_active_enums::CompetitionLevel,
    ) -> Result<(), GenericError>;
    async fn insert_in_fantasy(
        &self,
        db: &impl ConnectionTrait,
        fantasy_tournament_id: u32,
    ) -> Result<(), GenericError>;
}

impl InsertCompetition for PhantomCompetition {
    async fn insert_in_db(
        &self,
        db: &impl ConnectionTrait,
        level: sea_orm_active_enums::CompetitionLevel,
    ) -> Result<(), GenericError> {
        use entity::prelude::PhantomCompetition;
        PhantomCompetition::insert(self.active_model(level))
            .exec(db)
            .await
            .map_err(|_| GenericError::UnknownError("Unable to insert phantom competition"))?;
        Ok(())
    }

    async fn insert_in_fantasy(
        &self,
        db: &impl ConnectionTrait,
        fantasy_tournament_id: u32,
    ) -> Result<(), GenericError> {
        match PhantomCompetitionInFantasyTournament::insert(
            phantom_competition_in_fantasy_tournament::ActiveModel {
                id: NotSet,
                phantom_competition_id: Set(self.competition_id.unwrap() as i32),
                fantasy_tournament_id: Set(fantasy_tournament_id as i32),
            },
        )
        .exec(db)
        .await
        {
            Ok(_) => Ok(()),
            Err(_e) => Err(GenericError::Conflict("Competition already added")),
        }
    }
}

impl InsertCompetition for CompetitionInfo {
    async fn insert_in_db(
        &self,
        db: &impl ConnectionTrait,
        level: sea_orm_active_enums::CompetitionLevel,
    ) -> Result<(), GenericError> {
        self.insert_competition_in_db(db, level)
            .await
            .map_err(|_| GenericError::UnknownError("Unable to insert rounds in database"))?;
        self.insert_rounds(db).await?;
        Ok(())
    }

    async fn insert_in_fantasy(
        &self,
        db: &impl ConnectionTrait,
        fantasy_tournament_id: u32,
    ) -> Result<(), GenericError> {
        use entity::prelude::{Competition, CompetitionInFantasyTournament};
        match Competition::find_by_id(self.competition_id as i32)
            .one(db)
            .await
            .map_err(|_| GenericError::UnknownError("Internal error while trying to get competition"))?
        {
            Some(c) => {
                match c
                    .find_related(CompetitionInFantasyTournament)
                    .one(db)
                    .await
                    .map_err(|_| {
                        GenericError::UnknownError(
                            "Internal db error while trying to find competition in fantasy tournament",
                        )
                    })? {
                    Some(_) => Err(GenericError::Conflict("Competition already added")),
                    None => {
                        CompetitionInFantasyTournament::insert(self.fantasy_model(fantasy_tournament_id))
                            .exec(db)
                            .await
                            .map_err(|_| {
                                GenericError::UnknownError(
                                    "Unable to insert competition into fanatasy tournament due to unknown db error",
                                )
                            })?;
                        Ok(())
                    }
                }
            }
            None => Err(GenericError::NotFound("Competition not found in PDGA")),
        }
    }
}

impl CompetitionInfo {
    pub async fn insert_rounds(&self, db: &impl ConnectionTrait) -> Result<(), GenericError> {
        use entity::round::{Column as RoundColumn, Entity as RoundEnt};

        let cols = vec![RoundColumn::CompetitionId, RoundColumn::RoundNumber];
        let times = self.date_range.date_times();

        if times.len() != self.rounds.len() {
            return Err(GenericError::UnknownError("Unable to insert rounds into database"));
        }
        let round_models = self
            .rounds
            .iter()
            .enumerate()
            .map(|(idx, round)| {
                round.active_model(times[idx].fixed_offset())
            })
            .collect_vec();

        if !round_models.is_empty() {
            RoundEnt::insert_many(round_models)
                .on_conflict(
                    OnConflict::columns(cols)
                        .update_column(RoundColumn::Status)
                        .to_owned(),
                )
                .exec(db)
                .await
                .map_err(|e| {
                    error!("Unable to insert rounds into database: {:#?}", e);
                    GenericError::UnknownError("Unable to insert rounds into database")
                })?;
        }
        Ok(())
    }

    pub async fn save_round_scores(&self, db: &impl ConnectionTrait) -> Result<(), GenericError> {
        // TODO: ADD STATUS TO ROUND

        let rounds = self
            .rounds
            .iter()
            .filter(|r| r.status() != RoundStatus::Pending)
            .flat_map(|r| {
                r.all_player_active_models(r.round_number as i32, self.competition_id as i32)
                    .into_iter()
            })
            .collect_vec();
        if !rounds.is_empty() {
            super::super::update_or_insert_many_player_round_scores(db, rounds).await
        } else {
            Ok(())
        }
    }

    async fn insert_competition_in_db(
        &self,
        db: &impl ConnectionTrait,
        level: sea_orm_active_enums::CompetitionLevel,
    ) -> Result<(), GenericError> {
        let active = self.active_model(level);
        active
            .insert(db)
            .await
            .map_err(|_| GenericError::UnknownError("Unable to insert competition in database"))?;
        Ok(())
    }


    pub async fn insert_players(
        &self,
        db: &impl ConnectionTrait,
        fantasy_tournament_id: Option<i32>,
    ) -> Result<(), GenericError> {
        let players = self.get_all_player_scores();
        add_players(db, players, fantasy_tournament_id).await?;
        Ok(())
    }

    pub async fn save_user_scores(
        &self,
        db: &impl ConnectionTrait,
        fantasy_tournament_id: u32,
    ) -> Result<(), GenericError> {
        let user_scores = self.get_user_scores(db, fantasy_tournament_id).await?;
        if !user_scores.is_empty() {
            user_competition_score_in_fantasy_tournament::Entity::insert_many(
                user_scores
                    .into_iter()
                    .dedup_by(|a, b| a.competition_id == b.competition_id && a.pdga_num == b.pdga_num)
                    .map(|p| p.into_active_model(self.competition_id as i32)),
            )
            .on_conflict(
                sea_query::OnConflict::columns(vec![
                    user_competition_score_in_fantasy_tournament::Column::FantasyTournamentId,
                    user_competition_score_in_fantasy_tournament::Column::User,
                    user_competition_score_in_fantasy_tournament::Column::CompetitionId,
                    user_competition_score_in_fantasy_tournament::Column::PdgaNumber,
                ])
                .update_column(user_competition_score_in_fantasy_tournament::Column::Score)
                .to_owned(),
            )
            .exec(db)
            .await
            .map_err(|e| {
                error!("Unable to insert user scores into database: {:#?}", e);
                GenericError::UnknownError("Unable to insert user score from competition into database")
            })?;
        }
        Ok(())
    }
}
