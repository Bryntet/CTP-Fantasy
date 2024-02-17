use rocket::http::CookieJar;
use rocket::serde::json::Json;
use rocket::State;
use rocket_okapi::openapi;
use sea_orm::RuntimeErr::SqlxError;
use sea_orm::{DatabaseConnection, DbErr};
use sea_orm::{IntoActiveModel, TransactionTrait, TryIntoModel};

use error::GenericError;
use service::dto::{forms, traits::InsertCompetition, FantasyPick, FantasyPicks, UserLogin};

use crate::authenticate;
use crate::authenticate::AllowedToExchangeGuard;
use crate::error;
use crate::error::{TournamentError, UserError};

/// # Create a fantasy tournament
///
/// # Parameters
///
/// - `name` - The name of the tournament
///
/// - `auth` Cookie - The cookie of the user creating the tournament
///
/// # Returns
///
/// A string indicating success
///
/// # Errors
///
/// - `UserError::InvalidUserId` - The user ID in the cookie is invalid
///
/// - `UserError::UsernameConflict` - The username is already taken
///
/// - `Error::Other` - An unknown error occurred
///
/// - `Error::PlayerError` - The player does not exist
///
/// - `Error::DbErr` - A database error occurred
///
/// - `Error::CookieAuthError` - The cookie is invalid
#[openapi(tag = "Fantasy Tournament")]
#[post("/fantasy-tournament", format = "json", data = "<tournament>")]
pub(crate) async fn create_tournament(
    tournament: Json<service::dto::CreateTournament>,
    db: &State<DatabaseConnection>,
    user: authenticate::UserAuthentication,
) -> Result<(), GenericError> {
    let user_model = user.to_user_model(db.inner()).await?;
    let res = tournament
        .into_inner()
        .insert(db.inner(), user_model.id)
        .await;
    match res {
        Ok(_) => Ok(()),
        Err(DbErr::Query(SqlxError(sqlx::Error::Database(error)))) => {
            let msg = error.message();
            if msg.contains("violates foreign key constraint \"fantasy_tournament_owner_fkey\"") {
                Err(UserError::InvalidUserId("Your user id seems to be invalid?").into())
            } else if msg.contains("violates unique constraint") {
                Err(TournamentError::TournamentNameConflict("Username already taken").into())
            } else {
                Err(GenericError::UnknownError("Unknown error"))
            }
        }
        Err(_) => Err(GenericError::UnknownError("Unknown error")),
    }
}
/// # Create a user
///
/// # Parameters
///
/// - `username` - The username of the user to create
///
/// - `password` - The password of the user to create
#[openapi(tag = "User")]
#[post("/create-user", format = "json", data = "<user>")]
pub async fn create_user(
    user: Json<UserLogin>,
    db: &State<DatabaseConnection>,
    cookies: &CookieJar<'_>,
) -> Result<&'static str, GenericError> {
    user.0.insert(db, cookies).await?;
    Ok("Successfully created user")
}

/// # Add a pick to a fantasy tournament
///
/// # Parameters
///
/// - `slot` - The slot to add the pick to
///
/// - `pdga_number` - The PDGA number of the player to add
///
/// - `fantasy_tournament_id` - The ID of the fantasy tournament to add the pick to
///
/// # Returns
///
/// A string indicating success
#[openapi(tag = "Fantasy Tournament")]
#[allow(clippy::too_many_arguments)]
#[put("/fantasy-tournament/<fantasy_tournament_id>/user/<user_id>/picks/div/<division>/<slot>/<pdga_number>")]
pub(crate) async fn add_pick(
    user: authenticate::UserAuthentication,
    exchange: AllowedToExchangeGuard,
    db: &State<DatabaseConnection>,
    user_id: i32,
    fantasy_tournament_id: i32,
    slot: i32,
    pdga_number: i32,
    division: service::dto::Division,
) -> Result<&'static str, GenericError> {
    let not_permitted = UserError::NotPermitted("You are not permitted to add picks for this user");
    if !exchange.is_allowed() {
        return Err(GenericError::NotPermitted(
            "You are not allowed to exchange picks at this time",
        ));
    }
    let db = db.inner();
    let user = user.to_user_model(db).await?;
    if service::check_if_user_in_tournament(db, user.id, fantasy_tournament_id)
        .await
        .unwrap_or(false)
    {
        if user.id != user_id {
            return Err(not_permitted.into());
        }
        let pick = FantasyPick {
            slot,
            pdga_number,
            name: None,
            avatar: None,
            benched: false,
        };

        pick.change_or_insert(db, user.id, fantasy_tournament_id, division)
            .await?;

        Ok("Successfully added pick")
    } else {
        Err(not_permitted.into())
    }
}

