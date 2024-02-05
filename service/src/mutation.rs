use entity::prelude::*;
use entity::sea_orm_active_enums::FantasyTournamentInvitationStatus;
use entity::*;
use fantasy_tournament::Entity as FantasyTournament;
use rand::distributions::Alphanumeric;
use rand::Rng;
use rocket::http::{Cookie, CookieJar};
use sea_orm::ActiveValue::*;
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseConnection, EntityTrait, IntoActiveModel,
    TransactionTrait,
};
use sea_orm::{ColumnTrait, QueryFilter};

use crate::error::{GenericError, InviteError};
use crate::{dto, query};

pub async fn generate_cookie(
    db: &DatabaseConnection,
    user_id: i32,
    cookies: &CookieJar<'_>,
) -> Result<(), GenericError> {
    let random_value: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect();

    let user_cookie = user_cookies::ActiveModel {
        user_id: Set(user_id),
        cookie: Set(random_value.clone()),
    };
    UserCookies::insert(user_cookie).exec(db).await.map_err(|_|GenericError::UnknownError("unable to insert cookie in database"))?;

    #[cfg(debug_assertions)]
    let secure = false;
    #[cfg(not(debug_assertions))]
    let secure = true;

    let cookie: Cookie<'static> = Cookie::build(("auth".to_string(), random_value.clone()))
        .secure(secure)
        .build();

    cookies.add_private(cookie);
    Ok(())
}

pub async fn create_invite(
    db: &DatabaseConnection,
    sender: user::Model,
    receiver_name: String,
    fantasy_tournament_id: i32,
) -> Result<(), InviteError> {
    let tournament = if let Ok(Some(t)) = FantasyTournament::find_by_id(fantasy_tournament_id)
        .one(db)
        .await
    {
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

    if user_in_fantasy_tournament::Entity::insert(invite)
        .exec(db)
        .await
        .is_ok()
    {
        Ok(())
    } else {
        Err(InviteError::UserNotFound)
    }
}
pub async fn answer_invite(
    db: &DatabaseConnection,
    user: user::Model,
    fantasy_tournament_id: i32,
    invitation_status: bool,
) -> Result<(), InviteError> {
    let mut invite = if let Ok(Some(i)) = UserInFantasyTournament::find()
        .filter(
            user_in_fantasy_tournament::Column::FantasyTournamentId
                .eq(fantasy_tournament_id)
                .and(user_in_fantasy_tournament::Column::UserId.eq(user.id)),
        )
        .one(db)
        .await
    {
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

pub async fn update_round(
    db: &impl ConnectionTrait,
    round: round::Model,
) -> Result<(), GenericError> {
    let round_info = dto::RoundInformation::new(
        round.competition_id as usize,
        round.round_number as usize,
        dto::Division::MPO,
    )
    .await
    .map_err(|_| {
        GenericError::UnknownError("Unable to fetch round information")
    })?;

    round_info.update_all(db).await?;
    Ok(())
}

pub async fn update_active_rounds(db: &DatabaseConnection) {
    let rounds = query::active_rounds(db).await.unwrap();
    update_rounds(db, rounds).await;
}

pub async fn update_rounds(db: &DatabaseConnection, rounds: Vec<round::Model>) {
    for round in rounds {
        if let Ok(txn) = db.begin().await {
            let _ = update_round(&txn, round).await;
            txn.commit().await.expect("database failed");
        }
    }
}

pub async fn refresh_user_scores_in_fantasy(
    db: &impl ConnectionTrait,
    fantasy_tournament_id: u32,
) -> Result<(), GenericError> {
    let comp_ids: Vec<u32> =
        crate::get_competitions_in_fantasy_tournament(db, fantasy_tournament_id as i32)
            .await?
            .iter()
            .map(|c| c.id as u32)
            .collect();
    for comp_id in comp_ids {
        #[cfg(debug_assertions)]
        dbg!(comp_id);
        dto::CompetitionInfo::from_web(comp_id)
            .await
            .map_err(|_| {
                GenericError::UnknownError("unable to get competition info from pdga")
            })?
            .save_user_scores(db, fantasy_tournament_id)
            .await?
    }
    Ok(())
}

pub async fn refresh_user_scores_in_all(db: &impl ConnectionTrait) -> Result<(), GenericError> {
    let fantasy_tournaments = FantasyTournament::find().all(db).await.map_err(|_|GenericError::UnknownError("database error on fantasy tournament"))?;
    for tournament in fantasy_tournaments {
        refresh_user_scores_in_fantasy(db, tournament.id as u32).await.map_err(|e|{
            #[cfg(debug_assertions)]
            dbg!(&e);
            e
        })?;
    }
    Ok(())
}
