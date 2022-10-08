use std::fs::File;

use nanotec_stepper_driver::RotationDirection;

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

pub enum ControlComms<T> {
    Msg(T),
    Exit,
}

pub enum MotorControl {
    MoveAll(Movement),
    ReferenceAll,
    ReferenceAxis(Axis),
    Exit,
}

pub enum DecoderComms {
    Print(File),
    Stop,
    Play,
    Pause,
}

pub enum EStop {
    EStop,
    Exit,
}
