use crate::error::GenericError;
use crate::{ApiDivision, generate_cookie};
use bcrypt::{hash, DEFAULT_COST};
use entity::fantasy_pick;
use entity::prelude::{
    FantasyScores, FantasyTournament, User, UserAuthentication, UserInFantasyTournament,
};
use entity::sea_orm_active_enums::FantasyTournamentInvitationStatus;
use rocket::http::CookieJar;
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseConnection, EntityTrait, NotSet, TransactionTrait,
};

use super::*;

impl FantasyPick {
    pub async fn change_or_insert<C>(
        &self,
        db: &C,
        user_id: i32,
        tournament_id: i32,
    ) -> Result<(), GenericError>
    where
        C: ConnectionTrait,
    {
        let person_in_slot = Self::player_in_slot(db, user_id, tournament_id, self.slot).await?;

        if let Some(player) = person_in_slot {
            let player: fantasy_pick::ActiveModel = player.into();
            player.delete(db).await?;
        }

        if let Some(player) = Self::player_already_chosen(db, user_id, tournament_id, self.pdga_number).await?{
            let player: fantasy_pick::ActiveModel = player.into();
            player.delete(db).await?;
        }
        self.insert(db, user_id, tournament_id).await?;
        Ok(())
    }

    async fn insert<C>(&self, db: &C, user_id: i32, tournament_id: i32) -> Result<(), GenericError>
    where
        C: ConnectionTrait,
    {

        match crate::get_player_division(db, self.pdga_number).await {
            Ok(division) => {
                let division = division.first().unwrap_or(ApiDivision::MPO.into()).to_owned();
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

impl From<ApiDivision> for &sea_orm_active_enums::Division {
    fn from(division: ApiDivision) -> Self {
        match division {
            ApiDivision::MPO => &sea_orm_active_enums::Division::Mpo,
            ApiDivision::FPO => &sea_orm_active_enums::Division::Fpo,
        }
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
        self,
        db: &DatabaseConnection,
        owner_id: i32,
    ) -> Result<(), sea_orm::error::DbErr> {
        let tour = FantasyTournament::insert(self.into_active_model(owner_id))
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
        Ok(())
    }
}
