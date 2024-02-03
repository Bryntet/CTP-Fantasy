#[macro_use] extern crate rocket;

use rocket::Config;
use sea_orm::{ConnectOptions, Database, DatabaseConnection, EntityTrait};

async fn make_db() -> DatabaseConnection {
    let db_url =std::env::var("DEV_DATABASE_URL").expect("DEV_DATABASE_URL not set");
    let opt = ConnectOptions::new(db_url);
    //opt.sqlx_logging(true);
    Database::connect(opt).await.expect("Database must exist")
}

#[launch]
async fn rocket() -> _ {
    let config = Config {
        log_level: rocket::config::LogLevel::Off,
        ..Default::default()
    };
    rocket::build().manage(make_db().await)
        .mount("/", api::routes()).configure(config)
}

async fn any_round_scores() -> bool {
    let scores = entity::player_round_score::Entity::find().all(&make_db().await).await.unwrap();
    !scores.is_empty()
}

#[cfg(test)]
mod tests {
    use dotenvy::dotenv;
    use rocket::local::asynchronous::Client;

    use api::rocket;
    use migration::MigratorTrait;
    use service::dto::UserLogin;
    use crate::any_round_scores;

    #[rocket::async_test]
    async fn test_process() {
        dotenv().ok();

        let db = sea_orm::Database::connect(
            std::env::var("DEV_DATABASE_URL").expect("DEV_DATABASE_URL not set"),
        )
        .await
        .unwrap();
        migration::Migrator::fresh(&db)
            .await
            .expect("Migration success");

        db.close().await.expect("Closing db failed");
        let client = Client::tracked(super::rocket().await)
            .await
            .expect("valid rocket instance");

        // Create a User
        let u = UserLogin {
            username: "test_user".to_string(),
            password: "test_password".to_string(),
        };
        client
            .post("/create-user")
            .json(&u).dispatch().await;

        let new_tournament = service::dto::CreateTournament {
            divisions: vec![service::dto::Division::MPO, service::dto::Division::FPO],
            max_picks_per_user: Some(3),
            name: "test_tournament".to_string(),
        };
        client.post("/fantasy-tournament").json(&new_tournament).dispatch().await;
        let new_competition = service::dto::forms::AddCompetition {
            competition_id: 73691,
            level: service::dto::CompetitionLevel::Major,
        };

        client.post("/fantasy-tournament/1/competition/add").json(&new_competition).dispatch().await;
        assert!(any_round_scores().await);
    }
}
