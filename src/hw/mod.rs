mod comms;
mod control;
mod decode;
mod execute;
mod pi;
mod state;

pub use self::{
    control::{HwCtrl, PositionInfo, StateInfo, TryReferenceError},
    decode::error::GCodeError,
    state::StateError,
};
use crate::{comms::ControlComms, settings::Settings};
use anyhow::{Error, Result};
use crossbeam::channel::{self, Sender};
use std::thread::JoinHandle;

pub fn start(
    settings: Settings,
    error_send: Sender<ControlComms<Error>>,
) -> Result<(JoinHandle<()>, JoinHandle<()>, HwCtrl)> {
    let (estop_send, estop_recv) = channel::unbounded();
    let (pi_handle, pi_ctrl) = pi::start(settings.clone(), error_send.clone())?;
    let (estop_handle, executor_ctrl) = execute::start(
        settings.clone(),
        pi_ctrl.clone(),
        estop_recv,
        error_send.clone(),
    )?;
    let hw_ctrl = HwCtrl::new(settings, executor_ctrl, pi_ctrl, estop_send);
    Ok((pi_handle, estop_handle, hw_ctrl))
}
