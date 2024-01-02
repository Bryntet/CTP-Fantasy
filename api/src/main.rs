mod utils;
mod externally_update_internal;
mod query;
mod mutation;

use externally_update_internal as ext_to_int;

use rocket_okapi::{openapi, openapi_get_routes};



#[macro_use]
extern crate rocket;

use std::fmt::Debug;
use dotenvy::dotenv;
use rocket::response::Responder;
use rocket_okapi::rapidoc::{GeneralConfig, HideShowConfig, make_rapidoc, RapiDocConfig};
use rocket_okapi::settings::UrlObject;
use rocket_okapi::swagger_ui::{make_swagger_ui, SwaggerUIConfig};
use service::*;


#[launch]
async fn rocket() -> _ {
    dotenv().ok();
    let db = sea_orm::Database::connect(std::env::var("DATABASE_URL").expect("DATABASE_URL not set"))
        .await
        .unwrap();
    rocket::build()
        .manage(db)
        //.mount("/api", routes![mutation::create_tournament, ext_to_int::fetch_competition])
        .mount("/", openapi_get_routes![mutation::create_tournament, ext_to_int::fetch_competition])
        .mount(
            "/swagger-ui/",
            make_swagger_ui(&SwaggerUIConfig {
                url: "../openapi.json".to_owned(),
                ..Default::default()
            }),
        )
        .mount(
            "/rapidoc/",
            make_rapidoc(&RapiDocConfig {
                general: GeneralConfig {
                    spec_urls: vec![UrlObject::new("General", "../openapi.json")],
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
