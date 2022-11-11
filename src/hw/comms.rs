use crate::comms::OnewayAtomicF64Read;

pub enum EStopComms {
    EStop,
}

#[derive(Debug, Clone)]
pub struct OnewayPosRead {
    pub x: OnewayAtomicF64Read,
    pub y: OnewayAtomicF64Read,
    pub z: OnewayAtomicF64Read,
}
