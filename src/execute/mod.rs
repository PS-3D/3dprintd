mod executor;
mod motors;

use self::{executor::Executor, motors::Motors};
use crate::{
    comms::{Action, ControlComms, EStopComms},
    settings::Settings,
};
use anyhow::Result;
use crossbeam::channel::{self, Receiver};
use nanotec_stepper_driver::Driver;
use std::{
    thread::{self, JoinHandle},
    time::Duration,
};

fn executor_loop(
    settings: Settings,
    motors: Motors,
    executor_recv: Receiver<ControlComms<Action>>,
) {
    let mut exec = Executor::new(settings, motors);
    loop {
        match executor_recv.recv().unwrap() {
            // FIXME do something with result
            ControlComms::Msg(a) => exec.exec(a).unwrap(),
            ControlComms::Exit => break,
        }
    }
}

pub fn start(
    settings: Settings,
    executor_recv: Receiver<ControlComms<Action>>,
    estop_recv: Receiver<ControlComms<EStopComms>>,
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
                .open()?;
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
                executor_loop(settings, motors, executor_recv);
            }
            Err(e) => {
                setup_send.send(Err(e)).unwrap();
            }
        }
    });
    let estop_handle = setup_recv.recv().unwrap()?;
    Ok((executor_handle, estop_handle))
}
