use crate::query::get_fantasy_tournament_model;
use crate::{
    error::GenericError, get_competitions_in_fantasy_tournament, get_user_participants_in_tournament,
};
use chrono::{DateTime, Days, Duration, FixedOffset, Timelike};
use chrono_tz::Tz;
use entity::sea_orm_active_enums::CompetitionStatus;
use rocket::error;
use sea_orm::prelude::Time;
use sea_orm::ConnectionTrait;
use std::ops::Add;

pub async fn is_user_allowed_to_exchange(
    db: &impl ConnectionTrait,
    user_id: i32,
    tournament_id: i32,
) -> Result<bool, GenericError> {
    if let Some(tournament) = get_fantasy_tournament_model(db, tournament_id).await? {
        let users = see_which_users_can_exchange(db, &tournament).await?;
        Ok(!any_competitions_running(db, tournament.id).await? && users.iter().any(|u| u.user.id == user_id))
    } else {
        Err(GenericError::NotFound("Tournament not found"))
    }
}

pub async fn any_competitions_running(
    db: &impl ConnectionTrait,
    tournament_id: i32,
) -> Result<bool, GenericError> {
    let comps = get_competitions_in_fantasy_tournament(db, tournament_id).await?;
    Ok(comps.iter().any(|c| c.status == CompetitionStatus::Running))
}

pub async fn see_which_users_can_exchange(
    db: &impl ConnectionTrait,
    tournament: &entity::fantasy_tournament::Model,
) -> Result<Vec<crate::dto::UserWithScore>, GenericError> {
    let first_exchange_window = get_first_exchange_window_time(db, tournament).await?;

    let now = chrono::Utc::now();
    let mut users = get_sorted_users(db, tournament.id).await?;
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
        if let Some(user) = users.pop() {
            allowed_users.push(user);
        }
        possible_exchange_window_time = get_next_exchange_window_time(exchange_time).await;
    }
    Ok(allowed_users)
}

pub async fn has_exchange_begun(db: &impl ConnectionTrait, tournament_id: i32) -> Result<bool, GenericError> {
    if let Some(tournament) = get_fantasy_tournament_model(db, tournament_id).await? {
        let first_exchange_window = get_first_exchange_window_time(db, &tournament).await?;
        error!("{:#?}", &first_exchange_window);
        Ok(first_exchange_window
            .map(|x| x < chrono::Utc::now())
            .unwrap_or(false))
    } else {
        Err(GenericError::NotFound("Tournament not found"))
    }
}

async fn get_first_exchange_window_time(
    db: &impl ConnectionTrait,
    tournament: &entity::fantasy_tournament::Model,
) -> Result<Option<DateTime<FixedOffset>>, GenericError> {
    let comps = get_competitions_in_fantasy_tournament(db, tournament.id).await?;

    if comps.iter().any(|c| c.status == CompetitionStatus::Running) {
        Ok(None)
    } else if let Some(end_time) = comps.iter().filter_map(|c| c.ended_at).max() {
        // If ends at hour 6, assume that it ended due to the time restriction!
        let add_day = end_time.hour() != 6;
        let first_exchange_time: Option<DateTime<FixedOffset>> = end_time
            .with_timezone(&Tz::Europe__Stockholm)
            .fixed_offset()
            .checked_add_days(if add_day { Days::new(1) } else { Days::new(0) })
            .expect("Able to add one day")
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
) -> Result<Option<DateTime<FixedOffset>>, GenericError> {
    Ok(next_competition_beginning(db, tournament).await?.map(|x| {
        x.add(Duration::try_days(-1).unwrap())
            .with_hour(20)
            .unwrap()
            .with_minute(0)
            .unwrap()
            .with_second(0)
            .unwrap()
    }))
}

async fn next_competition_beginning(
    db: &impl ConnectionTrait,
    tournament: &entity::fantasy_tournament::Model,
) -> Result<Option<DateTime<FixedOffset>>, GenericError> {
    let comps = get_competitions_in_fantasy_tournament(db, tournament.id).await?;
    let next_competition = comps
        .iter()
        .filter(|comp| comp.status == CompetitionStatus::NotStarted)
        .min_by_key(|c| c.start_date);
    Ok(next_competition.map(|c| c.start_date.and_time(Time::default()).and_utc().fixed_offset()))
}

pub async fn see_when_users_can_exchange(
    db: &impl ConnectionTrait,
    tournament: i32,
) -> Result<Vec<(crate::dto::UserWithScore, DateTime<FixedOffset>)>, GenericError> {
    let tournament = get_fantasy_tournament_model(db, tournament)
        .await?
        .ok_or(GenericError::NotFound("Tournament not found"))?;
    let first_exchange_window = get_first_exchange_window_time(db, &tournament).await?;

    let mut users = get_sorted_users(db, tournament.id).await?;
    let mut possible_exchange_window_time = first_exchange_window;
    let mut user_exchange_times = Vec::new();

    while let Some(exchange_time) = possible_exchange_window_time {
        if let Some(user) = users.pop() {
            user_exchange_times.push((user, exchange_time));
        } else {
            break;
        }
        possible_exchange_window_time = get_next_exchange_window_time(exchange_time).await;
    }
    Ok(user_exchange_times)
}

async fn get_sorted_users(
    db: &impl ConnectionTrait,
    tournament_id: i32,
) -> Result<Vec<crate::dto::UserWithScore>, GenericError> {
    let mut users = get_user_participants_in_tournament(db, tournament_id).await?;
    users.sort_by(|a, b| b.score.cmp(&a.score));
    Ok(users)
}

async fn get_next_exchange_window_time(
    exchange_time: DateTime<FixedOffset>,
) -> Option<DateTime<FixedOffset>> {
    if exchange_time.hour() == 20 {
        exchange_time
            .add(Duration::try_days(1).unwrap())
            .with_hour(8)
            .and_then(|x| x.with_minute(0).and_then(|x| x.with_second(0)))
    } else {
        Some(exchange_time.add(Duration::try_hours(4).unwrap()))
    }
}
