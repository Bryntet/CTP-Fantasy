use api::launch;
use dotenvy::dotenv;
use sea_orm::DatabaseConnection;

use rocket::error;
use std::time::Duration;

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    dotenv().ok();
    let mut round_update_interval = tokio::time::interval(Duration::from_secs(60));

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
