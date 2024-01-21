use sea_orm::EnumIter;
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
    Avatar,
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
    Fpo,
}

#[derive(DeriveIden)]
pub(crate) enum Competition {
    Table,
    Id,
    Status,
    Rounds,
}

#[derive(Iden, EnumIter)]
pub(crate) enum CompetitionStatus {
    Table,
    #[iden = "NotStarted"]
    NotStarted,
    #[iden = "Running"]
    Running,
    #[iden = "Finished"]
    Finished,
}

#[derive(DeriveIden)]
pub(crate) enum PlayerInCompetition {
    Table,
    Id,
    PDGANumber,
    CompetitionId,
}

#[derive(DeriveIden)]
pub(crate) enum PlayerRoundScore {
    Table,
    Id,
    PDGANumber,
    CompetitionId,
    Round,
    Score,
}

#[derive(DeriveIden)]
pub(crate) enum CompetitionInFantasyTournament {
    Table,
    Id,
    CompetitionId,
    FantasyTournamentId,
}

#[derive(DeriveIden)]
pub(crate) enum FantasyTournament {
    Table,
    Id,
    Name,
    Owner,
    MaxPicksPerUser,
}

#[derive(DeriveIden)]
pub(crate) enum FantasyPick {
    Table,
    Id,
    User,
    Player,
    FantasyTournamentId,
    PickNumber,
    Division,
}
#[derive(DeriveIden)]
pub(crate) enum FantasyScores {
    Table,
    Id,
    User,
    Score,
    Ranking,
    FantasyTournamentId,
}

#[derive(DeriveIden)]
pub(crate) enum UserCookies {
    Table,
    Cookie,
    UserId,
}

#[derive(DeriveIden)]
pub(crate) enum UserAuthentication {
    Table,
    UserId,
    HashedPassword,
}

#[derive(Iden, EnumIter)]
pub(crate) enum FantasyTournamentInvitationStatus {
    Table,
    #[iden = "Pending"]
    Pending,
    #[iden = "Accepted"]
    Accepted,
    #[iden = "Declined"]
    Declined,
}

#[derive(DeriveIden)]
pub(crate) enum UserInFantasyTournament {
    Table,
    Id,
    UserId,
    FantasyTournamentId,
    InvitationStatus,
}

#[derive(DeriveIden)]
pub(crate) enum FantasyTournamentDivision {
    Table,
    Id,
    FantasyTournamentId,
    Division,
}