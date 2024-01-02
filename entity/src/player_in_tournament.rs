//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.10

use sea_orm::entity::prelude::*;
use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Deserialize)]
#[sea_orm(table_name = "player_in_tournament")]
#[serde(rename_all = "PascalCase")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub pdga_number: i32,
    pub tournament_id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::player::Entity",
        from = "Column::PdgaNumber",
        to = "super::player::Column::PdgaNumber",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Player,
    #[sea_orm(
        belongs_to = "super::tournament::Entity",
        from = "Column::TournamentId",
        to = "super::tournament::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Tournament,
}

impl Related<super::player::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Player.def()
    }
}

impl Related<super::tournament::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tournament.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelatedEntity)]
pub enum RelatedEntity {
    #[sea_orm(entity = "super::player::Entity")]
    Player,
    #[sea_orm(entity = "super::tournament::Entity")]
    Tournament,
}
