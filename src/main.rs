mod api;
mod args;
mod comms;
mod config;
mod hw;
mod log;
mod settings;
mod util;

use crate::{comms::ControlComms, log::target};
use anyhow::Result;
use crossbeam::channel;
use tracing::debug;
#[cfg(any(feature = "dev_no_pi", feature = "dev_no_motors"))]
use tracing::warn;

pub const APP_NAME: &'static str = env!("CARGO_BIN_NAME");

// rough outline of main:
//
// read config
// init thread comms
// start values thread
// start pi thread
// start estop & execute thread
// start decode thread
// start api
// wait for api to finish
// stop decode thread
// stop value thread
// stop pi thread
// stop estop thread
// stop values thread
//
// excute starts before decode and ends because execute can exist without input
// but ideally as long as decode exists it can send things to the execute thread
//
// the api will catch sigterm via rocket::Shutdown. it has to terminate before
// the other threads (mainly before the motors thread) so the outstanding requests
// can still be served like requesting some value from the motor.
//
// execute and decode are split so gcode can be decoded while commands are sent and
// received from the motors. this is due to the suspicion that delays in talking
// with the motors might result in bad print quality. this might also warrant
// setting the priorty of the motors thread to a realtime thread. also if we wait
// too long we might not optimally use the timeslots in the RS485 connection.
// the decode thread should be more than fast enough to keep up, the main concern
// with splitting is responsetime.
fn main() -> Result<()> {
    let args = args::args();
    let config = config::config(&args)?;
    log::setup(config.log.level);
    #[cfg(feature = "dev_no_pi")]
    warn!(
        target: target::INTERNAL,
        "Feature dev_no_pi is enabled, binary is not fully functional"
    );
    #[cfg(feature = "dev_no_motors")]
    warn!(
        target: target::INTERNAL,
        "Feature dev_no_motors is enabled, binary is not fully functional"
    );
    debug!(target: target::INTERNAL, "Args are: {:?}", args);
    debug!(target: target::INTERNAL, "Config is: {:?}", config);
    let settings = settings::settings(config)?;
    let (error_send, error_recv) = channel::unbounded();
    let (error_handle, errors) = api::values::start(error_recv)?;
    let (pi_handle, estop_handle, hw_ctrl) = hw::start(settings.clone(), error_send.clone())?;
    api::launch(settings.clone(), errors, hw_ctrl.clone())?;
    debug!(
        target: target::INTERNAL,
        "api exited gracefully, shutting down..."
    );
    hw_ctrl.exit();
    estop_handle.join().unwrap();
    pi_handle.join().unwrap();
    error_send.send(ControlComms::Exit).unwrap();
    error_handle.join().unwrap();
    Ok(())
}

// TODO change settings depending on release or debug build, i.e. maybe stop on ctrlc
// TODO consolidate errors
