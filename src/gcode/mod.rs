pub(self) mod action;
mod decoder;
pub mod error;
mod executor;

use self::{action::Action, decoder::Decoder, executor::Executor};
use crate::{comms::MotorControl, settings::Settings};
use anyhow::Result;
use crossbeam::channel::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

// NOTE maybe decode gcode into actions, which then can be buffered
// actions could have a format that is easily understood. that way the
// "lengthy" parsing can be sourced out ("parsing" could also then include already
// calculating things etc. so actions only need to be executed)

enum Comms {
    Action(Action),
    Exit,
}

fn decoder_loop(settings: Settings, action_send: Sender<Comms>) {
    let mut decoder = Decoder::new(settings);
    todo!()
}

fn executor_loop(
    action_recv: Receiver<Comms>,
    motor_send: Sender<MotorControl>,
    motor_ret_recv: Receiver<Result<()>>,
) {
    let mut exec = Executor::new(motor_send, motor_ret_recv);
    loop {
        match action_recv.recv().unwrap() {
            // FIXME do something with result
            Comms::Action(a) => exec.exec(a).unwrap(),
            Comms::Exit => break,
        }
    }
}

pub fn start(
    settings: Settings,
    motor_send: Sender<MotorControl>,
    motor_ret_recv: Receiver<Result<()>>,
) -> (JoinHandle<()>, JoinHandle<()>) {
    let (action_send, action_recv) = channel::bounded(16);
    let decoder_handle = thread::spawn(move || decoder_loop(settings, action_send));
    let executor_handle =
        thread::spawn(move || executor_loop(action_recv, motor_send, motor_ret_recv));
    (decoder_handle, executor_handle)
}

// let mut gcode_channel: Option<Receiver<GCode>> = None;

// // put handlers in macro so we can still break the loop from them
// macro_rules! handle_ctrl_msg {
//     ($msg:expr) => {
//         match $msg {
//             MotorControl::StartPrint(gc) => {
//                 gcode_channel.replace(gc);
//             }
//             // FIXME error handling
//             MotorControl::ReferenceAll => motors.reference(&settings).unwrap(),
//             MotorControl::Exit => break,
//         }
//     };
// }

// loop {
//     match motor_control_recv.try_recv() {
//         Ok(msg) => handle_ctrl_msg!(msg),
//         Err(e) => match e {
//             TryRecvError::Empty => (),
//             TryRecvError::Disconnected => {
//                 // FIXME logging and graceful exit
//                 panic!("motor control channel was unexpectedly closed")
//             }
//         },
//     }
//     match manual_gcode_recv.try_recv() {
//         Ok(msg) => todo!(),
//         Err(e) => match e {
//             TryRecvError::Empty => (),
//             TryRecvError::Disconnected => {
//                 // FIXME logging and graceful exit
//                 panic!("motor control channel was unexpectedly closed")
//             }
//         },
//     }
//     if let Some(gc) = gcode_channel.as_ref() {
//         match gc.try_recv() {
//             Ok(msg) => todo!(),
//             Err(e) => match e {
//                 TryRecvError::Empty => (),
//                 TryRecvError::Disconnected => {
//                     // FIXME logging and graceful exit
//                     panic!("motor control channel was unexpectedly closed")
//                 }
//             },
//         }
//     }
//     if let Some(gc) = gcode_channel.as_ref() {
//         select! {
//             recv(motor_control_recv) -> msg => handle_ctrl_msg!(msg.expect("motor control channel was unexpectedly closed")),
//             recv(manual_gcode_recv) -> msg => todo!(),
//             recv(gc) -> msg => todo!(),
//         }
//     } else {
//         select! {
//             recv(motor_control_recv) -> msg => handle_ctrl_msg!(msg.expect("motor control channel was unexpectedly closed")),
//             recv(manual_gcode_recv) -> msg =>  todo!(),
//         }
//     }
// }
