mod executor;
mod motors;

use self::{executor::Executor, motors::Motors};
use crate::{
    comms::{Action, ControlComms, EStopComms, ExecutorCtrl},
    settings::Settings,
    util::send_err,
};
use anyhow::{Context, Error, Result};
use crossbeam::{
    channel::{self, Receiver, Sender},
    select,
};
use nanotec_stepper_driver::Driver;
use std::{
    sync::atomic::Ordering,
    thread::{self, JoinHandle},
    time::Duration,
};

fn executor_loop(
    settings: Settings,
    motors: Motors,
    executor_ctrl_recv: Receiver<ControlComms<ExecutorCtrl>>,
    executor_manual_recv: Receiver<Action>,
    error_send: Sender<ControlComms<Error>>,
) {
    let mut exec = Executor::new(settings, motors);
    let mut gcode = None;
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
        handle_ctrl_msg!(executor_ctrl_recv.recv().unwrap());
        if let Some((gcode_recv, line)) = gcode.as_ref() {
            select! {
            recv(executor_ctrl_recv) -> msg => handle_ctrl_msg!(msg.unwrap()),
            recv(gcode_recv) -> msg => {
                let (action, span) = msg.unwrap();
                line.store(span.inner.line, Ordering::Release);
                // TODO attach span info to error
                send_err!(exec.exec(action), error_send)
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
) -> Result<(JoinHandle<()>, JoinHandle<()>)> {
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
                setup_send.send(Ok(estop_handle)).unwrap();
                executor_loop(
                    settings,
                    motors,
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
    let estop_handle = setup_recv.recv().unwrap()?;
    Ok((executor_handle, estop_handle))
}
