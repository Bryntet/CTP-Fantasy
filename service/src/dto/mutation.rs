use std::fmt::Display;
use crate::error::GenericError;
use crate::error::PlayerError;
use crate::{generate_cookie, get_player_division, player_exists};
use bcrypt::{hash, DEFAULT_COST};
use entity::fantasy_pick;
use entity::prelude::{
    FantasyScores, FantasyTournament, User, UserAuthentication, UserInFantasyTournament,
};
use entity::sea_orm_active_enums::FantasyTournamentInvitationStatus;
use rocket::http::CookieJar;
use rocket::request::FromParam;
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ConnectionTrait, DatabaseConnection, EntityTrait, IntoActiveModel, NotSet, TransactionTrait};

use super::*;

impl FantasyPick {
    pub async fn change_or_insert<C>(
        &self,
        db: &C,
        user_id: i32,
        tournament_id: i32,
        div: Division,
    ) -> Result<(), GenericError>
    where
        C: ConnectionTrait,
    {

        if player_exists(db, self.pdga_number).await {
            if get_player_division(db, self.pdga_number).await?.contains(div.clone().into()) {
                let person_in_slot =
                    Self::player_in_slot(db, user_id, tournament_id, self.slot, div.into()).await?;

                if let Some(player) = person_in_slot {
                    let player: fantasy_pick::ActiveModel = player.into();
                    player.delete(db).await?;
                }

                if let Some(player) =
                    Self::player_already_chosen(db, user_id, tournament_id, self.pdga_number).await?
                {
                    let player: fantasy_pick::ActiveModel = player.into();
                    player.delete(db).await?;
                }
                self.insert(db, user_id, tournament_id).await?;
                Ok(())
            } else {
                Err(PlayerError::WrongDivision.into())
            }

        } else {
            Err(PlayerError::NotFound.into())
        }
    }

    async fn insert<C>(&self, db: &C, user_id: i32, tournament_id: i32) -> Result<(), GenericError>
    where
        C: ConnectionTrait,
    {
        match crate::get_player_division(db, self.pdga_number).await {
            Ok(division) => {
                let division = division
                    .first()
                    .unwrap_or(Division::MPO.into())
                    .to_owned();
                let pick = fantasy_pick::ActiveModel {
                    id: NotSet,
                    user: Set(user_id),
                    pick_number: Set(self.slot),
                    player: Set(self.pdga_number),
                    fantasy_tournament_id: Set(tournament_id),
                    division: Set(division),
                };
                pick.save(db).await?;
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }
}

impl From<Division> for &sea_orm_active_enums::Division {
    fn from(division: Division) -> Self {
        match division {
            Division::MPO => &sea_orm_active_enums::Division::Mpo,
            Division::FPO => &sea_orm_active_enums::Division::Fpo,
        }
    }
}
impl From<Division > for sea_orm_active_enums::Division {
    fn from(division: Division) -> Self {
        match division {
            Division::MPO => sea_orm_active_enums::Division::Mpo,
            Division::FPO => sea_orm_active_enums::Division::Fpo,
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
            _ => Division::MPO,
        }
    }
}

impl Display for Division {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Division::MPO => "Mpo".to_string(),
            Division::FPO => "Fpo".to_string(),
        };
        write!(f, "{}", str)
    }
}


impl UserLogin {
    pub async fn insert<'a>(
        &'a self,
        db: &'a DatabaseConnection,
        cookies: &CookieJar<'_>,
    ) -> Result<(), sea_orm::error::DbErr> {
        let txn = db.begin().await?;
        let user = self.active_user();
        let user_id = User::insert(user).exec(&txn).await?.last_insert_id;
        let hashed_password = hash(&self.password, DEFAULT_COST).unwrap();
        let authentication = self.active_authentication(hashed_password, user_id);
        UserAuthentication::insert(authentication)
            .exec(&txn)
            .await?;
        txn.commit().await?;
        generate_cookie(db, user_id, cookies).await
    }
}

impl UserScore {
    pub async fn insert(self, db: &DatabaseConnection) -> Result<(), sea_orm::error::DbErr> {
        FantasyScores::insert(self.into_active_model())
            .exec(db)
            .await?;
        Ok(())
    }
}

impl CreateTournament {
    pub async fn insert(
        &self,
        db: &DatabaseConnection,
        owner_id: i32,
    ) -> Result<(), sea_orm::error::DbErr> {
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
    ) -> Result<(), sea_orm::error::DbErr> {
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