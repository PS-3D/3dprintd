use super::{json_ok_or, JsonResult};
use crate::{
    api::values::{ApiError, Errors},
    hw::{GCodeError, HwCtrl, StateError, StateInfo},
};
use rocket::{get, http::Status, post, response::status, serde::json::Json, Responder, State};
use serde::Deserialize;
use std::io::Error as IoError;
use std::path::PathBuf;

#[get("/gcode")]
pub fn get(hw_ctrl: &State<HwCtrl>) -> status::Custom<Json<StateInfo>> {
    status::Custom(Status::Ok, Json(hw_ctrl.state_info()))
}

#[derive(Responder)]
pub enum ApiGCodeActionResponse {
    #[response(status = 202)]
    Accepted(()),
    #[response(status = 405)]
    InvalidInput(()),
    #[response(status = 409)]
    StateError(()),
    #[response(status = 500)]
    OtherError(Json<ApiError>),
    #[response(status = 512)]
    IoError(Json<ApiError>),
    #[response(status = 513)]
    GCodeError(Json<ApiError>),
}

#[derive(Debug, Deserialize)]
pub struct ApiPostGCodeStartParams {
    path: PathBuf,
}

#[post("/gcode/start", data = "<params>")]
pub fn post_start(
    params: JsonResult<ApiPostGCodeStartParams>,
    hw_ctrl: &State<HwCtrl>,
    errors: &State<Errors>,
) -> ApiGCodeActionResponse {
    let params = json_ok_or!(params, ApiGCodeActionResponse::InvalidInput(()));
    let canonical_path = match params.path.canonicalize() {
        Ok(p) => p,
        Err(e) => return ApiGCodeActionResponse::IoError(Json(errors.insert_get(e.into()))),
    };
    match hw_ctrl.try_print(canonical_path) {
        Ok(()) => ApiGCodeActionResponse::Accepted(()),
        Err(e) => match e {
            e if e.is::<IoError>() => ApiGCodeActionResponse::IoError(Json(errors.insert_get(e))),
            e if e.is::<StateError>() => ApiGCodeActionResponse::StateError(()),
            e if e.is::<GCodeError>() => {
                ApiGCodeActionResponse::GCodeError(Json(errors.insert_get(e)))
            }
            _ => ApiGCodeActionResponse::OtherError(Json(errors.insert_get(e))),
        },
    }
}

#[post("/gcode/stop")]
pub fn post_stop(hw_ctrl: &State<HwCtrl>) -> status::Accepted<()> {
    hw_ctrl.stop();
    status::Accepted(None)
}

#[post("/gcode/continue")]
pub fn post_continue(hw_ctrl: &State<HwCtrl>) -> ApiGCodeActionResponse {
    match hw_ctrl.try_play() {
        Ok(()) => ApiGCodeActionResponse::Accepted(()),
        Err(_) => ApiGCodeActionResponse::StateError(()),
    }
}

#[post("/gcode/pause")]
pub fn post_pause(hw_ctrl: &State<HwCtrl>) -> ApiGCodeActionResponse {
    match hw_ctrl.try_pause() {
        Ok(()) => ApiGCodeActionResponse::Accepted(()),
        Err(_) => ApiGCodeActionResponse::StateError(()),
    }
}
