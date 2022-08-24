use crossbeam::channel::Receiver;

pub enum MotorControl {
    StartPrint(Receiver<GCode>),
    Exit,
}

pub enum ManualGCode {}

pub enum GCode {}

pub enum EStop {
    EStop,
    Exit,
}
