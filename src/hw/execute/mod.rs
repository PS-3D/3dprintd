mod executor;
mod motors;

use self::{executor::Executor, motors::Motors};
use super::comms::{Action, EStopComms, ExecutorCtrl};
use crate::{
    comms::{ControlComms, OnewayAtomicF64Read},
    hw::comms::OnewayPosRead,
    settings::Settings,
    util::send_err,
};
use anyhow::{Context, Error, Result};
use crossbeam::{
    channel::{self, Receiver, Sender, TryRecvError},
    select,
};
use nanotec_stepper_driver::Driver;
use std::{
    sync::atomic::Ordering,
    thread::{self, JoinHandle},
    time::Duration,
};

fn executor_loop(
    mut exec: Executor,
    executor_ctrl_recv: Receiver<ControlComms<ExecutorCtrl>>,
    executor_manual_recv: Receiver<Action>,
    error_send: Sender<ControlComms<Error>>,
) {
    let mut gcode = None;
    // has to be macro so break will work
    macro_rules! handle_ctrl_msg {
        ($msg:expr) => {{
            match $msg {
                ControlComms::Msg(c) => match c {
                    ExecutorCtrl::GCode(gcode_recv, line) => gcode = Some((gcode_recv, line)),
                    ExecutorCtrl::Manual => gcode = None,
                },
                ControlComms::Exit => break,
            };
        }};
    }
    loop {
        // try to receive a message from the controlchannel, since it has priority
        match executor_ctrl_recv.try_recv() {
            Ok(msg) => {
                handle_ctrl_msg!(msg);
                // in case there is another control message, we want to receive
                // it
                continue;
            }
            Err(e) => match e {
                TryRecvError::Empty => (),
                TryRecvError::Disconnected => {
                    panic!("executor_ctrl_recv unexpectedly disconnected")
                }
            },
        }
        if let Some((gcode_recv, line)) = gcode.as_ref() {
            select! {
                recv(executor_ctrl_recv) -> msg => handle_ctrl_msg!(msg.unwrap()),
                recv(gcode_recv) -> msg => {
                    // we need the Exit here as well, because that means
                    // that the gcode finished. We can't use exec_ctrl for this
                    // because we might get the message before we executed
                    // all messages from the gcode buffer
                    match msg.unwrap() {
                        ControlComms::Msg((action, span)) => {
                            // FIXME maybe use Ordering::Relaxed since it doesn't really matter?
                            line.store(span.inner.line, Ordering::Release);
                            // TODO attach span info to error
                            send_err!(exec.exec(action), error_send)
                        },
                        ControlComms::Exit => gcode = None,
                    }
                },
            }
        } else {
            select! {
                recv(executor_ctrl_recv) -> msg => handle_ctrl_msg!(msg.unwrap()),
                recv(executor_manual_recv) -> msg => send_err!(exec.exec(msg.unwrap()), error_send)
            }
        }
    }
}

pub fn start(
    settings: Settings,
    executor_ctrl_recv: Receiver<ControlComms<ExecutorCtrl>>,
    executor_manual_recv: Receiver<Action>,
    estop_recv: Receiver<ControlComms<EStopComms>>,
    error_send: Sender<ControlComms<Error>>,
) -> Result<(
    JoinHandle<()>,
    JoinHandle<()>,
    OnewayPosRead,
    OnewayAtomicF64Read,
)> {
    let (setup_send, setup_recv) = channel::bounded(1);
    // do it this way all in the executorhread because we can't send motors between
    // threads. We then send the result of the setup via the above channel.
    // the setup is all in a function so we can use the ? operator for convenience
    let executor_handle = thread::spawn(move || {
        fn setup(
            settings: &Settings,
            estop_recv: Receiver<ControlComms<EStopComms>>,
        ) -> Result<(Motors, JoinHandle<()>)> {
            let cfg = settings.config();
            let iface = serialport::new(cfg.motors.port.as_str(), cfg.motors.baud_rate)
                .timeout(Duration::from_secs(cfg.motors.timeout))
                .open()
                .context("Serialport to the motors couldn't be opened")?;
            let driver = Driver::new(iface)?;
            let mut estop = driver.new_estop();
            let estop_handle = thread::spawn(move || {
                loop {
                    match estop_recv
                        .recv()
                        .expect("estop channel was unexpectedly closed")
                    {
                        // if there's an IO error writing, it's probably a good plan to
                        // panic
                        ControlComms::Msg(m) => match m {
                            EStopComms::EStop => estop.estop(2000).unwrap(),
                        },
                        ControlComms::Exit => break,
                    }
                }
            });
            let motors = Motors::init(&settings, driver)?;
            Ok((motors, estop_handle))
        }
        match setup(&settings, estop_recv) {
            Ok((motors, estop_handle)) => {
                let oneway_pos_read = OnewayPosRead {
                    x: motors.x_pos_mm_read(),
                    y: motors.y_pos_mm_read(),
                    z: motors.z_pos_mm_read(),
                };
                let (executor, z_hotend_location) = Executor::new(settings, motors);
                setup_send
                    .send(Ok((estop_handle, oneway_pos_read, z_hotend_location)))
                    .unwrap();
                executor_loop(
                    executor,
                    executor_ctrl_recv,
                    executor_manual_recv,
                    error_send,
                );
            }
            Err(e) => {
                setup_send.send(Err(e)).unwrap();
            }
        }
    });
    let (estop_handle, oneway_data_read, z_hotend_location) = setup_recv.recv().unwrap()?;
    Ok((
        executor_handle,
        estop_handle,
        oneway_data_read,
        z_hotend_location,
    ))
}
