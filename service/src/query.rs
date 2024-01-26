use bcrypt::verify;
use rocket_okapi::okapi::schemars;
use rocket_okapi::okapi::schemars::JsonSchema;
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, DbErr, EntityTrait, QueryOrder};

use dto::InvitationStatus;
use entity::prelude::*;
use entity::sea_orm_active_enums::Division;
use entity::*;

use crate::dto;
use crate::dto::FantasyPicks;
use crate::error::{GenericError, PlayerError};

pub enum Auth {
    Password(String),
    Cookie(String),
}
pub async fn authenticate(
    db: &DatabaseConnection,
    username: String,
    auth: Auth,
) -> Result<bool, DbErr> {
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
            sea_orm_active_enums::FantasyTournamentInvitationStatus::Accepted => {
                InvitationStatus::Accepted
            }
            sea_orm_active_enums::FantasyTournamentInvitationStatus::Pending => {
                InvitationStatus::Pending
            }
            sea_orm_active_enums::FantasyTournamentInvitationStatus::Declined => {
                InvitationStatus::Declined
            }
        }
    }
}
pub async fn get_fantasy_tournaments(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<Vec<SimpleFantasyTournament>, DbErr> {
    let tournaments = UserInFantasyTournament::find()
        .filter(user_in_fantasy_tournament::Column::UserId.eq(user_id))
        .all(db)
        .await?;

    let mut out_things = Vec::new();
    for user_in_tournament in tournaments {
        if let Some(tournament) =
            FantasyTournament::find_by_id(user_in_tournament.fantasy_tournament_id)
                .one(db)
                .await?
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

pub async fn get_fantasy_tournament(
    db: &DatabaseConnection,
    tournament_id: i32,
) -> Result<Option<SimpleFantasyTournament>, DbErr> {
    let t = FantasyTournament::find_by_id(tournament_id).one(db).await?;

    if let Some(t) = t {
        Ok(Some(SimpleFantasyTournament {
            id: t.id,
            name: t.name.to_string(),
            invitation_status: InvitationStatus::Accepted,
            owner_id: t.owner,
        }))
    } else {
        Ok(None)
    }
}

pub async fn get_participants(
    db: &DatabaseConnection,
    tournament_id: i32,
) -> Result<Vec<dto::User>, DbErr> {
    let participants = UserInFantasyTournament::find()
        .filter(user_in_fantasy_tournament::Column::FantasyTournamentId.eq(tournament_id))
        .filter(
            user_in_fantasy_tournament::Column::InvitationStatus
                .eq(sea_orm_active_enums::FantasyTournamentInvitationStatus::Accepted),
        )
        .all(db)
        .await?;

    let mut out_things = Vec::new();
    for participant in participants {
        if let Some(participant) = participant.find_related(User).one(db).await? {
            out_things.push(
                participant
                    .find_related(FantasyScores)
                    .filter(fantasy_scores::Column::FantasyTournamentId.eq(tournament_id))
                    .one(db)
                    .await?
                    .map(|score| dto::User {
                        id: participant.id,
                        name: participant.name.to_string(),
                        score: score.score,
                    })
                    .unwrap_or(dto::User {
                        id: participant.id,
                        name: participant.name.to_string(),
                        score: 0,
                    }),
            );
        }
    }

    Ok(out_things)
}

pub async fn get_user_by_name(
    db: &DatabaseConnection,
    username: String,
) -> Result<Option<user::Model>, DbErr> {
    User::find()
        .filter(user::Column::Name.eq(username))
        .one(db)
        .await
}

pub async fn get_user_pick_in_tournament(
    db: &DatabaseConnection,
    user_id: i32,
    tournament_id: i32,
    slot: i32,
) -> Result<dto::FantasyPick, GenericError> {
    let pick = FantasyPick::find()
        .filter(
            fantasy_pick::Column::User
                .eq(user_id)
                .and(fantasy_pick::Column::FantasyTournamentId.eq(tournament_id))
                .and(fantasy_pick::Column::PickNumber.eq(slot)),
        )
        .one(db)
        .await?;

    if let Some(pick) = pick {
        Ok(dto::FantasyPick {
            slot: pick.pick_number,
            pdga_number: pick.player,
            name: get_player_name(db, pick.player).await.ok(),
        })
    } else {
        Err(GenericError::NotFound("Pick not found"))
    }
}

async fn get_player_name(db: &DatabaseConnection, player_id: i32) -> Result<String, GenericError> {
    let player = Player::find_by_id(player_id).one(db).await?;
    if let Some(player) = player {
        Ok(player.first_name + " " + &player.last_name)
    } else {
        Err(GenericError::NotFound("Player not found"))
    }
}

pub async fn get_user_picks_in_tournament(
    db: &DatabaseConnection,
    requester: user::Model,
    user_id: i32,
    tournament_id: i32,
    div: dto::Division,
) -> Result<FantasyPicks, DbErr> {
    let picks = FantasyPick::find()
        .filter(
            fantasy_pick::Column::User
                .eq(user_id)
                .and(fantasy_pick::Column::FantasyTournamentId.eq(tournament_id))
                .and(fantasy_pick::Column::Division.eq(div.to_string())),
        )
        .all(db)
        .await?;
    let owner = requester.id == user_id;

    Ok(FantasyPicks {
        picks: {
            let mut out = Vec::new();
            for p in &picks {
                out.push(dto::FantasyPick {
                    slot: p.pick_number,
                    pdga_number: p.player,
                    name: if let Ok(Some(p)) = p.find_related(Player).one(db).await {
                        Some(p.first_name + " " + &p.last_name)
                    } else {
                        None
                    },
                })
            }
            out
        },
        owner,
        fantasy_tournament_id: tournament_id,
    })
}

pub async fn max_picks(db: &DatabaseConnection, tournament_id: i32) -> Result<i32, DbErr> {
    let tournament = FantasyTournament::find_by_id(tournament_id).one(db).await?;
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
        .await?;
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

pub async fn is_competition_added(
    db: &impl ConnectionTrait,
    competition_id: u32,
) -> Result<bool, DbErr> {
    let comp = Competition::find_by_id(competition_id as i32)
        .one(db)
        .await?;
    Ok(comp.is_some())
}

pub async fn active_rounds(db: &impl ConnectionTrait) -> Result<Vec<round::Model>, DbErr> {
    let start = chrono::Utc::now().date_naive() - chrono::Duration::days(1);
    let end = chrono::Utc::now().date_naive() + chrono::Duration::days(1);
    //  dbg!(&start, &end);
    Round::find()
        .filter(round::Column::Date.between(start, end))
        .all(db)
        .await
}

pub async fn get_player_division_in_tournament(
    db: &impl ConnectionTrait,
    player_id: i32,
    tournament_id: i32,
) -> Result<Option<dto::Division>, DbErr> {
    let res = PlayerDivisionInFantasyTournament::find()
        .filter(
            player_division_in_fantasy_tournament::Column::PlayerPdgaNumber
                .eq(player_id)
                .and(
                    player_division_in_fantasy_tournament::Column::FantasyTournamentId
                        .eq(tournament_id),
                ),
        )
        .one(db)
        .await
        .map(|p| p.map(|p| p.division.into()));
    dbg!(&res);
    res
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

    player_round_scores.sort_by(|a, b| a.score.cmp(&b.score));
    player_round_scores.reverse();

    Ok(player_round_scores)
}
