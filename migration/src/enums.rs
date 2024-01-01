use sea_orm::{EnumIter, Iterable};
use sea_orm_migration::prelude::*;
use serde::Deserialize;

#[derive(DeriveIden)]
pub(crate) enum User {
    Table,
    Id,
    Name,
}

#[derive(DeriveIden)]
pub(crate) enum Player {
    Table,
    PDGANumber,
    FirstName,
    LastName,
    Rating,
    Avatar,
    Division,
}

#[derive(DeriveIden)]
pub(crate) enum PlayerDivision {
    Table,
    PlayerPDGANumber,
    Division,
}


#[derive(Iden, EnumIter, Deserialize)]
pub(crate) enum Division {
    Table,
    #[iden = "Mpo"]
    Mpo,
    #[iden = "Fpo"]
    Fpo
}


#[derive(DeriveIden)]
pub(crate) enum Tournament {
    Table,
    Id,
    Status,
}

#[derive(Iden, EnumIter)]
pub(crate) enum TournamentStatus {
    Table,
    #[iden = "NotStarted"]
    NotStarted,
    #[iden = "Running"]
    Running,
    #[iden = "Finished"]
    Finished,
}

#[derive(DeriveIden)]
pub(crate) enum PlayerInTournament {
    Table,
    Id,
    PDGANumber,
    TournamentId,
}

#[derive(DeriveIden)]
pub(crate) enum FantasyTournament {
    Table,
    Id,
    Owner,
    MaxPicksPerUser,

}
