use crate::comms::{Axis, Movement};
use std::time::Duration;

pub enum Action {
    MoveAll(Movement),
    ReferenceAll,
    ReferenceAxis(Axis),
    HotendTemp(u32),
    BedTemp(u32),
    WaitHotendTemp,
    WaitBedTemp,
    Wait(Duration),
}
