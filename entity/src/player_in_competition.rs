//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.10

use super::sea_orm_active_enums::Division;
use sea_orm::entity::prelude::*;
use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Deserialize)]
#[sea_orm(table_name = "player_in_competition")]
#[serde(rename_all = "PascalCase")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    pub pdga_number: i32,
    pub competition_id: i32,
    pub division: Division,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::competition::Entity",
        from = "Column::CompetitionId",
        to = "super::competition::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Competition,
    #[sea_orm(
        belongs_to = "super::player::Entity",
        from = "Column::PdgaNumber",
        to = "super::player::Column::PdgaNumber",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Player,
    #[sea_orm(
        belongs_to = "super::player_round_score::Entity",
        from = "(Column::CompetitionId, Column::CompetitionId, Column::PdgaNumber, Column::PdgaNumber)",
        to = "(super::player_round_score::Column::PdgaNumber, super::player_round_score::Column::CompetitionId, super::player_round_score::Column::PdgaNumber, super::player_round_score::Column::CompetitionId)",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    PlayerRoundScore,
}

impl Related<super::competition::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Competition.def()
    }
}

impl Related<super::player::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Player.def()
    }
}

impl Related<super::player_round_score::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PlayerRoundScore.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
