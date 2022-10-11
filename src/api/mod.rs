mod endpoints;
mod error;
pub mod values;

use self::values::Errors;
use crate::{hw::HwCtrl, settings::Settings};
use anyhow::Result;
use rocket::{catchers, config::Config as RocketConfig, routes};

pub fn launch(settings: Settings, errors: Errors, hw_ctrl: HwCtrl) -> Result<()> {
    let routes_v0 = {
        use self::endpoints::*;
        routes![
            post_estop,
            gcode::get,
            gcode::post_start,
            gcode::post_stop,
            gcode::post_continue,
            gcode::post_pause,
            axis::get_position,
            axis::get_axis_name_position,
            axis::get_axis_name_settings,
            axis::get_e_settings,
            axis::put_axis_name_settings,
            axis::put_e_settings,
            axis::post_axis_xy_reference,
            axis::post_axis_z_reference,
            heating::get_hotend_settings,
            heating::get_bed_settings,
            heating::get_chamber_settings,
            heating::put_hotend_settings,
            heating::put_bed_settings,
            heating::put_chamber_settings,
            error::get,
            error::get_last,
            error::get_id,
        ]
    };
    rocket::execute(
        rocket::build()
            .configure::<RocketConfig>((&settings.config().api).into())
            .manage(settings)
            .manage(errors)
            .manage(hw_ctrl)
            .mount("/v0/", routes_v0)
            .register("/", catchers![endpoints::catch_404])
            .launch(),
    )
    .map(|_| ())
    .map_err(Into::into)
}
