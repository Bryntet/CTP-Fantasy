use crate::error::UserError;
use crate::error::{GenericError, TournamentError};
use dto::{FantasyPick, FantasyPicks};
use rocket::fs::NamedFile;
use rocket::serde::json::Json;
use rocket::State;
use rocket_okapi::openapi;
use sea_orm::DatabaseConnection;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use crate::authenticate;
use service::dto::{Division, UserWithCompetitionScores};
use service::{dto, SimpleFantasyTournament};
use uuid::Uuid;

#[openapi(tag = "Fantasy Tournament")]
#[get("/my-tournaments")]
pub(crate) async fn see_tournaments(
    db: &State<DatabaseConnection>,
    user: authenticate::UserAuthentication,
) -> Result<Json<Vec<SimpleFantasyTournament>>, GenericError> {
    let user_model = user.to_user_model()?;
    match service::get_users_fantasy_tournaments(db.inner(), user_model).await {
        Ok(tournaments) => Ok(Json(tournaments)),
        Err(_) => Err(TournamentError::NotFound("Tournament not found").into()),
    }
}

#[openapi(tag = "Fantasy Tournament")]
#[get("/fantasy-tournament/<id>")]
pub(crate) async fn get_tournament(
    db: &State<DatabaseConnection>,
    auth: authenticate::UserAuthentication,
    id: i32,
) -> Result<Json<SimpleFantasyTournament>, GenericError> {
    if let Ok(model) = auth.to_user_model() {
        if model.admin {
            return match service::get_fantasy_tournament(db.inner(), id, Some(model.id)).await {
                Ok(Some(tournament)) => Ok(Json(tournament)),
                Ok(None) | Err(_) => Err(TournamentError::NotFound("Tournament not found").into()),
            };
        }
    }
    match service::get_fantasy_tournament(db.inner(), id, None).await {
        Ok(Some(tournament)) => Ok(Json(tournament)),
        Ok(None) | Err(_) => Err(TournamentError::NotFound("Tournament not found").into()),
    }
}

#[openapi(tag = "Fantasy Tournament")]
#[get("/fantasy-tournament/<id>/users")]
pub(crate) async fn see_participants(
    db: &State<DatabaseConnection>,
    id: i32,
) -> Result<Json<Vec<dto::UserWithScore>>, GenericError> {
    match service::get_user_participants_in_tournament(db.inner(), id).await {
        Ok(participants) => Ok(Json(participants)),
        Err(_) => Err(UserError::InvalidUserId("Unknown user").into()),
    }
}

#[openapi(tag = "Fantasy Tournament")]
#[get("/fantasy-tournament/<tournament_id>/user/<user_id>/picks/div/<division>/pick/<pick_slot>")]
pub(crate) async fn get_user_pick(
    db: &State<DatabaseConnection>,
    requester: authenticate::UserAuthentication,
    tournament_id: i32,
    user_id: i32,
    pick_slot: i32,
    division: Division,
) -> Result<Json<FantasyPick>, GenericError> {
    if requester.to_user_model()?.id != user_id {
        Err(UserError::NotPermitted("You are not permitted to view this pick").into())
    } else {
        match service::get_user_pick_in_tournament(
            db.inner(),
            user_id,
            tournament_id,
            pick_slot,
            division.into(),
        )
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
        requester.to_user_model()?.id,
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
pub(crate) async fn get_my_id(user: authenticate::UserAuthentication) -> Result<Json<i32>, GenericError> {
    Ok(Json(user.to_user_model()?.id))
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

#[openapi(tag = "Fantasy Tournament")]
#[get("/fantasy-tournament/<tournament_id>/competitions")]
pub(crate) async fn get_competitions(
    db: &State<DatabaseConnection>,
    tournament_id: i32,
) -> Result<Json<Vec<dto::Competition>>, GenericError> {
    Ok(Json(
        dto::Competition::all_in_fantasy_tournament(db.inner(), tournament_id).await?,
    ))
}
#[openapi(tag = "Player")]
#[get("/player/<pdga_number>/image")]
pub(crate) async fn proxy_image(
    db: &State<DatabaseConnection>,
    pdga_number: i32,
) -> Result<NamedFile, GenericError> {
    // Create a client
    // Send a GET request
    let url = service::get_player_image_path(db, pdga_number)
        .await?
        .ok_or(GenericError::NotFound("Player does not have an image"))?;
    let client = reqwest::Client::new();
    if let Ok(response) = client.get(url).send().await {
        if let Ok(bytes) = response.bytes().await {
            let unique_id = Uuid::new_v4();
            let file_path = format!("/tmp/{}_tmp_chains.jpg", unique_id);
            let mut temp_file = File::create(&file_path).await.expect("create file failed");
            // Write the bytes to a temporary file§
            temp_file.write_all(&bytes).await.expect("write to file failed");
            // Return the image file
            match NamedFile::open(&file_path).await {
                Ok(file) => {
                    tokio::fs::remove_file(&file_path)
                        .await
                        .expect("remove file failed");
                    Ok(file)
                }
                Err(e) => {
                    error!("Error opening file: {}", e);
                    Err(GenericError::UnknownError("Internal server error"))
                }
            }
        } else {
            Err(GenericError::UnknownError("Internal server error"))
        }
    } else {
        Err(GenericError::UnknownError("Internal server error"))
    }
}

#[openapi(tag = "Fantasy Tournament")]
#[get("/fantasy-tournament/<tournament_id>/competition/<competition_id>/scores")]
pub(crate) async fn get_competition_scores(
    db: &State<DatabaseConnection>,
    tournament_id: i32,
    competition_id: i32,
) -> Result<Json<Vec<UserWithCompetitionScores>>, GenericError> {
    dto::user_competition_scores(db.inner(), tournament_id, competition_id)
        .await
        .map(Json)
}
