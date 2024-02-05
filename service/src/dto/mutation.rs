use std::fmt::Display;

use bcrypt::{hash, DEFAULT_COST};
use itertools::Itertools;

use rocket::http::CookieJar;
use rocket::request::FromParam;
use sea_orm::ActiveValue::Set;
use sea_orm::{
    sea_query, ActiveModelTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait,
    ModelTrait, NotSet, TransactionTrait,
};

use entity::fantasy_pick;
use entity::prelude::{
    FantasyTournament, PhantomCompetitionInFantasyTournament, User,
    UserAuthentication, UserCompetitionScoreInFantasyTournament, UserInFantasyTournament,
};
use entity::sea_orm_active_enums::FantasyTournamentInvitationStatus;

use crate::dto::pdga::{add_players};
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
        if player_exists(db, self.pdga_number).await {
            if let Ok(Some(players_division)) =
                super::super::get_player_division_in_tournament(db, self.pdga_number, tournament_id)
                    .await
            {
                if players_division.eq(&div) {
                    let person_in_slot =
                        Self::player_in_slot(db, user_id, tournament_id, self.slot, div.into())
                            .await?;

                    if let Some(player) = person_in_slot {
                        let player: fantasy_pick::ActiveModel = player.into();
                        player.delete(db).await.map_err(|_|GenericError::UnknownError("Unable to remove pick"))?;
                    }

                    if let Some(player) =
                        Self::player_already_chosen(db, user_id, tournament_id, self.pdga_number)
                            .await?
                    {
                        let player: fantasy_pick::ActiveModel = player.into();
                        player.delete(db).await.map_err(|_|GenericError::UnknownError("Unable to remove pick"))?;
                    }
                    self.insert(db, user_id, tournament_id, players_division)
                        .await?;
                    Ok(())
                } else {
                    Err(PlayerError::WrongDivision.into())
                }
            } else {
                Err(PlayerError::WrongDivision.into())
            }
        } else {
            Err(PlayerError::NotFound.into())
        }
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
            division: Set(division.into()),
        };
        pick.save(db).await.map_err(|_|GenericError::UnknownError("Unknown error while saving pick"))?;
        Ok(())
    }
}

impl From<Division> for &sea_orm_active_enums::Division {
    fn from(division: Division) -> Self {
        match division {
            Division::MPO => &sea_orm_active_enums::Division::Mpo,
            Division::FPO => &sea_orm_active_enums::Division::Fpo,
            Division::Unknown => Division::MPO.into(),
        }
    }
}

impl From<Division> for sea_orm_active_enums::Division {
    fn from(division: Division) -> Self {
        match division {
            Division::MPO => sea_orm_active_enums::Division::Mpo,
            Division::FPO => sea_orm_active_enums::Division::Fpo,
            Division::Unknown => Division::MPO.into(),
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
                #[cfg(debug_assertions)]
                dbg!("Unknown division, defaulting to MPO");
                Division::MPO
            }
        }
    }
}

