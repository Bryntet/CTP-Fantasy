use dotenvy::dotenv;

mod pdga_handling;

#[tokio::main]
async fn main() {
    pdga_handling::fetch_lots_of_people().await
}



