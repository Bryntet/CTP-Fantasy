mod authenticate;
mod error;
mod externally_update_internal;
mod mutation;
mod query;
mod utils;

use externally_update_internal as ext_to_int;

use rocket_okapi::{openapi, openapi_get_routes};



#[macro_use]
extern crate rocket;

use dotenvy::dotenv;
use rocket::response::Responder;
use rocket_okapi::rapidoc::{make_rapidoc, GeneralConfig, HideShowConfig, RapiDocConfig};
use rocket_okapi::settings::UrlObject;
use rocket_okapi::swagger_ui::{make_swagger_ui, SwaggerUIConfig};
use service::*;
use std::fmt::Debug;

#[launch]
async fn rocket() -> _ {
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
                mutation::create_tournament,
                ext_to_int::fetch_competition,
                mutation::create_user,
                authenticate::login,
                mutation::add_pick,
                query::see_tournaments,
                query::see_participants,
                mutation::invite_user,
                mutation::answer_invite,
                query::get_user_picks,
                authenticate::check_cookie,
                authenticate::logout,
                query::get_my_id,
                query::get_tournament,

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
