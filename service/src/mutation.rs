use bcrypt::{hash, DEFAULT_COST};
use rocket::http::{Cookie, CookieJar};
use entity::prelude::*;
use entity::*;
use fantasy_tournament::Entity as FantasyTournament;
use rand::distributions::Alphanumeric;
use rand::Rng;
use sea_orm::ActiveValue::*;
use sea_orm::{ActiveModelTrait, DatabaseConnection, DbErr, EntityTrait, IntoActiveModel, TransactionTrait};
use serde::Deserialize;

use rocket_okapi::okapi::schemars;
use rocket_okapi::okapi::schemars::JsonSchema;
use entity::sea_orm_active_enums::FantasyTournamentInvitationStatus;
use sea_orm::{QueryFilter, ColumnTrait};
use crate::get_player_division;

#[derive(Deserialize, JsonSchema)]
pub struct CreateTournamentInput {
    pub name: String,
    pub max_picks_per_user: Option<i32>,
}

impl CreateTournamentInput {
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

/*pub struct CreatePickInput {
    pub user: i32,
    pub player: i32,
    pub fantasy_tournament_id: i32,
    pub
}

impl CreatePickInput {
    pub fn into_active_model(self) -> fantasy_pick::ActiveModel {
        fantasy_pick::ActiveModel {
            id: NotSet,
            user: Set(self.user),
            player: Set(self.player),
            fantasy_tournament_id: Set(self.fantasy_tournament_id),
            create
        }
    }
    pub async fn insert(self, db: &DatabaseConnection) -> Result<(), sea_orm::error::DbErr> {
        FantasyPick::insert(self.into_active_model())
            .exec(db)
            .await?;
        Ok(())
    }
}*/

pub struct CreateUserScoreInput {
    pub user: i32,
    pub score: i32,
    pub ranking: i32,
    pub fantasy_tournament_id: i32,
}

impl CreateUserScoreInput {
    pub fn into_active_model(self) -> fantasy_scores::ActiveModel {
        fantasy_scores::ActiveModel {
            id: NotSet,
            user: Set(self.user),
            score: Set(self.score),
            ranking: Set(self.ranking),
            fantasy_tournament_id: Set(self.fantasy_tournament_id),
        }
    }
    pub async fn insert(self, db: &DatabaseConnection) -> Result<(), sea_orm::error::DbErr> {
        FantasyScores::insert(self.into_active_model())
            .exec(db)
            .await?;
        Ok(())
    }
}

#[derive(Deserialize, JsonSchema, Debug, Clone)]
pub struct CreateUserInput {
    pub username: String,
    pub password: String,
}

impl CreateUserInput {
    fn active_user(&self) -> user::ActiveModel {
        user::ActiveModel {
            id: NotSet,
            name: Set(self.username.clone()),
        }
    }
    fn active_authentication(
        &self,
        hashed_password: String,
        user_id: i32,
    ) -> user_authentication::ActiveModel {
        user_authentication::ActiveModel {
            user_id: Set(user_id),
            hashed_password: Set(hashed_password),
        }
    }
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


pub async fn generate_cookie(
    db: &DatabaseConnection,
    user_id: i32,
    cookies: &CookieJar<'_>
) -> Result<(), DbErr> {
    let random_value: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect();


    let user_cookie = user_cookies::ActiveModel {
        user_id: Set(user_id),
        cookie: Set(random_value.clone()),
    };
    UserCookies::insert(user_cookie).exec(db).await?;

    let cookie: Cookie<'static> = Cookie::build(("auth".to_string(), random_value.clone()))
        .secure(true)
        .same_site(rocket::http::SameSite::None)
        .finish();

    cookies.add(cookie);
    Ok(())
}

pub enum InviteError {
    UserNotFound,
    TournamentNotFound,
    NotOwner,
}


pub async fn create_invite(
    db: &DatabaseConnection,
    sender: user::Model,
    receiver_name: String,
    fantasy_tournament_id: i32,
) -> Result<(), InviteError> {




    let tournament = if let Ok(Some(t))= FantasyTournament::find_by_id(fantasy_tournament_id).one(db).await {
        t
    } else {
        return Err(InviteError::TournamentNotFound);
    };

    if tournament.owner != sender.id {
        return Err(InviteError::NotOwner);
    }
    let invited_user = if let Ok(Some(u)) = crate::get_user_by_name(db, receiver_name).await {
        u
    } else {
        return Err(InviteError::UserNotFound);
    };
    let invite = user_in_fantasy_tournament::ActiveModel {
        id: NotSet,
        fantasy_tournament_id: Set(fantasy_tournament_id),
        user_id: Set(invited_user.id),
        invitation_status: Set(FantasyTournamentInvitationStatus::Pending),
    };

    if (user_in_fantasy_tournament::Entity::insert(invite).exec(db).await).is_ok() {
        Ok(())
    } else {
        Err(InviteError::UserNotFound)
    }
}
pub async fn answer_invite(
    db: &DatabaseConnection,
    user: user::Model,
    fantasy_tournament_id: i32,
    invitation_status: bool
) -> Result<(), InviteError> {
    let mut invite = if let Ok(Some(i)) = UserInFantasyTournament::find().filter(
        user_in_fantasy_tournament::Column::FantasyTournamentId
            .eq(fantasy_tournament_id)
            .and(user_in_fantasy_tournament::Column::UserId.eq(user.id)),
    ).one(db).await {
        i.into_active_model()
    } else {
        return Err(InviteError::UserNotFound);
    };
    invite.invitation_status = Set(if invitation_status {
        FantasyTournamentInvitationStatus::Accepted
    } else {
        FantasyTournamentInvitationStatus::Declined
    });

    if invite.save(db).await.is_ok() {
        Ok(())
    } else {
        Err(InviteError::UserNotFound)
    }
}



pub async fn add_picks_to_tournament(
    db: &DatabaseConnection,
    user: user::Model,
    picks: Vec<FantasyPick>,
    fantasy_tournament_id: i32,
) -> Result<(), DbErr> {
    Ok(())
}

