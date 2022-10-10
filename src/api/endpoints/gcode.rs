use std::sync::Arc;

use crate::{
    comms::{ControlComms, DecoderComms},
    decode::Decoder,
};
use crossbeam::channel::Sender;
use rocket::{get, http::Status, post, response::status, State};

#[get("/gcode")]
pub fn get() -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
}

#[post("/gcode/start")]
pub fn post_start() -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
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
