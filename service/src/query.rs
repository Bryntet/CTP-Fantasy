use bcrypt::verify;

use dto::InvitationStatus;
use entity::prelude::*;
use entity::sea_orm_active_enums::{CompetitionStatus, Division};
use entity::*;

use log::{error, warn};

use rocket_okapi::okapi::schemars::JsonSchema;
use sea_orm::entity::prelude::*;
use sea_orm::ActiveValue::Set;
use sea_orm::IntoActiveModel;

use crate::dto;
use crate::dto::{CompetitionInfo, FantasyPicks};
use crate::error::GenericError;

pub enum Auth {
    Password(String),
    Cookie(String),
}
pub async fn authenticate(db: &DatabaseConnection, username: String, auth: Auth) -> Result<bool, DbErr> {
    let user = User::find()
        .filter(user::Column::Name.eq(username))
        .one(db)
        .await?;

    if let Some(user) = user {
        match auth {
            Auth::Password(password) => {
                let user_auth = UserAuthentication::find()
                    .filter(user_authentication::Column::UserId.eq(user.id))
                    .one(db)
                    .await?;
                if let Some(user_auth) = user_auth {
                    Ok(verify(&password, &user_auth.hashed_password).is_ok())
                } else {
                    Ok(false)
                }
            }
            Auth::Cookie(cookie_value) => {
                let user_cookie = UserCookies::find()
                    .filter(user_cookies::Column::UserId.eq(user.id))
                    .filter(user_cookies::Column::Cookie.eq(cookie_value))
                    .one(db)
                    .await?;
                Ok(user_cookie.is_some())
            }
        }
    } else {
        Ok(false)
    }
}

pub async fn get_player_image_path(
    db: &DatabaseConnection,
    pdga_number: i32,
) -> Result<Option<String>, GenericError> {
    let player = Player::find_by_id(pdga_number)
        .one(db)
        .await
        .map_err(|_| GenericError::UnknownError("Unknown DB ERROR"))?;
    if let Some(player) = player {
        Ok(player.avatar)
    } else {
        Err(GenericError::NotFound("Player not found"))
    }
}

pub async fn player_exists(db: &impl ConnectionTrait, player_id: i32) -> bool {
    Player::find_by_id(player_id).one(db).await.is_ok()
}

pub async fn get_player_division_in_competition(
    db: &impl ConnectionTrait,
    player_id: i32,
    competition_id: i32,
) -> Result<Option<dto::Division>, DbErr> {
    PlayerInCompetition::find()
        .filter(
            player_in_competition::Column::CompetitionId
                .eq(competition_id)
                .and(player_in_competition::Column::PdgaNumber.eq(player_id)),
        )
        .one(db)
        .await
        .map(|p| p.map(|p| p.division.into()))
}

#[derive(serde::Serialize, serde::Deserialize, JsonSchema, Debug)]
pub struct SimpleFantasyTournament {
    id: i32,
    name: String,
    pub(crate) owner_id: i32,
    invitation_status: InvitationStatus,
}

impl From<sea_orm_active_enums::FantasyTournamentInvitationStatus> for InvitationStatus {
    fn from(status: sea_orm_active_enums::FantasyTournamentInvitationStatus) -> Self {
        match status {
            sea_orm_active_enums::FantasyTournamentInvitationStatus::Accepted => InvitationStatus::Accepted,
            sea_orm_active_enums::FantasyTournamentInvitationStatus::Pending => InvitationStatus::Pending,
            sea_orm_active_enums::FantasyTournamentInvitationStatus::Declined => InvitationStatus::Declined,
        }
    }
}

