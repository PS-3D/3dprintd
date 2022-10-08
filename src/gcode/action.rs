use crate::comms::{Axis, Movement};
use std::time::Duration;

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
