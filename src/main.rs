use api::launch;
use dotenvy::dotenv;
use sea_orm::{DatabaseConnection, IntoActiveModel, TransactionTrait};
use service::dto::Division;
use std::time::Duration;
use tokio::time::interval;

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    dotenv().ok();

    let db =
        sea_orm::Database::connect(std::env::var("DATABASE_URL").expect("DATABASE_URL not set"))
            .await
            .unwrap();

    let mut round_update_interval = tokio::time::interval(Duration::from_secs(30));
    let mut event_status_check_interval = tokio::time::interval(Duration::from_secs(120));

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

    launch().await.launch().await?;
    Ok(())
}

async fn check_active_rounds(db: &DatabaseConnection) {
    if let Ok(rounds) = service::query::active_rounds(db).await {
        for round in rounds {
            if let Ok(txn) = db.begin().await {
                if let Ok(round_info) = service::dto::RoundInformation::new(
                    round.competition_id as usize,
                    round.round_number as usize,
                    Division::MPO,
                )
                .await
                {
                    if let Err(e) = round_info.update_all(&txn).await {
                        dbg!(e);
                    }
                }
                if let Err(e) = txn.commit().await {
                    dbg!(e);
                }
            }
        }
    }
}
