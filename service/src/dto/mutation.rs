use std::fmt::Display;

use bcrypt::{hash, DEFAULT_COST};
use chrono::Utc;
use log::error;
use rocket::http::CookieJar;
use rocket::warn;
use sea_orm::sea_query::OnConflict;
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, IntoActiveModel, ModelTrait,
    NotSet, QueryFilter, SqlErr, TransactionTrait,
};

use entity::prelude::{
    FantasyTournament, PhantomCompetitionInFantasyTournament, User, UserAuthentication,
    UserCompetitionScoreInFantasyTournament, UserInFantasyTournament,
};
use entity::sea_orm_active_enums::FantasyTournamentInvitationStatus;

use crate::dto::pdga::{add_players, RoundStatus};
use crate::error::PlayerError;
use crate::{generate_cookie, player_exists};

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
            return Err(PlayerError::NotFound.into());
        }
        let actual_player_div =
            super::super::get_player_division_in_tournament(db, self.pdga_number, tournament_id).await;

        let actual_player_div: Division = match actual_player_div {
            Err(_) => Err(GenericError::UnknownError("Unable to get player division")),
            Ok(None) => {
                if let Ok(Some(player)) = player::Entity::find_by_id(self.pdga_number).one(db).await {
                    if let Ok(Some(div)) = player
                        .find_related(player_division_in_fantasy_tournament::Entity)
                        .all(db)
                        .await
                        .map(|divs| divs.first().cloned())
                    {
                        Ok(div.division.into())
                    } else {
                        Err(GenericError::UnknownError("Unable to get player division"))
                    }
                } else {
                    Err(GenericError::UnknownError("Unable to get player division"))
                }
            }
            Ok(Some(v)) => Ok(v),
        }?;

        if !actual_player_div.eq(&div) {
            Err(GenericError::Conflict("Player division does not match division"))?
        }

        //let person_in_slot = Self::player_in_slot(db, user_id, tournament_id, self.slot, (&div).into()).await?;

        /*if let Some(player) = person_in_slot {
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
        }*/
        self.insert(db, user_id, tournament_id, actual_player_div).await?;
        Ok(())
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
        let mut new_pick = fantasy_pick::ActiveModel {
            id: NotSet,
            user: Set(user_id),
            pick_number: Set(self.slot),
            player: Set(self.pdga_number),
            fantasy_tournament_id: Set(tournament_id),
            division: Set((&division).into()),
            benched: Set(self.is_benched(db, tournament_id).await?),
        };
        let column_filter = fantasy_pick::Column::User
            .eq(user_id)
            .and(fantasy_pick::Column::FantasyTournamentId.eq(tournament_id))
            .and(fantasy_pick::Column::Division.eq::<sea_orm_active_enums::Division>(division.into()));

        let possible_prev_pick = fantasy_pick::Entity::find()
            .filter(
                column_filter
                    .clone()
                    .and(fantasy_pick::Column::Player.eq(new_pick.player.clone().take())),
            )
            .one(db)
            .await
            .map(|p| p.map(|p| p.into_active_model()));
        let other_pick = fantasy_pick::Entity::find()
            .filter(column_filter.and(fantasy_pick::Column::PickNumber.eq(self.slot)))
            .one(db)
            .await
            .map(|p| p.map(|p| p.into_active_model()));
        match (possible_prev_pick, other_pick) {
            // Swap two picks
            (Ok(Some(mut previous_placement_of_player)), Ok(Some(mut other_pick))) => {
                if previous_placement_of_player
                    .pick_number
                    .clone()
                    .take()
                    .is_some_and(|num| num != self.slot)
                {
                    previous_placement_of_player.player = Set(other_pick.player.clone().take().unwrap());
                    other_pick.clone().delete(db).await.map_err(|e| {
                        warn!("Unable to insert pick: {:#?}", e);
                        GenericError::UnknownError("Unable to insert pick")
                    })?;
                    new_pick.id = Set(other_pick.id.take().unwrap());
                    player_trade::ActiveModel {
                        id: NotSet,
                        user: Set(user_id),
                        player: Set(self.pdga_number),
                        slot: Set(self.slot),
                        fantasy_tournament_id: Set(tournament_id),
                        timestamp: Set(Utc::now().fixed_offset()),
                        is_local_swap: Set(true),
                        other_player: Set(previous_placement_of_player.player.clone().take()),
                        other_slot: Set(previous_placement_of_player.pick_number.clone().take()),
                    }
                    .save(db)
                    .await
                    .map_err(|e| {
                        warn!("Unable to insert pick: {:#?}", e);
                        GenericError::UnknownError("Unable to insert pick")
                    })?;
                    previous_placement_of_player.save(db).await.map_err(|e| {
                        warn!("Unable to insert pick: {:#?}", e);
                        GenericError::UnknownError("Unable to insert pick")
                    })?;
                    new_pick.insert(db).await.map_err(|e| {
                        warn!("Unable to insert pick: {:#?}", e);
                        GenericError::UnknownError("Unable to insert pick")
                    })?;
                }
                Ok(())
            }

            // Move pick to new slot when there is no pick in the new slot
            (Ok(Some(mut previous_placement_of_player)), Ok(None)) => {
                if previous_placement_of_player
                    .pick_number
                    .clone()
                    .take()
                    .is_some_and(|num| num != self.slot)
                {
                    player_trade::ActiveModel {
                        id: NotSet,
                        user: Set(user_id),
                        player: Set(self.pdga_number),
                        slot: Set(self.slot),
                        fantasy_tournament_id: Set(tournament_id),
                        timestamp: Set(Utc::now().fixed_offset()),
                        is_local_swap: Set(false),
                        other_player: Set(None),
                        other_slot: Set(previous_placement_of_player.pick_number.clone().take()),
                    }
                    .save(db)
                    .await
                    .map_err(|e| {
                        warn!("Unable to insert pick: {:#?}", e);
                        GenericError::UnknownError("Unable to insert pick")
                    })?;
                    previous_placement_of_player.pick_number = Set(self.slot);
                    dbg!(&previous_placement_of_player);

                    previous_placement_of_player.save(db).await.map_err(|e| {
                        warn!("Unable to insert pick: {:#?}", e);
                        GenericError::UnknownError("Unable to insert pick")
                    })?;
                }
                Ok(())
            }

            // Insert new pick when there is no pick in the new slot
            (Ok(None), Ok(None)) => {
                dbg!(&new_pick);
                player_trade::ActiveModel {
                    id: NotSet,
                    user: Set(user_id),
                    player: Set(self.pdga_number),
                    slot: Set(self.slot),
                    fantasy_tournament_id: Set(tournament_id),
                    timestamp: Set(Utc::now().fixed_offset()),
                    is_local_swap: Set(false),
                    other_player: Set(None),
                    other_slot: Set(None),
                }
                .save(db)
                .await
                .map_err(|e| {
                    warn!("Unable to insert pick: {:#?}", e);
                    GenericError::UnknownError("Unable to insert pick")
                })?;
                fantasy_pick::Entity::insert(new_pick)
                    .exec(db)
                    .await
                    .map_err(|e| {
                        warn!("Unable to insert pick: {:#?}", e);
                        GenericError::UnknownError("Unable to insert pick")
                    })?;
                Ok(())
            }

            // Insert new pick when there is a pick in the new slot
            (Ok(None), Ok(Some(mut other_pick))) => {
                other_pick.player = Set(self.pdga_number);
                dbg!(&other_pick);
                player_trade::ActiveModel {
                    id: NotSet,
                    user: Set(user_id),
                    player: Set(self.pdga_number),
                    slot: Set(self.slot),
                    fantasy_tournament_id: Set(tournament_id),
                    timestamp: Set(Utc::now().fixed_offset()),
                    is_local_swap: Set(false),
                    other_player: Set(other_pick.player.clone().take()),
                    other_slot: Set(other_pick.pick_number.clone().take()),
                }
                .save(db)
                .await
                .map_err(|e| {
                    warn!("Unable to insert pick: {:#?}", e);
                    GenericError::UnknownError("Unable to insert pick")
                })?;
                other_pick.player = Set(self.pdga_number);
                other_pick.save(db).await.map_err(|e| {
                    warn!("Unable to insert pick: {:#?}", e);
                    GenericError::UnknownError("Unable to insert pick")
                })?;
                Ok(())
            }
            (Err(_), Err(_)) | (Err(_), _) | (_, Err(_)) => {
                Err(GenericError::UnknownError("Unable to insert pick"))
            }
        }
    }
}

