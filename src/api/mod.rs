mod axis;
mod gcode;
mod heating;

use crate::{
    comms::{ControlComms, DecoderComms, EStopComms},
    settings::Settings,
};
use anyhow::Result;
use crossbeam::channel::Sender;
use rocket::{config::Config as RocketConfig, post, routes, State};

#[post("/estop")]
fn post_estop(estop_send: &State<Sender<ControlComms<EStopComms>>>) {
    estop_send
        .send(ControlComms::Msg(EStopComms::EStop))
        .unwrap();
}

pub fn launch(
    settings: Settings,
    decoder_send: Sender<ControlComms<DecoderComms>>,
    estop_send: Sender<ControlComms<EStopComms>>,
) -> Result<()> {
    rocket::execute(
        rocket::build()
            .configure::<RocketConfig>((&settings.config().api).into())
            .manage(settings)
            .manage(decoder_send)
            .manage(estop_send)
            .mount(
                "/v0/",
                routes![
                    post_estop,
                    gcode::get,
                    gcode::post_start,
                    gcode::post_stop,
                    gcode::post_continue,
                    gcode::post_pause,
                    axis::get,
                    axis::get_axis_name_position,
                    axis::get_axis_name_settings,
                    axis::get_e_settings,
                    axis::put_axis_name_settings,
                    axis::put_e_settings,
                    axis::post_axis_name_reference,
                    heating::get_hotend_settings,
                    heating::get_bed_settings,
                    heating::get_chamber_settings,
                    heating::put_hotend_settings,
                    heating::put_bed_settings,
                    heating::put_chamber_settings,
                ],
            )
            .launch(),
    )
    .map(|_| ())
    .map_err(Into::into)
}