pub async fn get_users_fantasy_tournaments(
    db: &DatabaseConnection,
    user: &user::Model,
) -> Result<Vec<SimpleFantasyTournament>, GenericError> {
    if user.admin {
        let a = fantasy_tournament::Entity::find()
            .all(db)
            .await
            .map_err(|e| {
                error!("Error while getting tournaments: {:#?}", e);
                GenericError::UnknownError("Unknown error while getting tournaments")
            })?
            .iter()
            .map(|t| SimpleFantasyTournament {
                id: t.id,
                name: t.name.to_string(),
                invitation_status: InvitationStatus::Accepted,
                owner_id: t.owner,
            })
            .collect();
        Ok(a)
    } else {
        let tournaments = user
            .find_related(user_in_fantasy_tournament::Entity)
            .all(db)
            .await
            .map_err(|e| {
                error!("Error while getting tournaments: {:#?}", e);
                GenericError::UnknownError("Unknown error while getting tournaments")
            })?;

        let mut out_things = Vec::new();
        for user_in_tournament in tournaments {
            if let Some(tournament) = user_in_tournament
                .find_related(fantasy_tournament::Entity)
                .one(db)
                .await
                .map_err(|e| {
                    error!("Error while getting tournament: {:#?}", e);
                    GenericError::UnknownError("Unknown error while getting tournament")
                })?
            {
                out_things.push(SimpleFantasyTournament {
                    id: tournament.id,
                    name: tournament.name.to_string(),
                    invitation_status: user_in_tournament.invitation_status.into(),
                    owner_id: tournament.owner,
                });
            }
        }
        Ok(out_things)
    }
}

pub async fn get_fantasy_tournament(
    db: &impl ConnectionTrait,
    tournament_id: i32,
    user_admin_id: Option<i32>,
) -> Result<Option<SimpleFantasyTournament>, GenericError> {
    let t = FantasyTournament::find_by_id(tournament_id)
        .one(db)
        .await
        .map_err(|_| GenericError::UnknownError("Unknown DB ERROR"))?;

    if let Some(t) = t {
        Ok(Some(SimpleFantasyTournament {
            id: t.id,
            name: t.name.to_string(),
            invitation_status: InvitationStatus::Accepted,
            owner_id: user_admin_id.unwrap_or(t.owner),
        }))
    } else {
        Ok(None)
    }
}

pub(crate) async fn get_fantasy_tournament_model(
    db: &impl ConnectionTrait,
    tournament_id: i32,
) -> Result<Option<fantasy_tournament::Model>, GenericError> {
    FantasyTournament::find_by_id(tournament_id)
        .one(db)
        .await
        .map_err(|_| GenericError::UnknownError("Unknown DB ERROR"))
}

pub async fn get_user_participants_in_tournament(
    db: &impl ConnectionTrait,
    tournament_id: i32,
) -> Result<Vec<dto::UserWithScore>, GenericError> {
    let participants = UserInFantasyTournament::find()
        .filter(user_in_fantasy_tournament::Column::FantasyTournamentId.eq(tournament_id))
        .filter(
            user_in_fantasy_tournament::Column::InvitationStatus
                .eq(sea_orm_active_enums::FantasyTournamentInvitationStatus::Accepted),
        )
        .all(db)
        .await
        .map_err(|_| GenericError::UnknownError("Unable to recieve users from database"))?;
    let mut out_things = Vec::new();
    for participant in participants {
        if let Some(participant) = participant
            .find_related(User)
            .one(db)
            .await
            .map_err(|_| GenericError::UnknownError("Unable to recieve user from database"))?
        {
            let score = participant
                .find_related(UserCompetitionScoreInFantasyTournament)
                .filter(
                    user_competition_score_in_fantasy_tournament::Column::FantasyTournamentId
                        .eq(tournament_id),
                )
                .all(db)
                .await
                .map_err(|_| GenericError::UnknownError("Unable to recieve user scores from database"))?
                .iter()
                .map(|score| score.score)
                .sum::<i32>();
            let user = dto::UserWithScore {
                user: dto::User {
                    id: participant.id,
                    username: participant.name,
                },
                score,
            };
            out_things.push(user);
        }
    }

    Ok(out_things)
}

