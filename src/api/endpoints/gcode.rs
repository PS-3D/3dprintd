use std::path::PathBuf;

use super::{json_ok_or, JsonResult};
use crate::{
    api::values::{ApiError, Errors},
    decode::{
        error::{GCodeError, StateError},
        DecoderCtrl, PrintingStateInfo, StateInfo,
    },
};
use rocket::{get, http::Status, post, response::status, serde::json::Json, Responder, State};
use serde::{Deserialize, Serialize};
use std::io::Error as IoError;

#[derive(Debug, Serialize)]
pub struct ApiGetGCodePrintingPaused {
    path: PathBuf,
    line: usize,
}

impl From<PrintingStateInfo> for ApiGetGCodePrintingPaused {
    fn from(info: PrintingStateInfo) -> Self {
        Self {
            path: info.path,
            line: info.current_line,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum ApiGetGCodeStatus {
    Printing(ApiGetGCodePrintingPaused),
    Paused(ApiGetGCodePrintingPaused),
    Stopped,
}

impl From<StateInfo> for ApiGetGCodeStatus {
    fn from(info: StateInfo) -> Self {
        match info {
            StateInfo::Printing(i) => Self::Printing(i.into()),
            StateInfo::Paused(i) => Self::Paused(i.into()),
            StateInfo::Stopped => Self::Stopped,
        }
    }
}

#[get("/gcode")]
pub fn get(decoder: &State<DecoderCtrl>) -> status::Custom<Json<ApiGetGCodeStatus>> {
    status::Custom(Status::Ok, Json(decoder.state_info().into()))
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
    decoder: &State<DecoderCtrl>,
    errors: &State<Errors>,
) -> ApiGCodeActionResponse {
    let params = json_ok_or!(params, ApiGCodeActionResponse::InvalidInput(()));
    let canonical_path = match params.path.canonicalize() {
        Ok(p) => p,
        Err(e) => return ApiGCodeActionResponse::IoError(Json(errors.insert_get(e.into()))),
    };
    match decoder.try_print(canonical_path) {
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
pub fn post_stop(decoder_ctrl: &State<DecoderCtrl>) -> status::Accepted<()> {
    decoder_ctrl.stop();
    status::Accepted(None)
}

#[post("/gcode/continue")]
pub fn post_continue(decoder_ctrl: &State<DecoderCtrl>) -> ApiGCodeActionResponse {
    match decoder_ctrl.try_play() {
        Ok(()) => ApiGCodeActionResponse::Accepted(()),
        Err(_) => ApiGCodeActionResponse::StateError(()),
    }
}

#[post("/gcode/pause")]
pub fn post_pause(decoder: &State<DecoderCtrl>) -> ApiGCodeActionResponse {
    match decoder.try_pause() {
        Ok(()) => ApiGCodeActionResponse::Accepted(()),
        Err(_) => ApiGCodeActionResponse::StateError(()),
    }
}
