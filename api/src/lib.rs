mod authenticate;
mod error;
mod externally_update_internal;
mod mutation;
mod query;
mod utils;

use externally_update_internal as ext_to_int;

use rocket_okapi::openapi_get_routes;

#[macro_use]
extern crate rocket;

use authenticate::*;
use dotenvy::dotenv;
use rocket::{Build, Rocket};
use ext_to_int::*;
use mutation::*;
use query::*;
use rocket_okapi::rapidoc::{make_rapidoc, GeneralConfig, HideShowConfig, RapiDocConfig};
use rocket_okapi::settings::UrlObject;
use rocket_okapi::swagger_ui::{make_swagger_ui, SwaggerUIConfig};
use service::*;
pub async fn launch() -> Rocket<Build> {
    dotenv().ok();
    let db =
        sea_orm::Database::connect(std::env::var("DATABASE_URL").expect("DATABASE_URL not set"))
            .await
            .unwrap();
    rocket::build()
        .manage(db)
        .mount(
            "/api",
            openapi_get_routes![
                create_tournament,
                fetch_competition,
                create_user,
                login,
                add_pick,
                see_tournaments,
                see_participants,
                invite_user,
                answer_invite,
                get_user_picks,
                check_cookie,
                logout,
                get_my_id,
                get_tournament,
                logout_all
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
}