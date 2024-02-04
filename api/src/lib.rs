#[macro_use]
pub extern crate rocket;

use dotenvy::dotenv;
use rocket::fs::FileServer;
use rocket::{Build, Rocket, Route};
use rocket_okapi::openapi_get_routes;
use rocket_okapi::rapidoc::{make_rapidoc, GeneralConfig, HideShowConfig, RapiDocConfig};
use rocket_okapi::settings::UrlObject;
use rocket_okapi::swagger_ui::{make_swagger_ui, SwaggerUIConfig};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};

use authenticate::*;
use mutation::*;
use query::*;
use service::*;

pub mod authenticate;
pub mod mutation;
pub mod query;
pub mod utils;

pub mod endpoints {
    pub use super::authenticate;
    pub use super::mutation;
    pub use super::query;
    pub use super::utils;
}

#[catch(404)]
fn general_not_found() -> &'static str {
    "Api endpoint not found"
}

async fn get_db(mock: bool) -> DatabaseConnection {
    let url = if mock {
        std::env::var("DEV_DATABASE_URL").expect("DEV_DATABASE_URL not set")
    } else {
        std::env::var("DATABASE_URL").expect("DATABASE_URL not set")
    };

    let mut opt = ConnectOptions::new(url);
    if mock {
        opt.sqlx_logging(true);
    }

    Database::connect(opt).await.expect("CAN'T CONNECT TO DB")
}

pub fn routes() -> Vec<Route> {
    openapi_get_routes![
        create_tournament,
        create_user,
        login,
        add_pick,
        add_picks,
        see_tournaments,
        see_participants,
        invite_user,
        answer_invite,
        get_user_picks,
        check_cookie,
        check_cookie_failed,
        logout,
        get_my_id,
        get_tournament,
        logout_all,
        get_max_picks,
        get_user_pick,
        get_divisions,
        add_competition,
        force_refresh_competitions
    ]
}

pub async fn launch(mock: bool) -> Rocket<Build> {
    dotenv().ok();

    let flutter_path = std::env::var("FLUTTER_PATH").expect("FLUTTER_PATH not set");

    rocket::build()
        .manage(get_db(mock).await)
        .mount("/api", routes())
        .mount(
            "/api/swagger",
            make_swagger_ui(&SwaggerUIConfig {
                url: "../openapi.json".to_owned(),
                ..Default::default()
            }),
        )
        .mount(
            "/api",
            make_rapidoc(&RapiDocConfig {
                general: GeneralConfig {
                    spec_urls: vec![UrlObject::new("General", "./openapi.json")],
                    ..Default::default()
                },
                hide_show: HideShowConfig {
                    allow_spec_url_load: false,
                    allow_spec_file_load: false,
                    ..Default::default()
                },
                ..Default::default()
            }),
        )
        .register("/api", catchers![general_not_found])
        .mount("/", FileServer::from(flutter_path))
    //.configure(release_config)
}
