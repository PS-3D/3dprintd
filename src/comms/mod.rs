use nanotec_stepper_driver::RotationDirection;
use std::{fs::File, time::Duration};

#[derive(Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
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

pub enum DecoderComms {
    Print(File),
    Stop,
    Play,
    Pause,
}

pub enum EStopComms {
    EStop,
}
