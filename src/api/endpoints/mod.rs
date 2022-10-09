pub mod axis;
pub mod error;
pub mod gcode;
pub mod heating;

use crate::comms::{ControlComms, EStopComms};
use crossbeam::channel::Sender;
use rocket::{post, response::status, State};

#[post("/estop")]
pub fn post_estop(estop_send: &State<Sender<ControlComms<EStopComms>>>) -> status::Accepted<()> {
    estop_send
        .send(ControlComms::Msg(EStopComms::EStop))
        .unwrap();
    status::Accepted(None)
}
