use crate::dto::Division;
use crate::error::{GenericError, PlayerError};
use crate::player_exists;
use chrono::Utc;
use entity::{
    fantasy_pick, player, player_division_in_fantasy_tournament, player_trade, sea_orm_active_enums,
};
use itertools::Itertools;
use rayon::prelude::*;
use rocket::warn;
use rocket_okapi::JsonSchema;
use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, IntoActiveModel, ModelTrait, NotSet,
    QueryFilter,
};
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize, Serialize, JsonSchema, Debug)]
pub struct FantasyPick {
    pub slot: i32,
    pub pdga_number: i32,
    pub name: Option<String>,
    #[serde(default)]
    pub benched: bool,
}

impl FantasyPick {
    pub async fn change_or_insert(
        &self,
        db: &impl ConnectionTrait,
        user_id: i32,
        tournament_id: i32,
        div: Division,
    ) -> Result<(), GenericError> {
        if !player_exists(db, self.pdga_number).await {
            return Err(PlayerError::NotFound.into());
        }
        let actual_player_div =
            super::super::get_player_division_in_tournament(db, self.pdga_number, tournament_id).await;

        let actual_player_div: Division = match actual_player_div {
            Err(_) => Err(GenericError::UnknownError("Unable to get player division")),
            Ok(None) => {
                if let Ok(Some(player)) = player::Entity::find_by_id(self.pdga_number).one(db).await {
                    if let Ok(Some(div)) = player
                        .find_related(player_division_in_fantasy_tournament::Entity)
                        .all(db)
                        .await
                        .map(|divs| divs.first().cloned())
                    {
                        Ok(div.division.into())
                    } else {
                        Err(GenericError::UnknownError("Unable to get player division"))
                    }
                } else {
                    Err(GenericError::UnknownError("Unable to get player division"))
                }
            }
            Ok(Some(v)) => Ok(v),
        }?;

        if !actual_player_div.eq(&div) {
            Err(GenericError::Conflict("Player division does not match division"))?
        }

        self.insert(db, user_id, tournament_id, actual_player_div).await?;
        Ok(())
    }

    async fn is_benched(&self, db: &impl ConnectionTrait, tournament_id: i32) -> Result<bool, GenericError> {
        Ok(self.slot > (super::super::get_tournament_bench_limit(db, tournament_id).await?))
    }

