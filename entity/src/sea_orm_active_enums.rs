//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.10

use sea_orm::entity::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "division")]
pub enum Division {
    #[sea_orm(string_value = "Fpo")]
    Fpo,
    #[sea_orm(string_value = "Mpo")]
    Mpo,
}
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "tournament_status")]
pub enum TournamentStatus {
    #[sea_orm(string_value = "Finished")]
    Finished,
    #[sea_orm(string_value = "NotStarted")]
    NotStarted,
    #[sea_orm(string_value = "Running")]
    Running,
}
