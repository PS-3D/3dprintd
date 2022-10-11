mod comms;
mod control;
mod decode;
mod execute;
mod pi;
mod state;

pub use self::{
    control::{HwCtrl, PositionInfo},
    decode::error::GCodeError,
    state::{StateError, StateInfo},
};
use crate::{comms::ControlComms, settings::Settings};
use anyhow::{Error, Result};
use crossbeam::channel::{self, Sender};
use std::thread::JoinHandle;

pub fn start(
    settings: Settings,
    error_send: Sender<ControlComms<Error>>,
) -> Result<(JoinHandle<()>, JoinHandle<()>, JoinHandle<()>, HwCtrl)> {
    let (estop_send, estop_recv) = channel::unbounded();
    let (executor_ctrl_send, executor_ctrl_recv) = channel::unbounded();
    let (executor_manual_send, executor_manual_recv) = channel::unbounded();
    let (executor_handle, estop_handle, oneway_pos_read) = execute::start(
        settings.clone(),
        executor_ctrl_recv,
        executor_manual_recv,
        estop_recv,
        error_send.clone(),
    )?;
    let (decoder_handle, decoder_ctrl) = decode::start(settings);
    let hw_ctrl = HwCtrl::new(
        decoder_ctrl,
        executor_ctrl_send,
        executor_manual_send,
        estop_send,
        oneway_pos_read,
    );
    Ok((executor_handle, estop_handle, decoder_handle, hw_ctrl))
}
