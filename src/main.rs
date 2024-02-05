
use api::launch;
use dotenvy::dotenv;
use sea_orm::DatabaseConnection;

use std::time::Duration;
use rocket::log::private::LevelFilter;


#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    dotenv().ok();



    let mut round_update_interval = tokio::time::interval(Duration::from_secs(60));



    tokio::spawn(async move {
        let mut opt = sea_orm::ConnectOptions::new(std::env::var("DATABASE_URL").expect("DATABASE_URL not set"));
        #[cfg(debug_assertions)]
        opt.sqlx_logging(true);
        opt.sqlx_logging_level(LevelFilter::Trace);
        #[cfg(not(debug_assertions))]
        opt.sqlx_logging(false);

        let db = sea_orm::Database::connect(opt).await
            .unwrap();
        loop {
            check_active_rounds(&db).await;
            service::mutation::refresh_user_scores_in_all(&db).await.expect("PANIC WHY WRONG");
            round_update_interval.tick().await;
        }
    });

    launch(false).await.launch().await.unwrap();

    Ok(())
}

async fn check_active_rounds(db: &DatabaseConnection) {
    service::mutation::update_active_rounds(db).await;
}
