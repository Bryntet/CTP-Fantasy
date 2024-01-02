use rocket::http::Status;
use rocket::serde::json::Json;
use rocket_okapi::openapi;
use service::{CompetitionInfoInput, fetch_people_from_competition};

#[openapi(tag="Competition")]
#[post("/refresh-competition", format = "json", data = "<competition>")]
pub(crate) async fn fetch_competition(
    competition: Json<CompetitionInfoInput>,
) -> Result<Status, rocket::http::Status> {
    use entity::sea_orm_active_enums::Division;
    let comp = competition.into_inner();
    println!("Received competition with id: {} and division: {:?}", comp.id, comp.division);
    // Here you can add the code to process the competition data
    let div_string = match comp.division.to_division() {
        Division::Fpo => "FPO".to_string(),
        Division::Mpo => "MPO".to_string(),
    };
    fetch_people_from_competition(comp.id, &div_string, 1).await.unwrap();
    Ok(Status::Ok)
}