pub async fn get_user_by_name(
    db: &DatabaseConnection,
    username: String,
) -> Result<Option<user::Model>, DbErr> {
    User::find().filter(user::Column::Name.eq(username)).one(db).await
}

pub async fn get_user_pick_in_tournament(
    db: &DatabaseConnection,
    user_id: i32,
    tournament_id: i32,
    slot: i32,
    division: Division,
) -> Result<dto::FantasyPick, GenericError> {
    let pick = FantasyPick::find()
        .filter(
            fantasy_pick::Column::User
                .eq(user_id)
                .and(fantasy_pick::Column::FantasyTournamentId.eq(tournament_id))
                .and(fantasy_pick::Column::PickNumber.eq(slot))
                .and(fantasy_pick::Column::Division.eq(division)),
        )
        .one(db)
        .await
        .expect("good query");

    if let Some(pick) = pick {
        Ok(dto::FantasyPick {
            slot: pick.pick_number,
            pdga_number: pick.player,
            name: get_player_name(db, pick.player).await.ok(),
            benched: pick.benched,
        })
    } else {
        Err(GenericError::NotFound("Pick not found"))
    }
}

pub async fn get_tournament_bench_limit(
    db: &impl ConnectionTrait,
    tournament_id: i32,
) -> Result<i32, GenericError> {
    let tournament = FantasyTournament::find_by_id(tournament_id)
        .one(db)
        .await
        .map_err(|_| GenericError::UnknownError("database error while getting tournament"))?;
    if let Some(tournament) = tournament {
        Ok(tournament.max_picks_per_user - tournament.bench_size)
    } else {
        Err(GenericError::NotFound("Tournament not found"))
    }
}

pub async fn get_tournament_bench_size(
    db: &impl ConnectionTrait,
    tournament_id: i32,
) -> Result<i32, GenericError> {
    let tournament = FantasyTournament::find_by_id(tournament_id)
        .one(db)
        .await
        .map_err(|_| GenericError::UnknownError("database error while getting tournament"))?;
    if let Some(tournament) = tournament {
        Ok(tournament.bench_size)
    } else {
        Err(GenericError::NotFound("Tournament not found"))
    }
}

async fn get_player_name(db: &DatabaseConnection, player_id: i32) -> Result<String, GenericError> {
    let player = Player::find_by_id(player_id)
        .one(db)
        .await
        .map_err(|_| GenericError::UnknownError("database error while getting player"))?;
    if let Some(player) = player {
        Ok(player.first_name + " " + &player.last_name)
    } else {
        Err(GenericError::NotFound("Player not found"))
    }
}

pub async fn get_user_picks_in_tournament(
    db: &impl ConnectionTrait,
    requester_id: i32,
    user_id: i32,
    tournament_id: i32,
    div: &dto::Division,
) -> Result<FantasyPicks, GenericError> {
    let div: Division = div.into();
    let picks = FantasyPick::find()
        .filter(
            fantasy_pick::Column::User
                .eq(user_id)
                .and(fantasy_pick::Column::FantasyTournamentId.eq(tournament_id))
                .and(fantasy_pick::Column::Division.eq(div)),
        )
        .all(db)
        .await
        .map_err(|e| {
            warn!("Error while getting picks: {:#?}", e);
            GenericError::UnknownError("Unknown error while getting picks")
        })?;
    let owner = requester_id == user_id;

    Ok(FantasyPicks {
        picks: {
            let mut out = Vec::new();
            for p in &picks {
                let a: (Option<String>, Option<String>) =
                    if let Ok(Some(related_player)) = p.find_related(Player).one(db).await {
                        (
                            Some(related_player.first_name + " " + &related_player.last_name),
                            related_player.avatar,
                        )
                    } else {
                        (None, None)
                    };

                out.push(dto::FantasyPick {
                    slot: p.pick_number,
                    pdga_number: p.player,
                    name: a.0,
                    benched: p.benched,
                })
            }
            out
        },
        owner,
        fantasy_tournament_id: tournament_id,
    })
}

