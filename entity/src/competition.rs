//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.10

use super::sea_orm_active_enums::CompetitionStatus;
use sea_orm::entity::prelude::*;
use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Deserialize)]
#[sea_orm(table_name = "competition")]
#[serde(rename_all = "PascalCase")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    pub name: String,
    pub status: CompetitionStatus,
    pub rounds: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::competition_in_fantasy_tournament::Entity")]
    CompetitionInFantasyTournament,
    #[sea_orm(has_many = "super::fantasy_scores::Entity")]
    FantasyScores,
    #[sea_orm(has_many = "super::player_in_competition::Entity")]
    PlayerInCompetition,
    #[sea_orm(has_many = "super::player_round_score::Entity")]
    PlayerRoundScore,
    #[sea_orm(has_many = "super::round::Entity")]
    Round,
}

impl Related<super::competition_in_fantasy_tournament::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CompetitionInFantasyTournament.def()
    }
}

impl Related<super::fantasy_scores::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FantasyScores.def()
    }
}

impl Related<super::player_in_competition::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PlayerInCompetition.def()
    }
}

impl Related<super::player_round_score::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PlayerRoundScore.def()
    }
}

impl Related<super::round::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Round.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
