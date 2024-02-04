mod fetch_people;
mod get_competition;
mod player_scoring;

pub use fetch_people::{add_players, get_players_from_api, ApiPlayer};

pub use get_competition::CompetitionInfo;

pub use player_scoring::{PlayerScore, RoundInformation};