    async fn insert(
        &self,
        db: &impl ConnectionTrait,
        user_id: i32,
        tournament_id: i32,
        division: Division,
    ) -> Result<(), GenericError> {
        let mut new_pick = fantasy_pick::ActiveModel {
            id: NotSet,
            user: Set(user_id),
            pick_number: Set(self.slot),
            player: Set(self.pdga_number),
            fantasy_tournament_id: Set(tournament_id),
            division: Set((&division).into()),
            benched: Set(self.is_benched(db, tournament_id).await?),
        };
        let column_filter = fantasy_pick::Column::User
            .eq(user_id)
            .and(fantasy_pick::Column::FantasyTournamentId.eq(tournament_id))
            .and(fantasy_pick::Column::Division.eq::<sea_orm_active_enums::Division>(division.into()));

        let possible_prev_pick = fantasy_pick::Entity::find()
            .filter(
                column_filter
                    .clone()
                    .and(fantasy_pick::Column::Player.eq(new_pick.player.clone().take())),
            )
            .one(db)
            .await
            .map(|p| p.map(|p| p.into_active_model()));
        let other_pick = fantasy_pick::Entity::find()
            .filter(column_filter.and(fantasy_pick::Column::PickNumber.eq(self.slot)))
            .one(db)
            .await
            .map(|p| p.map(|p| p.into_active_model()));
        match (possible_prev_pick, other_pick) {
            // Swap two picks
            (Ok(Some(mut previous_placement_of_player)), Ok(Some(mut other_pick))) => {
                if previous_placement_of_player
                    .pick_number
                    .clone()
                    .take()
                    .is_some_and(|num| num != self.slot)
                {
                    previous_placement_of_player.player = Set(other_pick.player.clone().take().unwrap());
                    other_pick.clone().delete(db).await.map_err(|e| {
                        warn!("Unable to insert pick: {:#?}", e);
                        GenericError::UnknownError("Unable to insert pick")
                    })?;
                    new_pick.id = Set(other_pick.id.take().unwrap());
                    player_trade::ActiveModel {
                        id: NotSet,
                        user: Set(user_id),
                        player: Set(self.pdga_number),
                        slot: Set(self.slot),
                        fantasy_tournament_id: Set(tournament_id),
                        timestamp: Set(Utc::now().fixed_offset()),
                        is_local_swap: Set(true),
                        other_player: Set(previous_placement_of_player.player.clone().take()),
                        other_slot: Set(previous_placement_of_player.pick_number.clone().take()),
                    }
                    .save(db)
                    .await
                    .map_err(|e| {
                        warn!("Unable to insert pick: {:#?}", e);
                        GenericError::UnknownError("Unable to insert pick")
                    })?;
                    previous_placement_of_player.save(db).await.map_err(|e| {
                        warn!("Unable to insert pick: {:#?}", e);
                        GenericError::UnknownError("Unable to insert pick")
                    })?;
                    new_pick.insert(db).await.map_err(|e| {
                        warn!("Unable to insert pick: {:#?}", e);
                        GenericError::UnknownError("Unable to insert pick")
                    })?;
                }
                Ok(())
            }

            // Move pick to new slot when there is no pick in the new slot
            (Ok(Some(mut previous_placement_of_player)), Ok(None)) => {
                if previous_placement_of_player
                    .pick_number
                    .clone()
                    .take()
                    .is_some_and(|num| num != self.slot)
                {
                    player_trade::ActiveModel {
                        id: NotSet,
                        user: Set(user_id),
                        player: Set(self.pdga_number),
                        slot: Set(self.slot),
                        fantasy_tournament_id: Set(tournament_id),
                        timestamp: Set(Utc::now().fixed_offset()),
                        is_local_swap: Set(false),
                        other_player: Set(None),
                        other_slot: Set(previous_placement_of_player.pick_number.clone().take()),
                    }
                    .save(db)
                    .await
                    .map_err(|e| {
                        warn!("Unable to insert pick: {:#?}", e);
                        GenericError::UnknownError("Unable to insert pick")
                    })?;
                    previous_placement_of_player.pick_number = Set(self.slot);
                    previous_placement_of_player.save(db).await.map_err(|e| {
                        warn!("Unable to insert pick: {:#?}", e);
                        GenericError::UnknownError("Unable to insert pick")
                    })?;
                }
                Ok(())
            }

            // Insert new pick when there is no pick in the new slot
            (Ok(None), Ok(None)) => {
                player_trade::ActiveModel {
                    id: NotSet,
                    user: Set(user_id),
                    player: Set(self.pdga_number),
                    slot: Set(self.slot),
                    fantasy_tournament_id: Set(tournament_id),
                    timestamp: Set(Utc::now().fixed_offset()),
                    is_local_swap: Set(false),
                    other_player: Set(None),
                    other_slot: Set(None),
                }
                .save(db)
                .await
                .map_err(|e| {
                    warn!("Unable to insert pick: {:#?}", e);
                    GenericError::UnknownError("Unable to insert pick")
                })?;
                fantasy_pick::Entity::insert(new_pick)
                    .exec(db)
                    .await
                    .map_err(|e| {
                        warn!("Unable to insert pick: {:#?}", e);
                        GenericError::UnknownError("Unable to insert pick")
                    })?;
                Ok(())
            }

            // Insert new pick when there is a pick in the new slot
            (Ok(None), Ok(Some(mut other_pick))) => {
                player_trade::ActiveModel {
                    id: NotSet,
                    user: Set(user_id),
                    player: Set(self.pdga_number),
                    slot: Set(self.slot),
                    fantasy_tournament_id: Set(tournament_id),
                    timestamp: Set(Utc::now().fixed_offset()),
                    is_local_swap: Set(false),
                    other_player: Set(other_pick.player.clone().take()),
                    other_slot: Set(other_pick.pick_number.clone().take()),
                }
                .save(db)
                .await
                .map_err(|e| {
                    warn!("Unable to insert pick: {:#?}", e);
                    GenericError::UnknownError("Unable to insert pick")
                })?;
                other_pick.player = Set(self.pdga_number);
                other_pick.save(db).await.map_err(|e| {
                    warn!("Unable to insert pick: {:#?}", e);
                    GenericError::UnknownError("Unable to insert pick")
                })?;
                Ok(())
            }
            (Err(_), Err(_)) | (Err(_), _) | (_, Err(_)) => {
                Err(GenericError::UnknownError("Unable to insert pick"))
            }
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, JsonSchema, Debug)]
pub struct FantasyPicks {
    pub picks: Vec<FantasyPick>,
    pub(crate) owner: bool,
    pub(crate) fantasy_tournament_id: i32,
}
#[derive(Debug)]
struct PlayerTradeLog {
    user: i32,
    player: i32,
    slot: usize,
    action: PlayerTradingAction,
    timestamp: DateTimeWithTimeZone,
}

impl PlayerTradeLog {
    fn players(&self) -> Vec<i32> {
        let mut out = vec![self.player];
        if let PlayerTradingAction::Swap(swap) = &self.action {
            out.push(swap.get_player())
        }
        out
    }

