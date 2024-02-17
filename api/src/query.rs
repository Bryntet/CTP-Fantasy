use crate::error::UserError;
use crate::error::{GenericError, TournamentError};
use dto::{FantasyPick, FantasyPicks};
use rocket::serde::json::Json;
use rocket::State;
use rocket_okapi::openapi;
use sea_orm::DatabaseConnection;

use crate::authenticate;
use service::{dto, SimpleFantasyTournament};

#[openapi(tag = "Fantasy Tournament")]
#[get("/my-tournaments")]
pub(crate) async fn see_tournaments(
    db: &State<DatabaseConnection>,
    user: authenticate::UserAuthentication,
) -> Result<Json<Vec<SimpleFantasyTournament>>, GenericError> {
    let user_model = user.to_user_model(db.inner()).await?;
    match service::get_fantasy_tournaments(db.inner(), user_model.id).await {
        Ok(tournaments) => Ok(Json(tournaments)),
        Err(_) => Err(TournamentError::NotFound("Tournament not found").into()),
    }
}

#[openapi(tag = "Fantasy Tournament")]
#[get("/fantasy-tournament/<id>")]
pub(crate) async fn get_tournament(
    db: &State<DatabaseConnection>,
    id: i32,
) -> Result<Json<SimpleFantasyTournament>, GenericError> {
    match service::get_fantasy_tournament(db.inner(), id).await {
        Ok(Some(tournament)) => Ok(Json(tournament)),
        Ok(None) | Err(_) => Err(TournamentError::NotFound("Tournament not found").into()),
    }
}

#[openapi(tag = "Fantasy Tournament")]
#[get("/fantasy-tournament/<id>/users")]
pub(crate) async fn see_participants(
    db: &State<DatabaseConnection>,
    id: i32,
) -> Result<Json<Vec<dto::User>>, GenericError> {
    match service::get_user_participants_in_tournament(db.inner(), id).await {
        Ok(participants) => Ok(Json(participants)),
        Err(_) => Err(UserError::InvalidUserId("Unknown user").into()),
    }
}

#[openapi(tag = "Fantasy Tournament")]
#[get("/fantasy-tournament/<tournament_id>/user/<user_id>/picks/<pick_slot>")]
pub(crate) async fn get_user_pick(
    db: &State<DatabaseConnection>,
    requester: authenticate::UserAuthentication,
    tournament_id: i32,
    user_id: i32,
    pick_slot: i32,
) -> Result<Json<FantasyPick>, GenericError> {
    if requester.to_user_model(db.inner()).await?.id != user_id {
        Err(UserError::NotPermitted("You are not permitted to view this pick").into())
    } else {
        match service::get_user_pick_in_tournament(db.inner(), user_id, tournament_id, pick_slot)
            .await
        {
            Ok(pick) => Ok(Json(pick)),
            Err(_) => Err(GenericError::NotFound("Pick not found")),
        }
    }
}
#[openapi(tag = "Fantasy Tournament")]
#[get("/fantasy-tournament/<tournament_id>/user/<user_id>/picks/div/<division>")]
pub(crate) async fn get_user_picks(
    db: &State<DatabaseConnection>,
    requester: authenticate::UserAuthentication,
    tournament_id: i32,
    user_id: i32,
    division: dto::Division,
) -> Result<Json<FantasyPicks>, GenericError> {
    let res = service::get_user_picks_in_tournament(
        db.inner(),
        &requester.to_user_model(db.inner()).await?,
        user_id,
        tournament_id,
        &division,
    )
    .await;
    //dbg!(&res);
    match res {
        Ok(picks) => Ok(Json(picks)),
        Err(_) => Err(UserError::InvalidUserId("Unknown user").into()),
    }
}

#[openapi(tag = "Fantasy Tournament")]
#[get("/fantasy-tournament/<tournament_id>/divisions")]
pub(crate) async fn get_divisions(
    db: &State<DatabaseConnection>,
    tournament_id: i32,
) -> Result<Json<Vec<dto::Division>>, GenericError> {
    match service::get_tournament_divisions(db.inner(), tournament_id).await {
        Ok(divisions) => Ok(Json(divisions)),
        Err(_) => Err(TournamentError::NotFound("Tournament not found").into()),
    }
}

#[openapi(tag = "User")]
#[get("/my-id")]
pub(crate) async fn get_my_id(
    db: &State<DatabaseConnection>,
    user: authenticate::UserAuthentication,
) -> Result<Json<i32>, GenericError> {
    let user_model = user.get_user(db.inner()).await?;
    if let Some(user_model) = user_model {
        Ok(Json(user_model.id))
    } else {
        Err(UserError::InvalidUserId("Unknown user").into())
    }
}

#[openapi(tag = "Fantasy Tournament")]
#[get("/fantasy-tournament/<tournament_id>/max-picks")]
pub(crate) async fn get_max_picks(
    db: &State<DatabaseConnection>,
    tournament_id: i32,
) -> Result<Json<i32>, GenericError> {
    match service::max_picks(db.inner(), tournament_id).await {
        Ok(max_picks) => Ok(Json(max_picks)),
        Err(_) => Err(TournamentError::NotFound("Tournament not found").into()),
    }
}
