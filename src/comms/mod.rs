use crossbeam::channel::Receiver;
use gcode::Span as InnerSpan;
use nanotec_stepper_driver::RotationDirection;
use rocket::request::FromParam;
use std::{
    fmt::{self, Display},
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
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
            _ => Err("not a valid axis, must be x, y, or z"),
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
    ReferenceZHotend,
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

pub type ExecutorGCodeComms = ControlComms<(Action, GCodeSpan)>;

pub enum ExecutorCtrl {
    GCode(Receiver<ExecutorGCodeComms>, Arc<AtomicUsize>),
    Manual,
}

#[derive(Debug, Clone)]
pub struct OnewayAtomicF64Read(Arc<AtomicU64>);

impl OnewayAtomicF64Read {
    pub fn new(val: f64) -> Self {
        Self(Arc::new(AtomicU64::new(u64::from_ne_bytes(
            val.to_ne_bytes(),
        ))))
    }

    pub fn get_write(&self) -> OnewayAtomicF64Write {
        OnewayAtomicF64Write(Arc::clone(&self.0))
    }

    pub fn read(&self) -> f64 {
        // FIXME maybe use Ordering::Relaxed since it doesn't really matter?
        f64::from_ne_bytes(self.0.load(Ordering::Acquire).to_ne_bytes())
    }
}

#[derive(Debug, Clone)]
pub struct OnewayAtomicF64Write(Arc<AtomicU64>);

impl OnewayAtomicF64Write {
    pub fn new(val: f64) -> Self {
        Self(Arc::new(AtomicU64::new(u64::from_ne_bytes(
            val.to_ne_bytes(),
        ))))
    }

    pub fn get_read(&self) -> OnewayAtomicF64Read {
        OnewayAtomicF64Read(Arc::clone(&self.0))
    }

    pub fn write(&self, val: f64) {
        // FIXME maybe use Ordering::Relaxed since it doesn't really matter?
        self.0
            .store(u64::from_ne_bytes(val.to_ne_bytes()), Ordering::Release)
    }
}

pub struct OnewayDataRead {
    pub pos_x: OnewayAtomicF64Read,
    pub pos_y: OnewayAtomicF64Read,
    pub pos_z: OnewayAtomicF64Read,
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