pub async fn max_picks(db: &impl ConnectionTrait, tournament_id: i32) -> Result<i32, GenericError> {
    let tournament = FantasyTournament::find_by_id(tournament_id)
        .one(db)
        .await
        .map_err(|e| {
            error!("Error while getting tournament: {:#?}", e);
            GenericError::UnknownError("Unknown error while getting tournament")
        })?;
    if let Some(tournament) = tournament {
        Ok(tournament.max_picks_per_user)
    } else {
        Ok(0)
    }
}

pub async fn check_if_user_in_tournament(
    db: &DatabaseConnection,
    user_id: i32,
    tournament_id: i32,
) -> Result<bool, GenericError> {
    let user_in_tournament = UserInFantasyTournament::find()
        .filter(user_in_fantasy_tournament::Column::UserId.eq(user_id))
        .filter(user_in_fantasy_tournament::Column::FantasyTournamentId.eq(tournament_id))
        .one(db)
        .await
        .expect("good query");
    Ok(user_in_tournament.is_some())
}

pub async fn get_tournament_divisions(
    db: &DatabaseConnection,
    tournament_id: i32,
) -> Result<Vec<dto::Division>, DbErr> {
    let picks = FantasyTournamentDivision::find()
        .filter(fantasy_tournament_division::Column::FantasyTournamentId.eq(tournament_id))
        .all(db)
        .await?;

    Ok(picks.iter().map(|p| p.clone().division.into()).collect())
}

pub async fn is_competition_added(db: &impl ConnectionTrait, competition_id: u32) -> Result<bool, DbErr> {
    let comp = Competition::find_by_id(competition_id as i32).one(db).await?;
    Ok(comp.is_some())
}

pub async fn active_rounds(db: &impl ConnectionTrait) -> Result<Vec<round::Model>, DbErr> {
    let start = chrono::Utc::now().date_naive() - chrono::Duration::try_days(1).unwrap();
    let end = chrono::Utc::now().date_naive() + chrono::Duration::try_days(1).unwrap();
    //  dbg!(&start, &end);
    Round::find()
        .filter(round::Column::Date.between(start, end))
        .all(db)
        .await
}

pub async fn active_competitions(db: &impl ConnectionTrait) -> Result<Vec<CompetitionInfo>, GenericError> {
    let competition_models = Competition::find()
        .filter(
            competition::Column::Status
                .eq(CompetitionStatus::Running)
                .or(competition::Column::Status
                    .eq(CompetitionStatus::NotStarted)
                    .and(competition::Column::StartDate.lte(chrono::Utc::now().date_naive()))),
        )
        .all(db)
        .await
        .map_err(|_| GenericError::UnknownError("Unknown error while trying to find active competitions"))?;

    let mut competitions = Vec::new();

    for comp_model in competition_models {
        match CompetitionInfo::from_web(comp_model.id as u32).await {
            Ok(comp) => {
                if comp_model.status != comp.status().into() {
                    let mut model = comp_model.into_active_model();
                    let status: CompetitionStatus = comp.status().into();
                    if status == CompetitionStatus::Finished {
                        model.ended_at = Set(Some(chrono::Utc::now().fixed_offset()));
                    }
                    model.status = Set(comp.status().into());

                    if let Err(e) = model.save(db).await {
                        error!("Encountered db err: {:?}", e.sql_err());
                    }
                }
                competitions.push(comp);
            }
            Err(e) => {
                warn!("Unable to fetch competition from PDGA: {:?}", e);
                Err(GenericError::PdgaGaveUp(
                    "Internal error while fetching competition from PDGA",
                ))?
            }
        }
    }
    Ok(competitions)
}

