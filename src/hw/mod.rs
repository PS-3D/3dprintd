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
use std::{sync::Arc, thread::JoinHandle};

pub fn start(
    settings: Settings,
    error_send: Sender<ControlComms<Error>>,
) -> Result<(JoinHandle<()>, HwCtrl)> {
    let (estop_send, estop_recv) = channel::unbounded();
    let pi_ctrl = pi::start(settings.clone(), error_send.clone())?;
    let pi_ctrl = Arc::new(pi_ctrl);
    let (estop_handle, executor_ctrl) = execute::start(
        settings.clone(),
        Arc::clone(&pi_ctrl),
        estop_recv,
        error_send.clone(),
    )?;
    let hw_ctrl = HwCtrl::new(settings, executor_ctrl, pi_ctrl, estop_send);
    Ok((estop_handle, hw_ctrl))
}
