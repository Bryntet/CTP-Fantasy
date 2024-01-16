use bcrypt::{DEFAULT_COST, hash};
use rocket::http::CookieJar;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, NotSet, TransactionTrait};
use sea_orm::ActiveValue::Set;
use entity::fantasy_pick;
use entity::prelude::{FantasyScores, FantasyTournament, User, UserAuthentication, UserInFantasyTournament};
use entity::sea_orm_active_enums::FantasyTournamentInvitationStatus;
use crate::error::{GenericError, PlayerError};
use crate::generate_cookie;

use super::*;





impl FantasyPick {
    pub async fn insert_or_change(&self, db: &DatabaseConnection, user_id: i32) -> Result<(), GenericError> {
        use entity::prelude::FantasyPick as FantasyPickEntity;
        use sea_orm::{ColumnTrait, NotSet, QueryFilter, Set};

        let existing_pick = FantasyPickEntity::find()
            .filter(fantasy_pick::Column::PickNumber.eq(self.slot).and(fantasy_pick::Column::FantasyTournamentId.eq(self.fantasy_tournament_id)).and(fantasy_pick::Column::User.eq(user_id)))
            .one(db)
            .await?;

        if !crate::player_exists(db, self.pdga_number).await {
            Err::<(), GenericError>(PlayerError::PlayerNotFound("Unknown player id").into())?;
        }
        match existing_pick {
            Some(pick) => {
                let mut pick: fantasy_pick::ActiveModel = pick.into();
                pick.player = Set(self.pdga_number);
                pick.update(db).await?;
            }
            None => {
                let new_pick = fantasy_pick::ActiveModel {
                    id: NotSet,
                    user: Set(user_id),
                    pick_number: Set(self.slot),
                    player: Set(self.pdga_number),
                    fantasy_tournament_id: Set(self.fantasy_tournament_id),
                    division: Set(crate::get_player_division(db, self.pdga_number)
                        .await?
                        .first()
                        .unwrap()
                        .to_owned()),
                };
                new_pick.insert(db).await?;
            }
        }
        Ok(())
    }
}


impl UserLogin {
    pub async fn insert<'a>(&'a self, db: &'a DatabaseConnection, cookies: &CookieJar<'_>) -> Result<(), sea_orm::error::DbErr> {
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
        UserInFantasyTournament::insert(
            user_in_fantasy_tournament::ActiveModel {
                id: NotSet,
                user_id: Set(owner_id),
                fantasy_tournament_id: Set(tour.last_insert_id),
                invitation_status: Set(FantasyTournamentInvitationStatus::Accepted),
            },
        ).exec(db).await?;
        Ok(())
    }
}