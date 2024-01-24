use std::time::Duration;
use tokio::time::interval;
use api::launch;
use service::dto::Division;

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            dbg!(service::dto::get_round_information(65206,1, Division::MPO).await);
            interval.tick().await;
        }
    });
    launch().await.launch().await?;
    Ok(())
}

