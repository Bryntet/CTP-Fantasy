mod authenticate;
mod mutation;
mod query;
mod utils;

use rocket_okapi::openapi_get_routes;

#[macro_use]
extern crate rocket;

use authenticate::*;
use dotenvy::dotenv;
use mutation::*;
use query::*;
use rocket::fs::FileServer;
use rocket::{Build, Rocket};

use rocket_okapi::rapidoc::{make_rapidoc, GeneralConfig, HideShowConfig, RapiDocConfig};
use rocket_okapi::settings::UrlObject;
use rocket_okapi::swagger_ui::{make_swagger_ui, SwaggerUIConfig};
use service::*;

#[catch(404)]
fn general_not_found() -> &'static str {
    "Api endpoint not found"
}

pub async fn launch() -> Rocket<Build> {
    dotenv().ok();

    let db =
        sea_orm::Database::connect(std::env::var("DATABASE_URL").expect("DATABASE_URL not set"))
            .await
            .unwrap();
    let flutter_path = std::env::var("FLUTTER_PATH").expect("FLUTTER_PATH not set");

    rocket::build()
        .manage(db)
        .mount(
            "/api",
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
            ],
        )
        .mount(
            "/api/swagger",
            make_swagger_ui(&SwaggerUIConfig {
                url: "../openapi.json".to_owned(),
                ..Default::default()
            }),
        )
        .mount(
            "/api/",
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
