mod executor;

use self::executor::Executor;
use crate::comms::{Action, ControlComms, MotorControl};
use anyhow::Result;
use crossbeam::channel::{Receiver, Sender};
use std::thread::{self, JoinHandle};

fn executor_loop(
    action_recv: Receiver<ControlComms<Action>>,
    motor_send: Sender<MotorControl>,
    motor_ret_recv: Receiver<Result<()>>,
) {
    let mut exec = Executor::new(motor_send, motor_ret_recv);
    loop {
        match action_recv.recv().unwrap() {
            // FIXME do something with result
            ControlComms::Msg(a) => exec.exec(a).unwrap(),
            ControlComms::Exit => break,
        }
    }
}

pub fn start(
    executor_recv: Receiver<ControlComms<Action>>,
    motor_send: Sender<MotorControl>,
    motor_ret_recv: Receiver<Result<()>>,
) -> JoinHandle<()> {
    let executor_handle =
        thread::spawn(move || executor_loop(executor_recv, motor_send, motor_ret_recv));
    executor_handle
}
