use super::{
    comms::{Action, EStopComms, ExecutorCtrl, OnewayPosRead},
    decode::DecoderCtrl,
    state::{State, StateError, StateInfo},
};
use crate::{
    comms::{Axis, ControlComms},
    util::ensure_own,
};
use anyhow::{ensure, Result};
use crossbeam::channel::{self, Sender};
use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

pub struct PositionInfo {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl From<&OnewayPosRead> for PositionInfo {
    fn from(read: &OnewayPosRead) -> Self {
        Self {
            x: read.x.read(),
            y: read.x.read(),
            z: read.x.read(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HwCtrl {
    state: Arc<RwLock<State>>,
    decoder_ctrl: DecoderCtrl,
    executor_ctrl_send: Sender<ControlComms<ExecutorCtrl>>,
    executor_manual_send: Sender<Action>,
    estop_send: Sender<ControlComms<EStopComms>>,
    oneway_pos_read: OnewayPosRead,
}

macro_rules! pos_info_axis {
    ($func_name:ident, $axis:ident) => {
        pub fn $func_name(&self) -> f64 {
            self.oneway_pos_read.$axis.read()
        }
    };
}

impl HwCtrl {
    pub(super) fn new(
        decoder_ctrl: DecoderCtrl,
        executor_ctrl_send: Sender<ControlComms<ExecutorCtrl>>,
        executor_manual_send: Sender<Action>,
        estop_send: Sender<ControlComms<EStopComms>>,
        oneway_pos_read: OnewayPosRead,
    ) -> Self {
        Self {
            state: Arc::new(RwLock::new(State::new())),
            decoder_ctrl,
            executor_ctrl_send,
            executor_manual_send,
            estop_send,
            oneway_pos_read,
        }
    }

    pub fn state_info(&self) -> StateInfo {
        self.state.read().unwrap().info()
    }

    pub fn pos_info(&self) -> PositionInfo {
        (&self.oneway_pos_read).into()
    }

    pos_info_axis!(pos_info_x, x);
    pos_info_axis!(pos_info_y, y);
    pos_info_axis!(pos_info_z, z);

    pub fn try_reference_axis(&self, axis: Axis) -> Result<(), StateError> {
        let state = self.state.read().unwrap();
        ensure_own!(state.is_stopped(), StateError::NotStopped);
        self.executor_manual_send
            .send(Action::ReferenceAxis(axis))
            .unwrap();
        Ok(())
    }

    /// Tries to start a print
    ///
    /// Should only be used by the API thread, not the decoder thread
    pub fn try_print(&self, path: PathBuf) -> Result<()> {
        let mut state = self.state.write().unwrap();
        ensure!(state.is_stopped(), StateError::NotStopped);
        let (executor_gcode_send, executor_gcode_recv) = channel::bounded(16);
        self.decoder_ctrl.print(path.clone(), executor_gcode_send)?;
        let cur_line = state.print(path);
        self.executor_ctrl_send
            .send(ControlComms::Msg(ExecutorCtrl::GCode(
                executor_gcode_recv,
                Arc::clone(cur_line),
            )))
            .unwrap();
        Ok(())
    }

    pub fn stop(&self) {
        let mut state = self.state.write().unwrap();
        state.stop();
        self.decoder_ctrl.stop();
        self.executor_ctrl_send
            .send(ControlComms::Msg(ExecutorCtrl::Manual))
            .unwrap();
    }

    pub fn try_play(&self) -> Result<(), StateError> {
        let mut state = self.state.write().unwrap();
        ensure_own!(!state.is_stopped(), StateError::Stopped);
        state.play();
        self.decoder_ctrl.play();
        Ok(())
    }

    /// Tries to pause a print
    ///
    /// Should only be used by the API thread, not the decoder thread
    pub fn try_pause(&self) -> Result<(), StateError> {
        let mut state = self.state.write().unwrap();
        ensure_own!(!state.is_stopped(), StateError::Stopped);
        state.pause();
        self.decoder_ctrl.pause();
        Ok(())
    }

    pub fn estop(&self) {
        self.estop_send
            .send(ControlComms::Msg(EStopComms::EStop))
            .unwrap()
    }

    pub fn exit(&self) {
        self.decoder_ctrl.exit();
        self.executor_ctrl_send.send(ControlComms::Exit).unwrap();
        self.estop_send.send(ControlComms::Exit).unwrap();
    }
}
