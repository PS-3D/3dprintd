use crate::{comms::EStop, config::Config};
use anyhow::{ensure, Result};
use crossbeam::channel::Receiver;
use nanotec_stepper_driver::{
    AllMotor, Driver, Ignore, LimitSwitchBehavior, Motor, MotorStatus, PositioningMode,
    Repetitions, RespondMode, ResponseHandle, RotationDirection, SendAutoStatus,
};
use serialport;
use std::{
    sync::Arc,
    thread::{self, JoinHandle},
    time::Duration,
};

fn reference_motor(
    motor: &mut Motor<SendAutoStatus>,
    direction: RotationDirection,
    speed: u32,
    accel: u32,
    jerk: u32,
) -> Result<()> {
    motor
        .set_positioning_mode(PositioningMode::ExternalReference)?
        .wait()
        .ignore()?;
    motor
        .set_limit_switch_behavior(LimitSwitchBehavior::default())?
        .wait()
        .ignore()?;
    motor.set_rotation_direction(direction)?.wait().ignore()?;
    motor.set_min_frequency(1)?.wait().ignore()?;
    motor.set_max_frequency(speed)?.wait().ignore()?;
    motor
        .set_rotation_direction_change(false)?
        .wait()
        .ignore()?;
    motor.set_repetitions(Repetitions::N(1))?.wait().ignore()?;
    motor.set_continuation_record(None)?.wait().ignore()?;
    motor.set_accel_ramp_no_conversion(accel)?.wait().ignore()?;
    motor.set_brake_ramp_no_conversion(accel)?.wait().ignore()?;
    motor.set_max_accel_jerk(jerk)?.wait().ignore()?;
    motor.set_max_brake_jerk(jerk)?.wait().ignore()?;
    let status = motor.start_motor()?.wait().ignore()?.wait().ignore()?;
    ensure!(
        status == MotorStatus::Ready,
        "motor error while referencing, status was {}",
        status
    );
    Ok(())
}

struct Motors {
    all: AllMotor,
    x: Motor<SendAutoStatus>,
    y: Motor<SendAutoStatus>,
    z: Motor<SendAutoStatus>,
    e: Motor<SendAutoStatus>,
}

impl Motors {
    fn reference(&mut self, cfg: &Config) -> Result<()> {
        todo!("still have to implement persistent settings");
        // reference_motor(&mut self.x, cfg.motors.x.endstop_direction)?;
        // reference_motor(&mut self.y, cfg.motors.y.endstop_direction)?;
        // reference_motor(&mut self.z, cfg.motors.z.endstop_direction)?;
        // Ok(())
    }
}

fn init(cfg: &Config, mut driver: Driver) -> Result<Motors> {
    let all = driver.add_all_motor().expect("adding AllMotor failed");
    let x = driver
        .add_motor(cfg.motors.x.address, RespondMode::NotQuiet)
        .expect("adding x axis motor failed");
    let y = driver
        .add_motor(cfg.motors.y.address, RespondMode::NotQuiet)
        .expect("adding y axis motor failed");
    let z = driver
        .add_motor(cfg.motors.z.address, RespondMode::NotQuiet)
        .expect("adding z axis motor failed");
    let e = driver
        .add_motor(cfg.motors.e.address, RespondMode::NotQuiet)
        .expect("adding e axis motor failed");
    let mut x = x.start_sending_auto_status().ignore()?.wait().ignore()?;
    let mut y = y.start_sending_auto_status().ignore()?.wait().ignore()?;
    let mut z = z.start_sending_auto_status().ignore()?.wait().ignore()?;
    let mut e = e.start_sending_auto_status().ignore()?.wait().ignore()?;
    x.set_quickstop_ramp_no_conversion(cfg.motors.x.quickstop_ramp)?
        .wait()
        .ignore()?;
    y.set_quickstop_ramp_no_conversion(cfg.motors.y.quickstop_ramp)?
        .wait()
        .ignore()?;
    z.set_accel_ramp_no_conversion(cfg.motors.z.quickstop_ramp)?
        .wait()
        .ignore()?;
    e.set_accel_ramp_no_conversion(cfg.motors.e.quickstop_ramp)?
        .wait()
        .ignore()?;
    let mut motors = Motors { all, x, y, z, e };
    // FIXME maybe take out because this will move motors while there is no way
    // to actuate the digital estop
    motors.reference(cfg)?;
    Ok(motors)
}

// create serialport & driver
//
// get estop & spawn estop listener thread
//
// init:
//   setup quickstop ramp
//   reference axis
//
// spawn motor thread:
//   loop
//     check for msg on control channel -> exec that
//     check for msg on manual channel -> exec that
//     check for msg on gcode channel -> exec that
//     if none, select for message on channel
//       if on control channel -> exec that
//         if msg = exit -> break
//         if msg = new gcode channel -> replace
//       if on manual channel -> execute
//       if on gcode channel -> execute
pub fn start(cfg: Arc<Config>, estop_channel: Receiver<EStop>) -> Result<JoinHandle<()>> {
    // do onetimesetup first so we can still return and error out if any of that
    // fails
    let iface = serialport::new(cfg.motors.port.as_str(), cfg.motors.baud_rate)
        .timeout(Duration::from_secs(cfg.motors.timeout))
        .open()?;
    let driver = Driver::new(iface)?;
    let mut estop = driver.new_estop();
    let estop_handle = thread::spawn(move || {
        loop {
            match estop_channel
                .recv()
                .expect("estop channel was unexpectedly closed")
            {
                // if there's an IO error writing, it's probably a good plan to
                // panic
                EStop::EStop => estop.estop(2000).unwrap(),
                EStop::Exit => break,
            }
        }
    });
    init(cfg.as_ref(), driver)?;
    let handle = thread::spawn(move || todo!());
    Ok(handle)
}
