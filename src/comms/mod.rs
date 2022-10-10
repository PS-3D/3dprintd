use crossbeam::channel::Receiver;
use gcode::Span as InnerSpan;
use nanotec_stepper_driver::RotationDirection;
use rocket::request::FromParam;
use std::{
    fmt::{self, Display},
    path::PathBuf,
    sync::{atomic::AtomicUsize, Arc},
    time::Duration,
};

#[derive(Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl FromParam<'_> for Axis {
    type Error = &'static str;

    fn from_param(param: &str) -> Result<Self, Self::Error> {
        match param {
            "x" => Ok(Self::X),
            "y" => Ok(Self::Y),
            "z" => Ok(Self::Z),
            _ => Err("not a valid axis, must be X, Y or Z"),
        }
    }
}

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
pub enum ControlComms<T> {
    Msg(T),
    Exit,
}

#[derive(Debug)]
pub enum Action {
    MoveAll(Movement),
    ReferenceAll,
    ReferenceAxis(Axis),
    HotendTemp(Option<u32>),
    BedTemp(Option<u32>),
    WaitHotendTemp(Option<u32>),
    WaitBedTemp(Option<u32>),
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

pub enum ExecutorCtrl {
    GCode(Receiver<(Action, GCodeSpan)>, Arc<AtomicUsize>),
    Manual,
}

pub enum DecoderComms {
    Started,
    Stopped,
    Paused,
    Played,
}

pub enum EStopComms {
    EStop,
}