impl From<Division> for sea_orm_active_enums::Division {
    fn from(division: Division) -> Self {
        match division {
            Division::MPO => sea_orm_active_enums::Division::Mpo,
            Division::FPO => sea_orm_active_enums::Division::Fpo,
            Division::Unknown => sea_orm_active_enums::Division::Mpo,
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
            .map_err(|e| match e.sql_err() {
                Some(SqlErr::UniqueConstraintViolation(_)) => {
                    GenericError::Conflict("Tournament name already taken")
                }
                Some(SqlErr::ForeignKeyConstraintViolation(_)) => GenericError::NotFound("Owner not found"),
                _ => {
                    error!("Unable to insert fantasy tournament: {:#?}", e.sql_err());
                    GenericError::UnknownError("Unable to insert fantasy tournament")
                }
            })?;
        UserInFantasyTournament::insert(user_in_fantasy_tournament::ActiveModel {
            id: NotSet,
            user_id: Set(owner_id),
            fantasy_tournament_id: Set(tour.last_insert_id),
            invitation_status: Set(FantasyTournamentInvitationStatus::Accepted),
        })
        .exec(db)
        .await
        .map_err(|e| match e.sql_err() {
            Some(SqlErr::UniqueConstraintViolation(_)) => {
                GenericError::Conflict("User already in tournament")
            }
            Some(SqlErr::ForeignKeyConstraintViolation(_)) => GenericError::NotFound("User not found"),
            _ => {
                error!("Unable to insert user in fantasy tournament: {:#?}", e.sql_err());
                GenericError::UnknownError("Unable to insert user in fantasy tournament")
            }
        })?;
        FantasyTournamentDivs::insert(self.divisions.clone(), db, tour.last_insert_id)
            .await
            .map_err(|e| match e.sql_err() {
                Some(SqlErr::ForeignKeyConstraintViolation(_)) => {
                    GenericError::NotFound("Division not found")
                }
                _ => {
                    error!(
                        "Unable to insert fantasy tournament divisions: {:#?}",
                        e.sql_err()
                    );
                    GenericError::UnknownError("Unable to insert fantasy tournament divisions")
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
        self.insert_competition_in_db(db, level).await?;
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

        let round_models = self
            .rounds
            .iter()
            .sorted_by(|a, b| a.round_number.cmp(&b.round_number))
            .map(|round| {
                let time = times
                    .get(round.round_number - 1)
                    .unwrap_or(times.last().unwrap())
                    .fixed_offset();
                round.active_model(time)
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

    async fn make_sure_all_players_exist_in_db(&self, db: &impl ConnectionTrait) -> Result<(), GenericError> {
        let players = self.get_all_player_active_models();

        player::Entity::insert_many(players)
            .on_conflict(
                OnConflict::column(player::Column::PdgaNumber)
                    .do_nothing()
                    .to_owned(),
            )
            .do_nothing()
            .exec(db)
            .await
            .map_err(|_| {
                error!("Unable to insert players into database");
                GenericError::UnknownError("Unable to insert players into database")
            })?;

        let player_divs = self.get_all_player_divisions(1);
        let mut players_in_comp = Vec::new();
        for player in &player_divs {
            let player_in_comp = player_in_competition::ActiveModel {
                id: NotSet,
                pdga_number: player.player_pdga_number.to_owned(),
                competition_id: Set(self.competition_id as i32),
                division: player.division.to_owned(),
            };
            players_in_comp.push(player_in_comp);
        }
        player_in_competition::Entity::insert_many(players_in_comp)
            .on_conflict(
                OnConflict::columns(vec![
                    player_in_competition::Column::PdgaNumber,
                    player_in_competition::Column::CompetitionId,
                ])
                .do_nothing()
                .to_owned(),
            )
            .do_nothing()
            .exec(db)
            .await
            .map_err(|_| {
                error!("Unable to insert players into competition");
                GenericError::UnknownError("Unable to insert players into competition")
            })?;

        player_division_in_fantasy_tournament::Entity::insert_many(player_divs)
            .on_conflict(
                OnConflict::column(player_division_in_fantasy_tournament::Column::PlayerPdgaNumber)
                    .do_nothing()
                    .to_owned(),
            )
            .do_nothing()
            .exec(db)
            .await
            .map_err(|_| {
                error!("Unable to insert player divisions into fantasy tournament");
                GenericError::UnknownError("Unable to insert player divisions into fantasy tournament")
            })?;
        Ok(())
    }

    pub async fn save_round_scores(&self, db: &impl ConnectionTrait) -> Result<(), GenericError> {
        // TODO: ADD STATUS TO ROUND

        self.make_sure_all_players_exist_in_db(db).await?;

        let player_round_scores = self
            .rounds
            .iter()
            .filter(|r| r.status() != RoundStatus::Pending)
            .flat_map(|r| {
                r.all_player_round_score_active_models(r.round_number as i32, self.competition_id as i32)
                    .into_iter()
            })
            .collect_vec();
        if !player_round_scores.is_empty() {
            super::super::update_or_insert_many_player_round_scores(db, player_round_scores).await
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
        let players = self.get_current_player_scores();
        add_players(db, players, fantasy_tournament_id).await?;
        Ok(())
    }

    pub async fn save_user_scores(
        &self,
        db: &impl ConnectionTrait,
        fantasy_tournament_id: u32,
    ) -> Result<(), GenericError> {
        self.make_sure_all_players_exist_in_db(db).await?;
        let mut user_scores = self.get_user_scores(db, fantasy_tournament_id).await?;
        if !user_scores.is_empty() {
            user_competition_score_in_fantasy_tournament::Entity::delete_many()
                .filter(
                    user_competition_score_in_fantasy_tournament::Column::FantasyTournamentId
                        .eq(fantasy_tournament_id as i32)
                        .and(
                            user_competition_score_in_fantasy_tournament::Column::CompetitionId
                                .eq(self.competition_id as i32),
                        ),
                )
                .exec(db)
                .await
                .map_err(|e| {
                    error!("Unable to delete user scores from competition {:#?}", e);
                    GenericError::UnknownError("Unable to delete user scores from competition")
                })?;
            user_scores.dedup_by(|a, b| a.pdga_num == b.pdga_num);
            for score in &user_scores {
                if score.pdga_num == 91249 {}
            }
            let mut new_scores: Vec<UserScore> = Vec::new();
            for score in &user_scores {
                if !new_scores
                    .iter()
                    .any(|new_score| new_score.pdga_num == score.pdga_num)
                {
                    new_scores.push(score.clone());
                }
            }

            let scores = new_scores
                .into_iter()
                .map(|p| p.into_active_model(self.competition_id as i32))
                .dedup()
                .collect_vec();
            user_competition_score_in_fantasy_tournament::Entity::insert_many(scores)
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
