pub mod error;
mod file_decoder;
mod inner_decoder;
mod parser;
mod threaded_decoder;

pub use self::{
    file_decoder::FileDecoder,
    inner_decoder::State,
    parser::{GCode, GCodeSpan, ParserError, ParsingError},
    threaded_decoder::ThreadedDecoder,
};
use super::GCodeError;
use crate::comms::{Axis, ReferenceRunOptParameters};
use anyhow::Result;
use nanotec_stepper_driver::RotationDirection;
use std::time::Duration;
use thiserror::Error;

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
    MoveAxis(Axis, AxisMovement),
    // only allows referencing the z axis into the endstop direction
    // referencing it into the hotend direction can only be done manually
    ReferenceAxis(Axis, ReferenceRunOptParameters),
    HotendTarget(Option<u16>),
    BedTarget(Option<u16>),
    WaitHotendTarget,
    WaitBedTarget,
    WaitBedMinTemp(Option<u16>),
    Wait(Duration),
}

pub trait Decoder: Iterator<Item = Result<(Action, GCode), DecoderError>> {
    fn state(self) -> State;
}

#[derive(Debug, Error)]
pub enum DecoderError {
    #[error("Error while parsing: {}", .0)]
    ParserError(#[from] ParserError),
    #[error("Error while decoding: {}", .0)]
    GCodeError(#[from] GCodeError),
}
