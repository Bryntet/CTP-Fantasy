use api::launch;
use dotenvy::dotenv;
use sea_orm::DatabaseConnection;

use rocket::log::private::LevelFilter;
use std::time::Duration;
use rocket::fairing::AdHoc;

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    dotenv().ok();
    let mut round_update_interval = tokio::time::interval(Duration::from_secs(60));

    tokio::spawn(async move {
        let db = api::get_db().await;
        loop {
            check_active_rounds(&db).await;
            dbg!("hi");
            service::mutation::refresh_user_scores_in_all(&db).await;
            round_update_interval.tick().await;
        }
    });
    launch().await.launch().await.unwrap();

    Ok(())
}

async fn check_active_rounds(db: &DatabaseConnection) {
    service::mutation::update_active_rounds(db).await;
}
