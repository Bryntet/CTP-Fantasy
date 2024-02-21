use crate::query::get_fantasy_tournament_model;
use crate::{
    error::GenericError, get_competitions_in_fantasy_tournament, get_user_participants_in_tournament,
};
use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use entity::sea_orm_active_enums::CompetitionStatus;
use sea_orm::ConnectionTrait;
use std::ops::Add;
use log::info;
use rocket::error;

pub async fn is_user_allowed_to_exchange(
    db: &impl ConnectionTrait,
    user_id: i32,
    tournament_id: i32,
) -> Result<bool, GenericError> {
    if let Some(tournament) = get_fantasy_tournament_model(db, tournament_id).await? {

        let users = see_which_users_can_exchange(db, &tournament).await?;
        Ok(!any_competitions_running(db, &tournament).await? || users.iter().any(|u| u.id == user_id))
    } else {
        Err(GenericError::NotFound("Tournament not found"))
    }
}

pub async fn any_competitions_running(
    db: &impl ConnectionTrait,
    tournament: &entity::fantasy_tournament::Model,
) -> Result<bool, GenericError> {
    let comps = get_competitions_in_fantasy_tournament(db, tournament.id).await?;
    Ok(comps.iter().any(|c| c.status == CompetitionStatus::Running))
}

pub async fn see_which_users_can_exchange(
    db: &impl ConnectionTrait,
    tournament: &entity::fantasy_tournament::Model,
) -> Result<Vec<crate::dto::User>, GenericError> {
    let first_exchange_window = get_first_exchange_window_time(db, tournament).await?;

    let now = chrono::Utc::now().naive_local();
    let mut users = get_user_participants_in_tournament(db, tournament.id).await?;
    // Sort by score
    users.sort_by(|a, b| a.score.cmp(&b.score));
    let mut possible_exchange_window_time = first_exchange_window;
    let mut allowed_users = Vec::new();
    let time_to_allow_all = last_possible_exchange_window_time(db, tournament).await?;
    if let Some(time_to_allow_all) = time_to_allow_all {
        if now > time_to_allow_all {
            return Ok(users);
        }
    }
    while let Some(exchange_time) = possible_exchange_window_time {
        if now < exchange_time {
            break;
        }
        // Use the users with worst score first
        if let Some(user) = users.pop() {
            allowed_users.push(user);
        }
        if exchange_time.hour() == 20 {
            possible_exchange_window_time = exchange_time
                .add(Duration::days(1))
                .with_hour(8)
                .and_then(|x| x.with_minute(0).and_then(|x| x.with_second(0)))
        } else {
            possible_exchange_window_time = Some(exchange_time.add(Duration::hours(4)));
        }
    }
    Ok(allowed_users)
}

pub async fn has_exchange_begun(db: &impl ConnectionTrait, tournament_id: i32) -> Result<bool, GenericError> {
    if let Some(tournament) = get_fantasy_tournament_model(db, tournament_id).await? {
        let first_exchange_window = get_first_exchange_window_time(db, &tournament).await?;
        error!("{:#?}",&first_exchange_window);
        Ok(first_exchange_window
            .map(|x| x < chrono::Utc::now().naive_local())
            .unwrap_or(false))
    } else {
        Err(GenericError::NotFound("Tournament not found"))
    }
}

async fn get_first_exchange_window_time(
    db: &impl ConnectionTrait,
    tournament: &entity::fantasy_tournament::Model,
) -> Result<Option<NaiveDateTime>, GenericError> {
    let comps = get_competitions_in_fantasy_tournament(db, tournament.id).await?;

    if comps.iter().any(|c| c.status == CompetitionStatus::Running) {
        Ok(None)
    } else if let Some(end_time) = comps.iter().filter_map(|c| c.ended_at).max() {
        let first_exchange_time: Option<NaiveDateTime> = end_time
            .add(Duration::days(1))
            .naive_local()
            .with_hour(8)
            .and_then(|x| x.with_minute(0).and_then(|x| x.with_second(0)));
        Ok(first_exchange_time)
    } else {
        Ok(None)
    }
}

async fn last_possible_exchange_window_time(
    db: &impl ConnectionTrait,
    tournament: &entity::fantasy_tournament::Model,
) -> Result<Option<NaiveDateTime>, GenericError> {
    Ok(next_competition_beginning(db, tournament).await?.map(|x| {
        x.add(Duration::days(-1))
            .and_time(NaiveTime::from_hms_opt(20, 0, 0).unwrap())
    }))
}

async fn next_competition_beginning(
    db: &impl ConnectionTrait,
    tournament: &entity::fantasy_tournament::Model,
) -> Result<Option<NaiveDate>, GenericError> {
    let comps = get_competitions_in_fantasy_tournament(db, tournament.id).await?;
    let next_competition = comps
        .iter()
        .filter(|comp| comp.status == CompetitionStatus::NotStarted)
        .min_by_key(|c| c.start_date);
    Ok(next_competition.map(|c| c.start_date))
}
