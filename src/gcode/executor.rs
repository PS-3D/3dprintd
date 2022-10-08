use std::{thread, time::Duration};

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

    fn exec_motor(&self, msg: MotorControl) -> Result<()> {
        self.motor_send.send(msg).unwrap();
        self.motor_ret_recv.recv().unwrap()
    }

    fn exec_wait(&self, time: Duration) {
        thread::sleep(time);
    }

    pub fn exec(&mut self, action: Action) -> Result<()> {
        match action {
            Action::MoveAll(m) => self.exec_motor(MotorControl::MoveAll(m)),
            Action::ReferenceAll => self.exec_motor(MotorControl::ReferenceAll),
            Action::ReferenceAxis(a) => self.exec_motor(MotorControl::ReferenceAxis(a)),
            Action::HotendTemp(t) => todo!(),
            Action::BedTemp(t) => todo!(),
            // FIXME add timeouts for temp waits, otherwise it might wait forever
            //       or add error checking
            Action::WaitHotendTemp(t) => todo!(),
            Action::WaitBedTemp(t) => todo!(),
            Action::Wait(d) => Ok(self.exec_wait(d)),
        }
    }
}

impl Drop for Executor {
    fn drop(&mut self) {
        self.motor_send.send(MotorControl::Exit).unwrap();
    }
}