impl Display for Division {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Division::MPO => "Mpo".to_string(),
            Division::FPO => "Fpo".to_string(),
            Division::Unknown => {
                #[cfg(debug_assertions)]
                dbg!("Unknown division, defaulting to MPO");
                "Mpo".to_string()
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
        let txn = db.begin().await.map_err(|_|GenericError::UnknownError("Unable to start transaction"))?;
        let user = self.active_user();
        let user_id = User::insert(user).exec(&txn).await.map_err(|e| {
            e.sql_err().map(|_| {
                GenericError::Conflict("Username already taken")
            }).unwrap_or_else(||
            GenericError::UnknownError("Unable to insert user into database") )
        })?.last_insert_id;
        let hashed_password = hash(&self.password, DEFAULT_COST).expect("hashing should work");
        let authentication = self.active_authentication(hashed_password, user_id);
        UserAuthentication::insert(authentication)
            .exec(&txn)
            .await.map_err(|_|GenericError::UnknownError("Inserting user authentication failed"))?;
        txn.commit().await.map_err(|_|GenericError::UnknownError("Transaction commit failed"))?;
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
    pub async fn insert(&self, db: &DatabaseConnection, owner_id: i32) -> Result<(), DbErr> {
        let tour = FantasyTournament::insert(self.clone().into_active_model(owner_id))
            .exec(db)
            .await?;
        UserInFantasyTournament::insert(user_in_fantasy_tournament::ActiveModel {
            id: NotSet,
            user_id: Set(owner_id),
            fantasy_tournament_id: Set(tour.last_insert_id),
            invitation_status: Set(FantasyTournamentInvitationStatus::Accepted),
        })
        .exec(db)
        .await?;
        FantasyTournamentDivs::insert(self.divisions.clone(), db, tour.last_insert_id).await?;

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
                division: Set(div.into()),
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
            division: Set(self.division.clone().into()),
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
            .await.map_err(|_|GenericError::UnknownError("Unable to insert phantom competition"))?;
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
        self.active_model(level).insert(db).await.map_err(|_|GenericError::UnknownError("Unable to insert competition in database"))?;
        self.insert_rounds(db).await.map_err(|_|GenericError::UnknownError("Unable to insert rounds in database"))?;
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
            .await.map_err(|_|GenericError::UnknownError("Internal error while trying to get competition"))?
        {
            Some(c) => {
                match c
                    .find_related(CompetitionInFantasyTournament)
                    .one(db)
                    .await.map_err(|_|GenericError::UnknownError("Internal db error while trying to find competition in fantasy tournament"))?
                {
                    Some(_) => Err(GenericError::Conflict("Competition already added")),
                    None => {
                        CompetitionInFantasyTournament::insert(
                            self.fantasy_model(fantasy_tournament_id),
                        )
                        .exec(db)
                        .await.map_err(|_|GenericError::UnknownError("Unable to insert competition into fanatasy tournament due to unknown db error"))?;
                        Ok(())
                    }
                }
            }
            None => Err(GenericError::NotFound("Competition not found in PDGA")),
        }
    }
}

impl CompetitionInfo {
    pub async fn insert_rounds(&self, db: &impl ConnectionTrait) -> Result<(), DbErr> {
        use entity::prelude::Round;
        Round::insert_many(
            self.date_range
                .iter()
                .enumerate()
                .map(|(i, d)| self.round_active_model(i + 1, *d)),
        )
        .exec(db)
        .await?;

        Ok(())
    }

    pub async fn insert_players(
        &self,
        db: &impl ConnectionTrait,
        fantasy_tournament_id: Option<i32>,
    ) -> Result<(), GenericError> {
        let players = self.get_all_player_scores().await?;
        add_players(db, players, fantasy_tournament_id).await?;
        Ok(())
    }

    pub async fn save_user_scores(
        &self,
        db: &impl ConnectionTrait,
        fantasy_tournament_id: u32,
    ) -> Result<(), GenericError> {
        let user_scores = self.get_user_scores(db, fantasy_tournament_id).await?;
        #[cfg(debug_assertions)]
        dbg!(&user_scores);
        if !user_scores.is_empty() {
            user_competition_score_in_fantasy_tournament::Entity::insert_many(
                user_scores
                    .into_iter()
                    .map(|p| p.into_active_model(self.competition_id as i32))
                    .dedup_by(|a, b| a.user == b.user),
            )
            .on_conflict(
                sea_query::OnConflict::columns(vec![
                    user_competition_score_in_fantasy_tournament::Column::FantasyTournamentId,
                    user_competition_score_in_fantasy_tournament::Column::User,
                    user_competition_score_in_fantasy_tournament::Column::CompetitionId,
                ])
                .update_column(user_competition_score_in_fantasy_tournament::Column::Score)
                .to_owned(),
            )
            .exec(db)
            .await.map_err(|_|GenericError::UnknownError("Unable to insert user score from competition into database"))?;
        }
        Ok(())
    }
}
