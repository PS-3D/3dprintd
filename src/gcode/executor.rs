use super::action::Action;
use crate::comms::MotorControl;
use anyhow::Result;
use crossbeam::channel::{Receiver, Sender};

pub struct Executor {
    motor_send: Sender<MotorControl>,
    motor_ret_recv: Receiver<Result<()>>,
}

impl Executor {
    pub fn new(motor_send: Sender<MotorControl>, motor_ret_recv: Receiver<Result<()>>) -> Self {
        Self {
            motor_send,
            motor_ret_recv,
        }
    }

    pub fn exec(&mut self, action: Action) -> Result<()> {
        match action {
            Action::MoveAll(m) => todo!(),
            Action::ReferenceAll => todo!(),
            Action::ReferenceAxis(a) => todo!(),
            Action::HotendTemp(t) => todo!(),
            Action::BedTemp(t) => todo!(),
            Action::WaitHotendTemp(t) => todo!(),
            Action::WaitBedTemp(t) => todo!(),
            Action::Wait(d) => todo!(),
        }
    }
}

impl Drop for Executor {
    fn drop(&mut self) {
        self.motor_send.send(MotorControl::Exit).unwrap();
    }
}
