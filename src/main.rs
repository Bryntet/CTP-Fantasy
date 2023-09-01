use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenvy::dotenv;
use std::env;

pub fn establish_connection() -> PgConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}

pub mod models;
pub mod schema;
pub mod pdga_handling; 
use models::{ Player, User, SQLStuff};


#[tokio::main]
async fn main() {
    let conn = &mut establish_connection();
    
    // for i in 64500..=65000 {
    //     get_players_from_tournament(conn, i);
    //     println!("Added round: {}", i);
    // }
    
    let my_user = User::from_id(1, conn).unwrap();
    
    println!("{:?}", my_user.get_players(conn));
}


async fn get_players_from_tournament(conn: &mut PgConnection, i: i32) {
    let tour_players = pdga_handling::get_tournament(i, "MPO", 1);
    for player in tour_players.await.unwrap_or(vec![]) {
        if !player.exists(conn) {
            player.insert_into_sql(conn);
        } 
    }
}



