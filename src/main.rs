mod api;
mod comms;
mod decode;
mod execute;
mod pi;
mod settings;
mod util;

use crate::comms::ControlComms;
use anyhow::Result;
use crossbeam::channel;
use tracing::Level;
use tracing_subscriber;

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
    // TODO swap out for something better
    tracing_subscriber::fmt::fmt()
        .with_max_level(Level::DEBUG)
        .init();
    let settings = settings::settings()?;
    let (error_send, error_recv) = channel::unbounded();
    let (error_handle, errors) = api::values::start(error_recv);
    let (estop_send, estop_recv) = channel::unbounded();
    let (executor_send, executor_recv) = channel::bounded(16);
    let (executor_handle, estop_handle) = execute::start(
        settings.clone(),
        executor_recv,
        estop_recv,
        error_send.clone(),
    )?;
    let (decoder_send, decoder_recv) = channel::unbounded();
    let (decoder_handle, decoder) = decode::start(settings.clone(), decoder_recv, executor_send);
    api::launch(
        settings.clone(),
        errors,
        decoder,
        decoder_send.clone(),
        estop_send.clone(),
    )?;
    decoder_send.send(ControlComms::Exit).unwrap();
    decoder_handle.join().unwrap();
    executor_handle.join().unwrap();
    estop_send.send(ControlComms::Exit).unwrap();
    estop_handle.join().unwrap();
    error_send.send(ControlComms::Exit).unwrap();
    error_handle.join().unwrap();
    Ok(())
}
