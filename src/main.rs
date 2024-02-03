use api::launch;
use dotenvy::dotenv;
use sea_orm::DatabaseConnection;

use std::time::Duration;

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    dotenv().ok();

    let db =
        sea_orm::Database::connect(std::env::var("DATABASE_URL").expect("DATABASE_URL not set"))
            .await
            .unwrap();

    let mut round_update_interval = tokio::time::interval(Duration::from_secs(30));
    let mut event_status_check_interval = tokio::time::interval(Duration::from_secs(120));

    launch(false).await.launch().await.unwrap();

    tokio::spawn(async move {
        loop {
            check_active_rounds(&db).await;
            round_update_interval.tick().await;
        }
    });

    tokio::spawn(async move {
        loop {
            event_status_check_interval.tick().await;
        }
    });

    Ok(())
}

async fn check_active_rounds(db: &DatabaseConnection) {
    service::mutation::update_active_rounds(db).await;
}
