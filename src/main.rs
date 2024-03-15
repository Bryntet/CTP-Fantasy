use api::launch;
use dotenvy::dotenv;
use sea_orm::DatabaseConnection;

use rocket::error;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use service::dto::CompetitionInfo;
use std::time::Duration;

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    dotenv().ok();
    let mut round_update_interval = tokio::time::interval(Duration::from_secs(60));
    let mut player_insert_interval = tokio::time::interval(Duration::from_secs(60 * 10));

    // Start the background task to update the scores of all active competitions
    tokio::spawn(async move {
        let db = api::get_db().await;
        loop {
            if let Err(e) = service::mutation::refresh_user_scores_in_all(&db).await {
                error!("Unable to refresh global user scores {:#?}", e);
            }
            check_active_rounds(&db).await;
            round_update_interval.tick().await;
        }
    });

    // Start the background task to insert new players into the database
    tokio::spawn(async move {
        let db = api::get_db().await;
        loop {
            if let Ok(comps) = entity::competition::Entity::find()
                .filter(
                    entity::competition::Column::Status
                        .eq(entity::sea_orm_active_enums::CompetitionStatus::NotStarted),
                )
                .all(&db)
                .await
            {
                for comp in comps {
                    let comp_info = CompetitionInfo::from_web(comp.id as u32).await.unwrap();
                    comp_info.save_round_scores(&db).await.unwrap();
                }
            };
            player_insert_interval.tick().await;
        }
    });

    launch().await.launch().await.unwrap();

    Ok(())
}
// TODO: Refactor so that this function only switches a finished competition from running to finished.
async fn check_active_rounds(db: &DatabaseConnection) {
    let _ = service::mutation::update_active_competitions(db)
        .await
        .map_err(|e| {
            error!("Unable to update active competitions {:#?}", e);
        });
}
