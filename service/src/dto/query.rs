use super::*;
use crate::error::GenericError;
use entity::prelude::{Player, Round};
use entity::{user, user_authentication};
use sea_orm::ActiveValue::Set;
use sea_orm::{ConnectionTrait, DatabaseConnection, EntityTrait, ModelTrait, NotSet};
use sea_orm::prelude::Date;
use crate::CompetitionInfoInput;
use crate::dto::pdga::CompetitionInfo;
use sea_orm::ColumnTrait;
use sea_orm::DbErr;
use sea_orm::QueryFilter;
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
    pub fn into_active_model(self) -> fantasy_scores::ActiveModel {
        fantasy_scores::ActiveModel {
            id: NotSet,
            user: Set(self.user),
            score: Set(self.score),
            ranking: Set(self.ranking),
            fantasy_tournament_id: Set(self.fantasy_tournament_id),
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
        use sea_orm::{ColumnTrait, QueryFilter};
        let existing_pick = FantasyPickEntity::find()
            .filter(
                fantasy_pick::Column::PickNumber
                    .eq(slot)
                    .and(fantasy_pick::Column::FantasyTournamentId.eq(fantasy_tournament_id))
                    .and(fantasy_pick::Column::User.eq(user_id))
                    .and(fantasy_pick::Column::Division.eq(division)),
            )
            .one(db)
            .await?;
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
        use sea_orm::{ColumnTrait, QueryFilter};
        let existing_pick = FantasyPickEntity::find()
            .filter(
                fantasy_pick::Column::Player
                    .eq(pdga_number)
                    .and(fantasy_pick::Column::FantasyTournamentId.eq(fantasy_tournament_id))
                    .and(fantasy_pick::Column::User.eq(user_id)),
            )
            .one(db)
            .await?;
        Ok(existing_pick)
    }
}



impl CompetitionInfo {
    pub(crate) fn active_model(&self) -> competition::ActiveModel {
        competition::ActiveModel {
            id: Set(self.competition_id as i32),
            status: Set(sea_orm_active_enums::CompetitionStatus::NotStarted),
            name: Set(self.name.clone()),
            rounds: Default::default(),
        }
    }

    pub(crate) fn round_active_model(&self, date: sea_orm::prelude::Date) -> round::ActiveModel {
        round::ActiveModel {
            id: NotSet,
            round_number: sea_orm::Set(1),
            competition_id: sea_orm::Set(self.competition_id as i32),
            date: sea_orm::Set(date),
        }
    }

    pub(crate) async fn round<C>(&self, db: &C, date: Date) -> Result<Option<round::Model>, DbErr> where C: ConnectionTrait {
        Round::find().filter(entity::round::Column::Date.eq::<Date>(date).and(entity::round::Column::CompetitionId.eq(self.competition_id as i32)))
            .one(db)
            .await
    }
}