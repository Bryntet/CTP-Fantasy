use sqlx::postgres::PgPool;
use std::env;
use rand::Rng;

#[derive(sqlx::FromRow)]
struct Player {
    first_name: String,
    last_name: Option<String>,
    pdga_number: i32,
    rating: Option<i32>
}

impl Player {
    fn new(first_name: String, last_name: Option<String>, pdga_number: i32, rating: Option<i32>) -> Self {
        Self {
            first_name,
            last_name,
            pdga_number,
            rating
        }
    }

    async fn add_to_db(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
        sqlx::query!("INSERT INTO Players (first_name, last_name, pdga_number, rating) VALUES ($1, $2, $3, $4)", self.first_name, self.last_name, self.pdga_number, self.rating)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn exists(&self, pool: &PgPool) -> bool {
        let player = sqlx::query_as!(Player, "SELECT * FROM Players WHERE pdga_number = $1", self.pdga_number)
            .fetch_optional(pool)
            .await;
        player.is_ok()
    }

}

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    // Let's use an environment variable for the connection string.
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://brynte:password@localhost/ctp-fantasy".into()
    });

    // Creating a connection pool
    let pool = PgPool::connect(&database_url).await?;

    // A little query to fetch all the first names from the Players table
    let players: Vec<Player> = sqlx::query_as!(Player, "SELECT * FROM Players")
        .fetch_all(&pool)
        .await?;
    


    for player in players {
        println!("Player: {}", player.first_name);
    }
    let mut rng = rand::thread_rng();

    let frederik = sqlx::query!("INSERT INTO Players (first_name, pdga_number) VALUES ($1, $2)", "Frederik", rng.gen_range(0..10))
        .execute(&pool)
        .await?;


    let tjena = Player { first_name: "Frederik".to_string(), last_name: None, pdga_number: 0, rating: None};
    sqlx::query!("INSERT INTO Players (first_name, pdga_number) VALUES ($1, $2)", tjena.first_name, tjena.pdga_number)
        .execute(&pool)
        .await?;
        
    println!("Frederik: {:?}", frederik);

    Ok(())
}


