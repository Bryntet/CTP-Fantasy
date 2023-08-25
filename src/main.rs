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
use models::{ Player };
pub mod pdga_handling; 


#[tokio::main]
async fn main() {
    use crate::schema::players::dsl::*;
    use std::{thread, time};
    let conn = &mut establish_connection();
    
    let non_avatars = get_players_without_avatar(conn);


    for i in 64500..65000 {
        let tour_players = pdga_handling::get_tournament(i, "MPO", 1);
        for player in tour_players.await.unwrap_or(vec![]) {
            if !player.exists(conn) {
                player.insert_into_sql(conn);
                println!("Added {:?}", player.pdga_number)
            } else if non_avatars.contains(&player.pdga_number) && player.avatar.is_some() { 
                {
                    diesel::update(players.filter(pdga_number.eq(player.pdga_number)))
                        .set(avatar.eq(player.avatar.clone()))
                        .execute(conn)
                        .expect("Error updating player");
                    println!("Updated {:?}", player.pdga_number)
                }
            }
            
        }
        thread::sleep(time::Duration::from_millis(50));
    } 

    
}


fn get_players_without_avatar(conn: &mut PgConnection) -> Vec<i32> {
    use crate::schema::players::dsl::*;
    players
        .filter(avatar.is_null())
        .select(pdga_number)
        .load(conn)
        .expect("Error loading posts")
}





