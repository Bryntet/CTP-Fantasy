//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

use super::sea_orm_active_enums::CompetitionLevel;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "phantom_competition")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    pub name: String,
    pub date: Date,
    pub level: CompetitionLevel,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::phantom_competition_in_fantasy_tournament::Entity")]
    PhantomCompetitionInFantasyTournament,
}

impl Related<super::phantom_competition_in_fantasy_tournament::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PhantomCompetitionInFantasyTournament.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
