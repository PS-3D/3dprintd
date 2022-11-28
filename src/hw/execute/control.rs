use super::{super::callbacks::StopCallback, ExecutorCtrlComms, ExecutorManualComms, SharedRawPos};
use crate::{
    comms::{Axis, ControlComms, ReferenceRunOptParameters},
    settings::Settings,
    util::ensure_own,
};
use anyhow::{Context, Result};
use atomic_float::AtomicF64;
use crossbeam::channel::Sender;
use std::{
    fs::File,
    mem::ManuallyDrop,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread::JoinHandle,
};
use thiserror::Error;

#[derive(Debug, Error)]
#[error("{} was out of bounds, was {}, must be <= {}", .0, .1, .2)]
pub struct OutOfBoundsError(&'static str, u32, u32);

// not implementing clone since that could lead to the executor thread being
// stopped twice due to implementing drop. though this makes intuitive sense
// anyways, one executor thread, one control for it
#[derive(Debug)]
pub struct ExecutorCtrl {
    settings: Settings,
    executor_handle: ManuallyDrop<JoinHandle<()>>,
    executor_ctrl_send: Sender<ControlComms<ExecutorCtrlComms>>,
    executor_manual_send: Sender<ExecutorManualComms>,
    line: Arc<AtomicUsize>,
    shared_pos: SharedRawPos,
    // location of the hotend on the z axis, assuming zero point is at endstop
    // shared with the executor thread, only to calculate the z position properly
    shared_z_hotend_location: Arc<AtomicF64>,
}

impl ExecutorCtrl {
    pub(super) fn new(
        settings: Settings,
        executor_handle: JoinHandle<()>,
        executor_ctrl_send: Sender<ControlComms<ExecutorCtrlComms>>,
        executor_manual_send: Sender<ExecutorManualComms>,
        shared_pos: SharedRawPos,
        shared_z_hotend_location: Arc<AtomicF64>,
    ) -> Self {
        Self {
            settings,
            executor_handle: ManuallyDrop::new(executor_handle),
            executor_ctrl_send,
            executor_manual_send,
            line: Arc::new(AtomicUsize::new(0)),
            shared_pos,
            shared_z_hotend_location,
        }
    }

    fn send_executor_ctrl(&self, msg: ExecutorCtrlComms) {
        self.executor_ctrl_send
            .send(ControlComms::Msg(msg))
            .unwrap();
    }

    // end_callback could in theory also be a generic but that would be
    // 1. a pain in the ass to properly pass around that generic
    // 2. since the callback is a new one every time, we could also allow
    //    the actual type to be different every time
    pub fn print(&self, path: PathBuf, end_callback: Box<dyn StopCallback>) -> Result<()> {
        let file = File::open(&path).context("failed to open gcode file")?;
        self.send_executor_ctrl(ExecutorCtrlComms::Print(
            file,
            path,
            Arc::clone(&self.line),
            end_callback,
        ));
        Ok(())
    }

    pub fn stop(&self) {
        self.send_executor_ctrl(ExecutorCtrlComms::Stop)
    }

    pub fn play(&self) {
        self.send_executor_ctrl(ExecutorCtrlComms::Play)
    }

    pub fn pause(&self) {
        self.send_executor_ctrl(ExecutorCtrlComms::Pause)
    }

    pub fn current_line(&self) -> usize {
        self.line.load(Ordering::Acquire)
    }

    pub fn pos_x(&self) -> f64 {
        self.settings
            .config()
            .motors
            .x
            .steps_to_mm(self.shared_pos.x.load(Ordering::Acquire))
    }

    pub fn pos_y(&self) -> f64 {
        self.settings
            .config()
            .motors
            .y
            .steps_to_mm(self.shared_pos.y.load(Ordering::Acquire))
    }

    pub fn pos_z(&self) -> f64 {
        self.settings
            .config()
            .motors
            .z
            .steps_to_mm(self.shared_pos.z.load(Ordering::Acquire))
            // must be subtraction because z_hotend_location is already negative
            - self.shared_z_hotend_location.load(Ordering::Acquire)
    }

    pub fn reference_axis(
        &self,
        axis: Axis,
        parameters: ReferenceRunOptParameters,
    ) -> Result<(), OutOfBoundsError> {
        let cfg = self.settings.config().motors.axis(&axis);
        if let Some(speed) = parameters.speed.as_ref() {
            ensure_own!(
                *speed <= cfg.speed_limit,
                OutOfBoundsError("speed", *speed, cfg.speed_limit)
            );
        }
        if let Some(accel_decel) = parameters.accel_decel.as_ref() {
            ensure_own!(
                *accel_decel <= cfg.accel_limit,
                OutOfBoundsError("accel_decel", *accel_decel, cfg.accel_limit)
            );
            ensure_own!(
                *accel_decel <= cfg.decel_limit,
                OutOfBoundsError("accel_decel", *accel_decel, cfg.decel_limit)
            );
        }
        if let Some(jerk) = parameters.jerk.as_ref() {
            ensure_own!(
                *jerk <= cfg.accel_jerk_limit,
                OutOfBoundsError("accel_decel", *jerk, cfg.accel_jerk_limit)
            );
            ensure_own!(
                *jerk <= cfg.decel_limit,
                OutOfBoundsError("accel_decel", *jerk, cfg.decel_jerk_limit)
            );
        }
        self.executor_manual_send
            .send(ExecutorManualComms::ReferenceAxis(axis, parameters))
            .unwrap();
        Ok(())
    }

    pub fn reference_z_hotend(&self) {
        self.executor_manual_send
            .send(ExecutorManualComms::ReferenceZAxisHotend)
            .unwrap()
    }
}

impl Drop for ExecutorCtrl {
    fn drop(&mut self) {
        self.executor_ctrl_send.send(ControlComms::Exit).unwrap();
        // safety:
        // since we are in drop, self.executor_handle will not be used again
        unsafe { ManuallyDrop::take(&mut self.executor_handle) }
            .join()
            .unwrap();
    }
}