    fn into_external_formatting(
        self,
        users: &HashMap<i32, String>,
        players: &HashMap<i32, String>,
    ) -> String {
        let user = users
            .get(&self.user)
            .map(|s| s.to_owned())
            .unwrap_or(self.user.to_string());
        let player = players
            .get(&self.player)
            .map(|s| s.to_owned())
            .unwrap_or(self.player.to_string());

        let action = match self.action {
            PlayerTradingAction::Add => format!("Added {} to slot {}", player, self.slot),
            PlayerTradingAction::Swap(swap) => match swap {
                PlayerTradingSwapType::Local {
                    other_slot,
                    other_player,
                } => {
                    let other_player = players
                        .get(&other_player)
                        .map(|s| s.to_owned())
                        .unwrap_or(swap.get_player().to_string());
                    format!(
                        "Swapped {} with {} (slots {} and {})",
                        player, other_player, self.slot, other_slot
                    )
                }
                PlayerTradingSwapType::Tournament { other_player } => {
                    let other_player = players
                        .get(&other_player)
                        .map(|s| s.to_owned())
                        .unwrap_or(swap.get_player().to_string());
                    format!("Swapped {} in slot {} with {}", player, self.slot, other_player,)
                }
            },
        };

        // TODO: Send timestamp data with TZ to frontend to display based on local timezone
        format!(
            "{}: {} - At {}",
            user,
            action,
            self.timestamp.format("%Y-%m-%d %H:%M:%S")
        )
    }
}
#[derive(Debug)]
enum PlayerTradingAction {
    Add,
    Swap(PlayerTradingSwapType),
}
#[derive(Debug)]
enum PlayerTradingSwapType {
    Local { other_slot: usize, other_player: i32 },
    Tournament { other_player: i32 },
}

impl PlayerTradingSwapType {
    fn get_player(&self) -> i32 {
        match self {
            Self::Local { other_player, .. } => *other_player,
            Self::Tournament { other_player } => *other_player,
        }
    }
}

impl From<player_trade::Model> for PlayerTradeLog {
    fn from(trade: player_trade::Model) -> Self {
        let user = trade.user;
        let player = trade.player;
        let slot = trade.slot as usize;
        let action = if trade.is_local_swap {
            PlayerTradingAction::Swap(PlayerTradingSwapType::Local {
                other_slot: trade
                    .other_slot
                    .expect("Other slot needs to be used for local swap")
                    as usize,
                other_player: trade
                    .other_player
                    .expect("Other player needs to be used for local swap"),
            })
        } else if let Some(other_player) = trade.other_player {
            PlayerTradingAction::Swap(PlayerTradingSwapType::Tournament { other_player })
        } else {
            PlayerTradingAction::Add
        };

        Self {
            user,
            player,
            slot,
            action,
            timestamp: trade.timestamp,
        }
    }
}

pub struct PlayerTradesLog(Vec<PlayerTradeLog>);

impl PlayerTradesLog {
    pub async fn get(db: &impl ConnectionTrait, tournament_id: i32) -> Self {
        let trades = player_trade::Entity::find()
            .filter(player_trade::Column::FantasyTournamentId.eq(tournament_id))
            .all(db)
            .await
            .unwrap_or_default();
        Self(
            trades
                .into_iter()
                .sorted_by(|a, b| b.timestamp.cmp(&a.timestamp))
                .map(PlayerTradeLog::from)
                .collect_vec(),
        )
    }

    pub async fn into_formatting(self, db: &impl ConnectionTrait) -> Vec<String> {
        let trades = self.0;

        let (users, players): (Vec<_>, Vec<_>) =
            trades.iter().map(|trade| (trade.user, trade.players())).unzip();
        let players = players.into_iter().flatten().dedup().collect_vec();
        let users = users.into_iter().dedup().collect_vec();

        let mut player_map = HashMap::new();

        for player_id in players {
            if let Ok(Some(player)) = entity::player::Entity::find_by_id(player_id).one(db).await {
                player_map.insert(player_id, format!("{} {}", player.first_name, player.last_name));
            }
        }

        let mut user_map = HashMap::new();

        for user_id in users {
            if let Ok(Some(user)) = entity::user::Entity::find_by_id(user_id).one(db).await {
                user_map.insert(user_id, user.name);
            }
        }

        trades
            .into_par_iter()
            .map(|player| player.into_external_formatting(&user_map, &player_map))
            .collect()
    }
}