#[openapi(tag = "Fantasy Tournament")]
#[post(
    "/fantasy-tournament/<fantasy_tournament_id>/user/<user_id>/picks/div/<division>",
    format = "json",
    data = "<json_picks>"
)]
pub(crate) async fn add_picks(
    user: authenticate::UserAuthentication,
    exchange: AllowedToExchangeGuard,
    db: &State<DatabaseConnection>,
    user_id: i32,
    fantasy_tournament_id: i32,
    json_picks: Json<Vec<FantasyPick>>,
    division: service::dto::Division,
) -> Result<String, GenericError> {
    let db = db.inner();
    let user = user.to_user_model(db).await?;
    if user.id != user_id {
        return Err(
            UserError::NotPermitted("You are not permitted to add picks for this user").into(),
        );
    }

    let current_picks: FantasyPicks = service::query::get_user_picks_in_tournament(
        db,
        &user,
        user.id,
        fantasy_tournament_id,
        &division,
    )
    .await?;

    let picks = json_picks.into_inner();
    let all_picks_match = picks.iter().all(|p| {
        current_picks
            .picks
            .iter()
            .any(|other| other.pdga_number == p.pdga_number)
    });

    let move_allowed = exchange.is_move_allowed(db, fantasy_tournament_id).await && all_picks_match;

    if exchange.is_allowed() || move_allowed {
        let txn = db
            .begin()
            .await
            .map_err(|_| GenericError::UnknownError("transaction start failed"))?;
        for pick in picks {
            pick.change_or_insert(&txn, user.id, fantasy_tournament_id, division.clone())
                .await?;
        }
        txn.commit()
            .await
            .map_err(|_| GenericError::UnknownError("transaction failed"))?;
        Ok("Successfully added or reordered picks".to_string())
    } else {
        Err(GenericError::NotPermitted(
            "You are not allowed to exchange picks at this time",
        ))
    }
}

#[openapi(tag = "Fantasy Tournament")]
#[post("/fantasy-tournament/<fantasy_tournament_id>/invite/<invited_user>")]
pub(crate) async fn invite_user(
    auth: authenticate::TournamentOwner,
    db: &State<DatabaseConnection>,
    fantasy_tournament_id: i32,
    invited_user: String,
) -> Result<String, GenericError> {
    let user = auth.user.to_user_model(db).await?;
    match service::create_invite(db, user, invited_user, fantasy_tournament_id).await {
        Ok(_) => Ok("Successfully invited user".to_string()),
        Err(e) => Err(e.into()),
    }
}

#[openapi(tag = "Fantasy Tournament")]
#[post("/fantasy-tournament/<fantasy_tournament_id>/answer-invite/<accepted>")]
pub(crate) async fn answer_invite(
    user: authenticate::UserAuthentication,
    db: &State<DatabaseConnection>,
    fantasy_tournament_id: i32,
    accepted: bool,
) -> Result<String, GenericError> {
    let user = user.to_user_model(db).await?;
    match service::answer_invite(db, user, fantasy_tournament_id, accepted).await {
        Ok(()) => Ok("Successfully answered invite".to_string()),
        Err(e) => Err(e.into()),
    }
}
#[openapi(tag = "Fantasy Tournament")]
#[post(
    "/fantasy-tournament/<fantasy_tournament_id>/competition/add",
    data = "<competition>"
)]
pub(crate) async fn add_competition(
    _auth: authenticate::TournamentOwner,
    db: &State<DatabaseConnection>,
    fantasy_tournament_id: u32,
    competition: Json<forms::AddCompetition>,
) -> Result<String, GenericError> {
    let db = db.inner();
    let txn = db.begin().await.map_err(|_| {
        GenericError::UnknownError(
            "internal error, please try again or contact support if problem persists",
        )
    })?;
    let competition_input = competition.into_inner();
    let competition =
        service::dto::CompetitionInfo::from_web(competition_input.competition_id).await?;
    if !competition.is_in_db(&txn).await? {
        competition
            .insert_in_db(&txn, competition_input.level.into())
            .await?
    }
    competition
        .insert_in_fantasy(&txn, fantasy_tournament_id)
        .await?;
    competition
        .insert_players(&txn, Some(fantasy_tournament_id as i32))
        .await?;

    txn.commit().await.map_err(|_| {
        GenericError::UnknownError("Unknown error while trying to commit transaction")
    })?;
    let rounds =
        service::get_rounds_in_competition(db, competition_input.competition_id as i32).await?;
    service::mutation::update_rounds(db, rounds).await;
    competition
        .save_user_scores(db, fantasy_tournament_id)
        .await?;
    Ok("Successfully added competition".to_string())
}

//#[openapi(tag="Fantasy Tournament")]
//#[post("/fantasy-tournament/<fantasy_tournament_id>/add-competition/<competition_id>/placeholder/")]
