use crate::comms::{Axis, ControlComms, OnewayAtomicF64Read, ReferenceRunOptParameters};
use crossbeam::channel::{Receiver, Sender};
use gcode::Span as InnerSpan;
use nanotec_stepper_driver::RotationDirection;
use std::{
    fmt::{self, Display},
    path::PathBuf,
    sync::{atomic::AtomicUsize, Arc},
    time::Duration,
};

#[derive(Debug, Default)]
pub struct AxisMovement {
    pub distance: i32,
    pub min_frequency: u32,
    pub max_frequency: u32,
    // accel and decel are in hz/s
    pub acceleration: u32,
    pub deceleration: u32,
    pub acceleration_jerk: u32,
    pub deceleration_jerk: u32,
}

#[derive(Debug, Default)]
pub struct ExtruderMovement {
    pub direction: RotationDirection,
    pub distance: u32,
    pub min_frequency: u32,
    pub max_frequency: u32,
    // accel and decel are in hz/s
    pub acceleration: u32,
    pub deceleration: u32,
    pub acceleration_jerk: u32,
    pub deceleration_jerk: u32,
}

// set distance to 0 if that axis shouldn't move
// anything else can be set to random values
#[derive(Debug)]
pub struct Movement {
    pub x: AxisMovement,
    pub y: AxisMovement,
    pub z: AxisMovement,
    pub e: ExtruderMovement,
}

#[derive(Debug)]
pub enum Action {
    MoveAll(Movement),
    ReferenceAxis(Axis, ReferenceRunOptParameters),
    ReferenceZHotend,
    HotendTarget(Option<u16>),
    BedTarget(Option<u16>),
    WaitHotendTarget,
    WaitBedTarget,
    WaitBedMinTemp(Option<u16>),
    Wait(Duration),
}

#[derive(Debug)]
pub struct GCodeSpan {
    pub path: PathBuf,
    pub inner: InnerSpan,
}

impl Display for GCodeSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} , line {}", self.path.display(), self.inner.line + 1)
    }
}

pub type ExecutorGCodeComms = ControlComms<(Action, GCodeSpan)>;

pub enum ExecutorCtrl {
    GCode(Receiver<ExecutorGCodeComms>, Arc<AtomicUsize>),
    Manual,
}

pub enum DecoderComms {
    Started(Sender<ExecutorGCodeComms>),
    Stopped,
    Paused,
    Played,
}

pub enum EStopComms {
    EStop,
}

#[derive(Debug, Clone)]
pub struct OnewayPosRead {
    pub x: OnewayAtomicF64Read,
    pub y: OnewayAtomicF64Read,
    pub z: OnewayAtomicF64Read,
}
