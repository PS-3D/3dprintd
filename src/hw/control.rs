use super::{
    comms::EStopComms,
    execute::{ExecutorCtrl, OutOfBoundsError},
    pi::PiCtrl,
    state::{State, StateError, StateInfo as InnerStateInfo},
};
use crate::{
    comms::{Axis, ControlComms, ReferenceRunOptParameters},
    settings::Settings,
    util::ensure_own,
};
use anyhow::{ensure, Result};
use crossbeam::channel::Sender;
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
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

#[derive(Debug, Clone)]
pub struct HwCtrl {
    state: Arc<RwLock<State>>,
    settings: Settings,
    executor_ctrl: Arc<ExecutorCtrl>,
    pi_ctrl: PiCtrl,
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
    pub(super) fn new(
        settings: Settings,
        executor_ctrl: ExecutorCtrl,
        pi_ctrl: PiCtrl,
        estop_send: Sender<ControlComms<EStopComms>>,
    ) -> Self {
        Self {
            state: Arc::new(RwLock::new(State::new())),
            settings,
            executor_ctrl: Arc::new(executor_ctrl),
            pi_ctrl,
            estop_send,
        }
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
        self.executor_ctrl.print(path.clone())?;
        let cur_line = state.print(path);
        Ok(())
    }

    pub fn stop(&self) {
        let mut state = self.state.write().unwrap();
        state.stop();
        self.executor_ctrl.stop();
    }

    pub fn try_play(&self) -> Result<(), StateError> {
        let mut state = self.state.write().unwrap();
        ensure_own!(!state.is_stopped(), StateError::Stopped);
        state.play();
        self.executor_ctrl.play();
        Ok(())
    }

    /// Tries to pause a print
    ///
    /// Should only be used by the API thread, not the decoder thread
    pub fn try_pause(&self) -> Result<(), StateError> {
        let mut state = self.state.write().unwrap();
        ensure_own!(!state.is_stopped(), StateError::Stopped);
        state.pause();
        self.executor_ctrl.pause();
        Ok(())
    }

    pub fn estop(&self) {
        self.estop_send
            .send(ControlComms::Msg(EStopComms::EStop))
            .unwrap()
    }

    pub fn exit(self) {
        drop(self.executor_ctrl);
        self.estop_send.send(ControlComms::Exit).unwrap();
        self.pi_ctrl.exit();
    }
}
