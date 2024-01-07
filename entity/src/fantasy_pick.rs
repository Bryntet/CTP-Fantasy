//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.10

use super::sea_orm_active_enums::Division;
use sea_orm::entity::prelude::*;
use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Deserialize)]
#[sea_orm(table_name = "fantasy_pick")]
#[serde(rename_all = "PascalCase")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user: i32,
    pub player: i32,
    pub fantasy_tournament_id: i32,
    pub pick_number: i32,
    pub division: Division,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::fantasy_tournament::Entity",
        from = "Column::FantasyTournamentId",
        to = "super::fantasy_tournament::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    FantasyTournament,
    #[sea_orm(
        belongs_to = "super::player::Entity",
        from = "Column::Player",
        to = "super::player::Column::PdgaNumber",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Player,
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::User",
        to = "super::user::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    User,
}

impl Related<super::fantasy_tournament::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FantasyTournament.def()
    }
}

impl Related<super::player::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Player.def()
    }
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
