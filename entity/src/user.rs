//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.10

use sea_orm::entity::prelude::*;
use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Deserialize)]
#[sea_orm(table_name = "user")]
#[serde(rename_all = "PascalCase")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::fantasy_pick::Entity")]
    FantasyPick,
    #[sea_orm(has_many = "super::fantasy_scores::Entity")]
    FantasyScores,
    #[sea_orm(has_many = "super::fantasy_tournament::Entity")]
    FantasyTournament,
    #[sea_orm(has_many = "super::user_authentication::Entity")]
    UserAuthentication,
    #[sea_orm(has_many = "super::user_cookies::Entity")]
    UserCookies,
    #[sea_orm(has_many = "super::user_in_fantasy_tournament::Entity")]
    UserInFantasyTournament,
}

impl Related<super::fantasy_pick::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FantasyPick.def()
    }
}

impl Related<super::fantasy_scores::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FantasyScores.def()
    }
}

impl Related<super::fantasy_tournament::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FantasyTournament.def()
    }
}

impl Related<super::user_authentication::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserAuthentication.def()
    }
}

impl Related<super::user_cookies::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserCookies.def()
    }
}

impl Related<super::user_in_fantasy_tournament::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserInFantasyTournament.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
