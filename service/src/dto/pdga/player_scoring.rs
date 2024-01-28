use crate::dto::pdga::add_players;
use crate::dto::Division;
use entity::player_round_score;
use entity::player_round_score::ActiveModel;
use entity::prelude::{
    Competition, CompetitionInFantasyTournament, FantasyTournament, PlayerRoundScore, Round,
};
use itertools::Itertools;
use sea_orm::ActiveValue::Set;
use sea_orm::{sea_query, TransactionTrait};
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, IntoActiveModel,
    ModelTrait, NotSet,
};
use sea_orm::{ColumnTrait, QueryFilter};
use serde_derive::Deserialize;

#[derive(Deserialize)]
enum Unit {
    Meters,
    Feet,
}
#[derive(Deserialize)]
struct ApiRes {
    data: RoundFromApi,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Layout {
    #[serde(rename = "Detail")]
    holes: Vec<Hole>,
    length: u32,
    #[serde(rename = "Units")]
    unit: Unit,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct Hole {
    pub par: u32,
    #[serde(rename = "HoleOrdinal")]
    pub hole_number: u32,
    pub length: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ApiPlayerInRound {
    #[serde(rename = "PDGANum")]
    pub pdga_number: i32,
    pub name: String,
    pub hole_scores: Vec<String>,
    pub round_score: u32,
    #[serde(rename = "RoundtoPar")]
    pub round_to_par: i32,
}
impl From<ApiPlayerInRound> for PlayerScore {
    fn from(p: ApiPlayerInRound) -> Self {
        Self {
            pdga_number: p.pdga_number,
            round_score: p.round_score,
            round_to_par: p.round_to_par,
            hole_scores: p
                .hole_scores
                .iter()
                .filter_map(|s| s.parse::<u32>().ok())
                .collect(),
        }
    }
}

#[derive(Debug)]
pub struct PlayerScore {
    pub pdga_number: i32,
    pub hole_scores: Vec<u32>,
    pub round_score: u32,
    pub round_to_par: i32,
}

impl PlayerScore {
    pub async fn update_and_save(
        &self,
        db: &impl ConnectionTrait,
        round: i32,
        competition_id: i32,
        div: &entity::sea_orm_active_enums::Division,
    ) -> Result<(), DbErr> {
        if let Some(score_update) =
        self.round_score_active_model(db, round, competition_id, div.clone())
            .await {
            score_update.save(db).await?;
        }
        self.make_sure_player_in_competition(db, competition_id, div)
            .await?;
        Ok(())
    }


    /// Returns ActiveModel if score is changed, otherwise None
    async fn round_score_active_model(
        &self,
        db: &impl ConnectionTrait,
        round: i32,
        competition_id: i32,
        division: entity::sea_orm_active_enums::Division,
    ) -> Option<ActiveModel> {
        let existing_score = player_round_score::Entity::find()
            .filter(player_round_score::Column::PdgaNumber.eq(self.pdga_number))
            .filter(player_round_score::Column::Round.eq(round))
            .one(db)
            .await;

        match existing_score {
            Ok(Some(score)) => {
                if score.score != self.round_score as i32 {
                    let mut score = score.into_active_model();
                    score.score = Set(self.round_score as i32);
                    Some(score)
                } else {
                    None
                }
            }
            Err(_) | Ok(None) => Some(ActiveModel {
                id: NotSet,
                pdga_number: Set(self.pdga_number),
                competition_id: Set(competition_id),
                round: Set(round),
                score: Set(self.round_score as i32),
                division: Set(division),
            }),

        }
    }

    async fn make_sure_player_in_competition(
        &self,
        db: &impl ConnectionTrait,
        competition_id: i32,
        div: &entity::sea_orm_active_enums::Division,
    ) -> Result<(), DbErr> {
        entity::player_in_competition::Entity::insert(entity::player_in_competition::ActiveModel {
            id: NotSet,
            pdga_number: Set(self.pdga_number),
            competition_id: Set(competition_id),
            division: Set(div.clone()),
        })
        .on_conflict(
            sea_query::OnConflict::columns(vec![
                entity::player_in_competition::Column::PdgaNumber,
                entity::player_in_competition::Column::CompetitionId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .do_nothing()
        .exec(db)
        .await?;

        Ok(())
    }
}

#[derive(Deserialize)]
struct RoundFromApi {
    layouts: Vec<Layout>,
    scores: Vec<ApiPlayerInRound>,
}

fn fix_length(length: u32, unit: &Unit) -> u32 {
    match unit {
        Unit::Feet => (length as f64 * 0.3048).round() as u32,
        Unit::Meters => length,
    }
}

impl
    From<(
        RoundFromApi,
        usize,
        usize,
        entity::sea_orm_active_enums::Division,
    )> for RoundInformation
{
    fn from(
        tup: (
            RoundFromApi,
            usize,
            usize,
            entity::sea_orm_active_enums::Division,
        ),
    ) -> Self {
        let round_from_api = tup.0;
        let competition_id = tup.1;
        let round_number = tup.2;
        let layout = round_from_api.layouts.first().unwrap();
        let holes = layout
            .holes
            .iter()
            .map(|h| Hole {
                par: h.par,
                hole_number: h.hole_number,
                length: fix_length(h.length, &layout.unit),
            })
            .collect_vec();

        let length = match layout.unit {
            Unit::Feet => (layout.length as f64 * 0.3048).round() as u32,
            Unit::Meters => (layout.length as f64).round() as u32,
        };

        RoundInformation {
            holes,
            players: round_from_api
                .scores
                .into_iter()
                .map(|p| p.into())
                .collect(),
            course_length: length,
            round_number,
            competition_id,
            div: tup.3,
        }
    }
}

#[derive(Debug)]
pub struct RoundInformation {
    pub holes: Vec<Hole>,
    pub players: Vec<PlayerScore>,
    pub course_length: u32,
    pub round_number: usize,
    pub competition_id: usize,
    pub div: entity::sea_orm_active_enums::Division,
}

impl RoundInformation {
    pub async fn new(
        competition_id: usize,
        round: usize,
        div: Division,
    ) -> Result<Self, reqwest::Error> {
        let div_str = div.to_string().to_uppercase();
        let url = format!("https://www.pdga.com/apps/tournament/live-api/live_results_fetch_round.php?TournID={competition_id}&Round={round}&Division={div_str}");
        //dbg!(&url);
        let resp: ApiRes = reqwest::get(url).await?.json().await?;
        Ok((resp.data, competition_id, round, div.into()).into())
    }

    pub async fn update_all(&self, db: &impl ConnectionTrait) -> Result<(), DbErr> {
        for player in &self.players {
            player
                .update_and_save(
                    db,
                    self.round_number as i32,
                    self.competition_id as i32,
                    &self.div,
                )
                .await?;
        }
        Ok(())
    }

    pub async fn all_player_scores_exist_in_db(
        &self,
        db: &impl ConnectionTrait,
    ) -> Result<bool, DbErr> {
        player_round_score::Entity::find()
            .filter(player_round_score::Column::Round.eq(self.round_number as i32))
            .all(db)
            .await
            .map(|x| x.len() == self.players.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_round_information() {
        let info = RoundInformation::new(65206, 1, Division::MPO)
            .await
            .unwrap();
        assert_eq!(info.holes.len(), 18);
    }
}