pub async fn get_player_division_in_tournament(
    db: &impl ConnectionTrait,
    player_id: i32,
    tournament_id: i32,
) -> Result<Option<dto::Division>, GenericError> {
    PlayerDivisionInFantasyTournament::find()
        .filter(
            player_division_in_fantasy_tournament::Column::PlayerPdgaNumber
                .eq(player_id)
                .and(player_division_in_fantasy_tournament::Column::FantasyTournamentId.eq(tournament_id)),
        )
        .one(db)
        .await
        .map(|p| p.map(|p| p.division.into()))
        .map_err(|_| {
            GenericError::UnknownError("Unknown error while trying to find player division in tournament")
        })
}

pub async fn get_player_positions_in_round(
    db: &impl ConnectionTrait,
    competition_id: i32,
    round: i32,
    division: Division,
) -> Result<Vec<player_round_score::Model>, DbErr> {
    let mut player_round_scores = PlayerRoundScore::find()
        .filter(
            player_round_score::Column::CompetitionId
                .eq(competition_id)
                .and(player_round_score::Column::Round.eq(round))
                .and(player_round_score::Column::Division.eq(division)),
        )
        .all(db)
        .await?;

    player_round_scores.sort_by(|a, b| a.throws.cmp(&b.throws));
    player_round_scores.reverse();

    Ok(player_round_scores)
}

pub async fn get_rounds_in_competition(
    db: &impl ConnectionTrait,
    competition_id: i32,
) -> Result<Vec<round::Model>, GenericError> {
    Round::find()
        .filter(round::Column::CompetitionId.eq(competition_id))
        .all(db)
        .await
        .map_err(|_| GenericError::UnknownError("Unknown error while trying to find round"))
}

pub async fn get_competitions_in_fantasy_tournament(
    db: &impl ConnectionTrait,
    fantasy_tournament_id: i32,
) -> Result<Vec<competition::Model>, GenericError> {
    let competitions = CompetitionInFantasyTournament::find()
        .filter(competition_in_fantasy_tournament::Column::FantasyTournamentId.eq(fantasy_tournament_id))
        .all(db)
        .await
        .expect("good query");
    let mut out_things = Vec::new();
    for competition in competitions {
        competition
            .find_related(Competition)
            .one(db)
            .await
            .map(|comp| comp.map(|if_comp| out_things.push(if_comp)))
            .expect("good query");
    }
    Ok(out_things)
}

pub async fn get_active_competitions(
    db: &impl ConnectionTrait,
) -> Result<Vec<competition::Model>, GenericError> {
    let competitions = Competition::find()
        .filter(competition::Column::Status.eq(CompetitionStatus::Running))
        .all(db)
        .await
        .expect("good query");
    let mut out_things = Vec::new();
    for competition in competitions {
        out_things.push(competition);
    }
    Ok(out_things)
}

pub async fn get_pending_competitions(
    db: &impl ConnectionTrait,
) -> Result<Vec<competition::Model>, GenericError> {
    let competitions = Competition::find()
        .filter(competition::Column::Status.eq(CompetitionStatus::NotStarted))
        .all(db)
        .await
        .expect("good query");
    let mut out_things = Vec::new();
    for competition in competitions {
        out_things.push(competition);
    }
    Ok(out_things)
}

pub async fn get_users_in_tournament(
    db: &impl ConnectionTrait,
    tournament_id: i32,
) -> Result<Vec<user::Model>, GenericError> {
    let users_in_tournament = UserInFantasyTournament::find()
        .filter(user_in_fantasy_tournament::Column::FantasyTournamentId.eq(tournament_id))
        .all(db)
        .await
        .map_err(|_| GenericError::UnknownError("Unknown error while trying to find users in tournament"))?;

    let mut out_things = Vec::new();
    for user_in_tournament in users_in_tournament {
        if let Some(user) = user_in_tournament
            .find_related(User)
            .one(db)
            .await
            .map_err(|_| GenericError::UnknownError("Unknown error while trying to find user"))?
        {
            out_things.push(user);
        }
    }
    Ok(out_things)
}
