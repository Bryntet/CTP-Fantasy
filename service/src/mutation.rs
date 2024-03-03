use itertools::Itertools;
use entity::prelude::*;
use entity::sea_orm_active_enums::{CompetitionStatus, FantasyTournamentInvitationStatus};
use entity::*;
use fantasy_tournament::Entity as FantasyTournament;
use log::{error, warn};
use rand::distributions::Alphanumeric;
use rand::Rng;
use rocket::http::{Cookie, CookieJar};
use sea_orm::ActiveValue::*;
use sea_orm::{
    sea_query, ActiveModelTrait, ConnectionTrait, DatabaseConnection, EntityTrait, IntoActiveModel,
    ModelTrait, TransactionTrait,
};
use sea_orm::{ColumnTrait, QueryFilter};

use crate::dto::traits::InsertCompetition;
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
    UserCookies::insert(user_cookie)
        .exec(db)
        .await
        .map_err(|_| GenericError::UnknownError("unable to insert cookie in database"))?;

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
    receiver_name: String,
    fantasy_tournament_id: i32,
) -> Result<(), InviteError> {
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
    user: &user::Model,
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

pub async fn update_active_competitions(db: &DatabaseConnection) -> Result<(), GenericError> {
    let competitions = query::active_competitions(db).await?;

    for competition in competitions {
        if let Ok(txn) = db.begin().await.map_err(|e| {
            warn!("Unable to start transaction: {:#?}", e);
        }) {
            let _ = competition.save_round_scores(&txn).await;
            let _ = txn.commit().await.map_err(|e| {
                warn!("Unable to commit transaction: {:#?}", e);
            });
        }
    }

    Ok(())
}

// TODO: Refactor out the saving to DB
pub async fn refresh_user_scores_in_fantasy(
    db: &impl ConnectionTrait,
    fantasy_tournament_id: u32,
) -> Result<(), GenericError> {
    let competition_ids = crate::get_competitions_in_fantasy_tournament(db, fantasy_tournament_id as i32)
        .await?
        .into_iter()
        .filter(|comp|comp.status==CompetitionStatus::Running)
        .map(|c| c.id as u32)
        .collect_vec();
    
    

    for id in competition_ids {
        match dto::CompetitionInfo::from_web(id).await {
            Err(GenericError::PdgaGaveUp(_)) => {
                tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
                let comp = dto::CompetitionInfo::from_web(id).await?;
                comp.save_user_scores(db, fantasy_tournament_id)
                    .await?;
            }
            Ok(comp) => {
                comp.save_user_scores(db, fantasy_tournament_id)
                    .await?
            }
            Err(e) => Err(e)?,
        }
    }
    Ok(())
}

pub async fn refresh_player_scores_in_active_competitions(
    db: &impl ConnectionTrait,
) -> Result<(), GenericError> {
    let active_comps = crate::get_active_competitions(db).await?;
    for comp in active_comps {
        let comp_info = dto::CompetitionInfo::from_web(comp.id as u32).await?;
        comp_info.save_round_scores(db).await?;
    }
    Ok(())
}

pub async fn refresh_user_scores_in_all(db: &impl ConnectionTrait) -> Result<(), GenericError> {
    let fantasy_tournaments = FantasyTournament::find()
        .all(db)
        .await
        .map_err(|_| GenericError::UnknownError("database error on fantasy tournament"))?;
    for tournament in fantasy_tournaments {
        refresh_user_scores_in_fantasy(db, tournament.id as u32).await?;
    }
    Ok(())
}

/*pub async fn set_activity_status_on_competitions(db: &impl ConnectionTrait) -> Result<(), GenericError> {
    let active_comps = crate::get_active_competitions(db).await?;
    for comp in active_comps {
        CompetitionInfo::from_web(comp.id).await.map(|c|)
    }
}*/

pub async fn update_or_insert_many_player_round_scores(
    db: &impl ConnectionTrait,
    scores: Vec<player_round_score::ActiveModel>,
) -> Result<(), GenericError> {
    player_round_score::Entity::insert_many(scores)
        .on_conflict(
            sea_query::OnConflict::columns([
                player_round_score::Column::PdgaNumber,
                player_round_score::Column::CompetitionId,
                player_round_score::Column::Round,
            ])
            .update_columns([
                player_round_score::Column::Throws,
                player_round_score::Column::Placement,
            ])
            .to_owned(),
        )
        .exec(db)
        .await
        .map_err(|e| {
            error!("Unable to insert player scores into database: {:#?}", e);
            GenericError::UnknownError("Unable to insert player scores into database")
        })?;
    Ok(())
}

pub async fn insert_competition_in_fantasy(
    db: &impl ConnectionTrait,
    fantasy_tournament_id: u32,
    competition_id: u32,
    level: dto::CompetitionLevel,
) -> Result<(), GenericError> {
    match Competition::find_by_id(competition_id as i32)
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
                    CompetitionInFantasyTournament::insert(competition_in_fantasy_tournament::ActiveModel {
                        id: NotSet,
                        competition_id: Set(competition_id as i32),
                        fantasy_tournament_id: Set(fantasy_tournament_id as i32),
                    })
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
        None => {
            let competition = dto::CompetitionInfo::from_web(competition_id).await?;
            competition.insert_in_db(db, level.into()).await?;

            competition.insert_in_fantasy(db, fantasy_tournament_id).await?;
            competition
                .insert_players(db, Some(fantasy_tournament_id as i32))
                .await?;
            Ok(())
        }
    }
}
