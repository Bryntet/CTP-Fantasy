#[macro_use]
extern crate rocket;

use dotenvy::dotenv;
use rocket::log::private::LevelFilter;
use rocket::Config;
use rocket::local::asynchronous::Client;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait};
use migration::MigratorTrait;
use service::dto::UserLogin;

async fn make_db() -> DatabaseConnection {
    let db_url = std::env::var("DEV_DATABASE_URL").expect("DEV_DATABASE_URL not set");
    let mut opt = ConnectOptions::new(db_url);
    opt.sqlx_logging(false);
    opt.sqlx_logging_level(LevelFilter::Off);
    Database::connect(opt).await.expect("Database must exist")
}

#[launch]
async fn rocket() -> _ {
    let config = Config {
        log_level: rocket::config::LogLevel::Normal,
        cli_colors: true,
        secret_key: rocket::config::SecretKey::from(&[1u8;64]),
        ..Default::default()
    };
    rocket::build()
        .manage(make_db().await)
        .mount("/", api::routes())
        .configure(config)
}

async fn any_round_scores(db: &impl ConnectionTrait) -> bool {
    let scores = entity::player_round_score::Entity::find()
        .all(db)
        .await
        .unwrap();
    !scores.is_empty()
}

async fn any_user_scores(db: &impl ConnectionTrait) -> bool {
    let scores = entity::user_competition_score_in_fantasy_tournament::Entity::find()
        .all(db)
        .await
        .ok();
    scores.map(|s| !s.is_empty()).unwrap_or(false)
}

async fn clear_db() -> DatabaseConnection {
    dotenv().ok();
    let db = make_db().await;
    migration::Migrator::fresh(&db)
        .await
        .expect("Migration success");
    db
}

async fn make_tracked_client() -> Client {
    Client::tracked(crate::rocket().await)
        .await
        .expect("valid rocket instance")
}


async fn create_user(client: &Client) {
    let u = UserLogin {
        username: "test_user".to_string(),
        password: "test_password".to_string(),
    };
    #[allow(unused_variables)]
    let res = client.post("/create-user").json(&u).dispatch().await;
    #[cfg(debug_assertions)]
    dbg!(&res,res.body());
}

async fn any_user(db: &DatabaseConnection) -> bool {
    let users = entity::user::Entity::find().all(db).await.unwrap();
    !users.is_empty()
}
async fn create_tournament(client: &Client) {
    let new_tournament = service::dto::CreateTournament {
        divisions: vec![service::dto::Division::MPO, service::dto::Division::FPO],
        max_picks_per_user: Some(3),
        name: "test_tournament".to_string(),
    };
    #[allow(unused_variables)]
    let res = client
        .post("/fantasy-tournament")
        .json(&new_tournament)
        .dispatch()
        .await;
    #[cfg(debug_assertions)]
    dbg!(&res,res.body());
}
async fn any_tournament(db: &DatabaseConnection) -> bool {
    let tournaments = entity::fantasy_tournament::Entity::find()
        .all(db)
        .await
        .unwrap();
    !tournaments.is_empty()
}
async fn add_competition(client: &Client, competition_id: u32, level: service::dto::CompetitionLevel) {
    let new_competition = service::dto::forms::AddCompetition {
        competition_id,
        level
    };
    #[allow(unused_variables)]
    let res = client
        .post("/fantasy-tournament/1/competition/add")
        .json(&new_competition)
        .dispatch()
        .await;
    #[cfg(debug_assertions)]
    dbg!(res.into_string().await.unwrap());

}
async fn any_competition(db: &DatabaseConnection) -> bool {
    let competitions = entity::competition::Entity::find()
        .all(db)
        .await
        .unwrap();
    !competitions.is_empty()
}

pub async fn add_pick(client: &Client, player: i32, div: service::dto::Division) {
    let div = div.to_string().to_uppercase();
    let res = client
        .put(format!("/fantasy-tournament/1/user/1/picks/div/{div}/1/{player}"))
        .dispatch()
        .await;
    warn!("{}",res.into_string().await.unwrap());
}
async fn any_pick(db: &DatabaseConnection) -> bool {
    let picks = entity::fantasy_pick::Entity::find()
        .all(db)
        .await
        .unwrap();
    !picks.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{any_round_scores, any_user_scores};
    use api::rocket;
    use service::dto::{CompetitionLevel, Division};

    #[rocket::async_test]
    async fn make_score_test() {
        let db = clear_db().await;

        let client = make_tracked_client().await;

        create_user(&client).await;
        assert!(any_user(&db).await);

        create_tournament(&client).await;
        assert!(any_tournament(&db).await);

        add_competition(&client, 73836, CompetitionLevel::Major).await;
        assert!(any_competition(&db).await);

        add_pick(&client, 7438, Division::FPO).await;
        assert!(any_pick(&db).await);

        assert!(any_round_scores(&db).await);
        assert!(any_user_scores(&db).await);
    }
}
