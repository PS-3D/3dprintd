mod callbacks;
mod comms;
mod decode;
mod execute;
mod pi;
mod state;

use self::{
    callbacks::{EStopCallback, StopCallback},
    comms::EStopComms,
    execute::{ExecutorCtrl, ExecutorStopper, OutOfBoundsError},
    pi::PiCtrl,
    state::{State, StateInfo as InnerStateInfo},
};
pub use self::{decode::error::GCodeError, state::StateError};
use crate::{
    comms::{Axis, ControlComms, ReferenceRunOptParameters},
    settings::Settings,
    util::ensure_own,
};
use anyhow::{ensure, Error, Result};
use crossbeam::channel::{self, Sender};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
    thread::JoinHandle,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TryReferenceError {
    #[error(transparent)]
    StateError(#[from] StateError),
    #[error(transparent)]
    OutOfBoundsError(#[from] OutOfBoundsError),
}

#[derive(Debug, Serialize)]
pub struct PrintingStateInfo {
    path: PathBuf,
    line: usize,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum StateInfo {
    Printing(PrintingStateInfo),
    Paused(PrintingStateInfo),
    Stopped,
}

impl StateInfo {
    pub(self) fn new(state: InnerStateInfo, line: usize) -> Self {
        match state {
            InnerStateInfo::Printing(path) => Self::Printing(PrintingStateInfo { path, line }),
            InnerStateInfo::Paused(path) => Self::Paused(PrintingStateInfo { path, line }),
            InnerStateInfo::Stopped => Self::Stopped,
        }
    }
}

pub struct PositionInfo {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

struct ExecutorGCodeCallback {
    state: Arc<RwLock<State>>,
}

impl ExecutorGCodeCallback {
    fn new(state: Arc<RwLock<State>>) -> Self {
        Self { state }
    }
}

impl StopCallback for ExecutorGCodeCallback {
    fn stop(&self) {
        let mut state = self.state.write().unwrap();
        // TODO maybe ensure that heaters etc. are turned off?
        state.stop();
    }
}

struct PiCtrlCallbacks {
    // state: Arc<RwLock<State>>,
    // executor_stopper: ExecutorStopper,
    estop_send: Sender<ControlComms<EStopComms>>,
}

// TODO uncomment once estop on the pi thread is actually implemented
// impl EStopCallback for PiCtrlCallbacks {
//     fn estop(&self) {
//         self.estop_send
//             .send(ControlComms::Msg(EStopComms::EStop))
//             .unwrap()
//     }
// }

#[derive(Debug, Clone)]
pub struct HwCtrl {
    state: Arc<RwLock<State>>,
    settings: Settings,
    executor_ctrl: Arc<ExecutorCtrl>,
    pi_ctrl: Arc<PiCtrl>,
    estop_send: Sender<ControlComms<EStopComms>>,
}

macro_rules! pos_info_axis {
    ($func_name:ident, $get_func:ident) => {
        pub fn $func_name(&self) -> f64 {
            self.executor_ctrl.$get_func()
        }
    };
}

impl HwCtrl {
    pub(self) fn init(
        settings: Settings,
        error_send: Sender<ControlComms<Error>>,
    ) -> Result<(JoinHandle<()>, Self)> {
        let state = Arc::new(RwLock::new(State::new()));
        // lock state so we have sole control over the state and noone else
        // can for example report an error until all parts are fully initialised
        let _lock = state.write().unwrap();
        let pi_ctrl = pi::start(settings.clone(), error_send.clone())?;
        let pi_ctrl = Arc::new(pi_ctrl);
        let (estop_send, estop_recv) = channel::unbounded();
        let (exec_stopper, exec_start) = execute::init();
        let (estop_handle, executor_ctrl) =
            exec_start(settings.clone(), pi_ctrl.clone(), estop_recv, error_send)?;
        // since we're done with the setup we can unlock state to be able to move
        // it
        drop(_lock);
        Ok((
            estop_handle,
            Self {
                state,
                settings,
                executor_ctrl: Arc::new(executor_ctrl),
                pi_ctrl,
                estop_send,
            },
        ))
    }

    pub fn state_info(&self) -> StateInfo {
        let state = self.state.read().unwrap();
        StateInfo::new(state.info(), self.executor_ctrl.current_line())
    }

    pos_info_axis!(pos_info_x, pos_x);
    pos_info_axis!(pos_info_y, pos_y);
    pos_info_axis!(pos_info_z, pos_z);

    pub fn pos_info(&self) -> PositionInfo {
        PositionInfo {
            x: self.pos_info_x(),
            y: self.pos_info_y(),
            z: self.pos_info_z(),
        }
    }

    pub fn try_reference_axis(
        &self,
        axis: Axis,
        parameters: ReferenceRunOptParameters,
    ) -> Result<(), TryReferenceError> {
        let state = self.state.read().unwrap();
        ensure_own!(state.is_stopped(), StateError::NotStopped);
        self.executor_ctrl
            .reference_axis(axis, parameters)
            .map_err(Into::into)
    }

    pub fn try_reference_z_hotend(&self) -> Result<(), StateError> {
        let state = self.state.read().unwrap();
        ensure_own!(state.is_stopped(), StateError::NotStopped);
        self.executor_ctrl.reference_z_hotend();
        Ok(())
    }

    /// Tries to start a print
    ///
    /// Should only be used by the API thread, not the decoder thread
    pub fn try_print(&self, path: PathBuf) -> Result<()> {
        let mut state = self.state.write().unwrap();
        ensure!(state.is_stopped(), StateError::NotStopped);
        self.executor_ctrl.print(
            path.clone(),
            Box::new(ExecutorGCodeCallback::new(Arc::clone(&self.state))),
        )?;
        state.print(path);
        Ok(())
    }

    pub fn stop(&self) {
        let mut state = self.state.write().unwrap();
        self.executor_ctrl.stop();
        state.stop();
    }

    pub fn try_play(&self) -> Result<(), StateError> {
        let mut state = self.state.write().unwrap();
        ensure_own!(!state.is_stopped(), StateError::Stopped);
        self.executor_ctrl.play();
        state.play();
        Ok(())
    }

    /// Tries to pause a print
    ///
    /// Should only be used by the API thread, not the decoder thread
    pub fn try_pause(&self) -> Result<(), StateError> {
        let mut state = self.state.write().unwrap();
        ensure_own!(!state.is_stopped(), StateError::Stopped);
        self.executor_ctrl.pause();
        state.pause();
        Ok(())
    }

    pub fn estop(&self) {
        self.estop_send
            .send(ControlComms::Msg(EStopComms::EStop))
            .unwrap()
    }

    pub fn exit(self) {
        drop(self.executor_ctrl);
        drop(self.pi_ctrl);
        self.estop_send.send(ControlComms::Exit).unwrap();
    }
}

pub fn start(
    settings: Settings,
    error_send: Sender<ControlComms<Error>>,
) -> Result<(JoinHandle<()>, HwCtrl)> {
    HwCtrl::init(settings, error_send)
}
