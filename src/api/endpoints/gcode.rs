use std::{path::PathBuf, sync::Arc};

use super::{json_ok_or, JsonResult};
use crate::{
    api::values::{ApiError, Errors},
    comms::{ControlComms, DecoderComms},
    decode::{
        error::{GCodeError, StateError},
        Decoder,
    },
};
use crossbeam::channel::Sender;
use rocket::{get, http::Status, post, response::status, serde::json::Json, Responder, State};
use serde::Deserialize;
use std::io::Error as IoError;

#[get("/gcode")]
pub fn get() -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
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
    decoder: &State<Arc<Decoder>>,
    decoder_send: &State<Sender<ControlComms<DecoderComms>>>,
    errors: &State<Errors>,
) -> ApiGCodeActionResponse {
    let params = json_ok_or!(params, ApiGCodeActionResponse::InvalidInput(()));
    match decoder.as_ref().try_print(params.path) {
        Ok(()) => {
            decoder_send
                .send(ControlComms::Msg(DecoderComms::StateChanged))
                .unwrap();
            ApiGCodeActionResponse::Accepted(())
        }
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
pub fn post_stop(
    decoder: &State<Arc<Decoder>>,
    decoder_send: &State<Sender<ControlComms<DecoderComms>>>,
) -> status::Accepted<()> {
    decoder.stop();
    decoder_send
        .send(ControlComms::Msg(DecoderComms::StateChanged))
        .unwrap();
    status::Accepted(None)
}

#[post("/gcode/continue")]
pub fn post_continue() -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
}

#[post("/gcode/pause")]
pub fn post_pause() -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
}
