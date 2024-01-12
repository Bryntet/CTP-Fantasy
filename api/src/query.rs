use sea_orm::DatabaseConnection;
use crate::error::{Error, TournamentError};
use crate::error::UserError;
use rocket::serde::json::Json;
use rocket::State;
use rocket_okapi::{openapi};

use crate::authenticate;
use service::SimpleFantasyTournament;


#[openapi(tag = "Fantasy Tournament")]
#[get("/my-tournaments")]
pub(crate) async fn see_tournaments(
    db: &State<DatabaseConnection>,
    user: authenticate::CookieAuth,
) -> Result<Json<Vec<SimpleFantasyTournament>>, Error> {
    let user_model = user.to_user_model(db.inner()).await?;
    match service::get_fantasy_tournaments(db.inner(), user_model.id).await {

        Ok(tournaments) => {
            Ok(Json(tournaments))
        },
        Err(_) => Err(TournamentError::NotFound.into()),
    }
}


#[openapi(tag = "Fantasy Tournament")]
#[get("/fantasy-tournament/<id>")]
pub(crate) async fn get_tournament(
    db: &State<DatabaseConnection>,
    id: i32,
) -> Result<Json<SimpleFantasyTournament>, Error> {
    match service::get_fantasy_tournament(db.inner(), id).await {
        Ok(Some(tournament)) => Ok(Json(tournament)),
        Ok(None) => Err(TournamentError::NotFound.into()),
        Err(_) => Err(TournamentError::NotFound.into()),
    }
}

#[openapi(tag = "Fantasy Tournament")]
#[get("/fantasy-tournament/<id>/participants")]
pub(crate) async fn see_participants(
    db: &State<DatabaseConnection>,
    id: i32,
) -> Result<Json<Vec<service::SimpleUser>>, Error> {
    match service::get_participants(db.inner(), id).await {
        Ok(participants) => Ok(Json(participants)),
        Err(_) => Err(UserError::InvalidUserId.into()),
    }
}



#[openapi(tag = "Fantasy Tournament")]
#[get("/fantasy-tournament/<tournament_id>/user_picks/<user_id>")]
pub(crate) async fn get_user_picks(
    db: &State<DatabaseConnection>,
    requester: authenticate::CookieAuth,
    tournament_id: i32,
    user_id: i32,
) -> Result<Json<service::SimpleFantasyPicks>, Error> {
    match service::get_user_picks_in_tournament(db.inner(), requester.to_user_model(db.inner()).await?, user_id, tournament_id).await {
        Ok(picks) => Ok(Json(picks)),
        Err(_) => Err(UserError::InvalidUserId.into()),
    }
}


#[openapi(tag = "User")]
#[get("/my-id")]
pub(crate) async fn get_my_id(
    db: &State<DatabaseConnection>,
    user: authenticate::CookieAuth,
) -> Result<Json<i32>, Error> {
    let user_model = user.get_user(db.inner()).await?;
    if let Some(user_model) = user_model {
        dbg!(Json(&user_model.id));
        Ok(Json(user_model.id))
    } else {
        Err(UserError::InvalidUserId.into())
    }
}