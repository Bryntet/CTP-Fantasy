use api::launch;
#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    launch().await.launch().await?;
    Ok(())
}
