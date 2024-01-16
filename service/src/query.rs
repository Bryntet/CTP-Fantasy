use bcrypt::verify;
use entity::prelude::*;
use entity::sea_orm_active_enums::Division;
use entity::*;
use rocket_okapi::okapi::schemars;
use rocket_okapi::okapi::schemars::JsonSchema;
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, DbErr, EntityTrait};
use serde::Deserialize;
use crate::dto;
use dto::InvitationStatus;


#[derive(Deserialize, JsonSchema, Debug)]
pub struct LoginInput {
    pub username: String,
    pub password: String,
}

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

pub async fn player_exists(db: &DatabaseConnection, player_id: i32) -> bool {
    Player::find_by_id(player_id).one(db).await.is_ok()
}

pub async fn get_player_division(
    db: &DatabaseConnection,
    player_id: i32,
) -> Result<Vec<Division>, DbErr> {
    let player_division = PlayerDivision::find_by_id(player_id).all(db).await?;

    let divs = player_division
        .iter()
        .map(|pd| pd.clone().division)
        .collect();
    Ok(divs)
}




#[derive(serde::Serialize, serde::Deserialize, JsonSchema, Debug)]
pub struct SimpleFantasyTournament {
    id: i32,
    name: String,
    pub(crate) owner_id: i32,
    invitation_status: InvitationStatus,
}

impl From<sea_orm_active_enums::FantasyTournamentInvitationStatus>  for InvitationStatus {
    fn from(status: sea_orm_active_enums::FantasyTournamentInvitationStatus) -> Self {
        match status {
            sea_orm_active_enums::FantasyTournamentInvitationStatus::Accepted => InvitationStatus::Accepted,
            sea_orm_active_enums::FantasyTournamentInvitationStatus::Pending => InvitationStatus::Pending,
            sea_orm_active_enums::FantasyTournamentInvitationStatus::Declined => InvitationStatus::Declined,
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
        if let Some(tournament) = FantasyTournament::find_by_id(user_in_tournament.fantasy_tournament_id)
            .one(db)
            .await? {
            out_things.push(
                SimpleFantasyTournament {
                    id: tournament.id,
                    name: tournament.name.to_string(),
                    invitation_status: user_in_tournament.invitation_status.into(),
                    owner_id: tournament.owner,
                }
            );
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
        if let Some(participant) = participant.find_related(User)
            .one(db)
            .await? {
            out_things.push(
                participant.find_related(FantasyScores)
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
                    })
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
#[derive(serde::Serialize, serde::Deserialize, JsonSchema, Debug)]
struct SimpleFantasyPick {
    slot: i32,
    pdga_number: i32,
    name: String
}

#[derive(serde::Serialize, serde::Deserialize, JsonSchema, Debug)]
pub struct SimpleFantasyPicks {
    picks: Vec<SimpleFantasyPick>,
    owner: bool,
    fantasy_tournament_id: i32,
}

pub async fn get_user_picks_in_tournament(
    db: &DatabaseConnection,
    requester: user::Model,
    user_id: i32,
    tournament_id: i32,
) -> Result<SimpleFantasyPicks, DbErr> {
    let picks = FantasyPick::find()
        .filter(fantasy_pick::Column::User.eq(user_id).and(fantasy_pick::Column::FantasyTournamentId.eq(tournament_id)))
        .all(db)
        .await?;
    let owner = requester.id == user_id;


    Ok(SimpleFantasyPicks {
        picks: {
            let mut out = Vec::new();
            for p in &picks {
                out.push(SimpleFantasyPick {
                    slot: p.pick_number,
                    pdga_number: p.player,
                    name: if let Ok(Some(p))= p.find_related(Player).one(db).await {
                        p.first_name + " " + &p.last_name
                    } else {
                        DbErr::RecordNotFound("Player not found".to_string()).to_string()
                    }
                })
            }
            out
        },
        owner,
        fantasy_tournament_id: tournament_id,
    })
}

