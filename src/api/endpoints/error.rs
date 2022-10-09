use crate::api::values::{ApiError, Errors};
use rocket::{get, http::Status, response::status, serde::json::Json, Responder, State};
use serde::Serialize;

#[derive(Serialize)]
pub struct ErrorPage {
    pub page: usize,
    pub errors: Vec<ApiError>,
}

#[get("/errors?<page>")]
pub fn get(page: Option<usize>, errors: &State<Errors>) -> status::Custom<Json<ErrorPage>> {
    let page = page.unwrap_or(0);
    status::Custom(
        Status::Ok,
        Json(ErrorPage {
            page,
            errors: errors.get_page(page, 10),
        }),
    )
}

#[derive(Responder)]
pub enum LastError {
    #[response(status = 200)]
    Error(Json<ApiError>),
    #[response(status = 204)]
    NoError(()),
}

#[get("/error/last")]
pub fn get_last(errors: &State<Errors>) -> LastError {
    match errors.get_last() {
        Some(e) => LastError::Error(Json(e)),
        None => LastError::NoError(()),
    }
}

#[get("/error/<id>")]
pub fn get_id(
    id: u64,
    errors: &State<Errors>,
) -> Result<status::Custom<Json<ApiError>>, status::NotFound<()>> {
    errors
        .get(id)
        .map(|e| status::Custom(Status::Ok, Json(e)))
        .ok_or(status::NotFound(()))
}
