pub mod error;

use self::error::{MotorError, MotorsError};
use crate::{
    comms::{AxisMovement, ExtruderMovement, Movement},
    settings::Settings,
};
use anyhow::{ensure, Result};
use nanotec_stepper_driver::{
    AllMotor, Driver, DriverError, Ignore, LimitSwitchBehavior, Motor, MotorStatus,
    PositioningMode, Repetitions, RespondMode, ResponseHandle, RotationDirection, SendAutoStatus,
};

// TODO maybe store state for all motors as in valid or invalid
// -> set invalid after encountered error,
// -> set invalid at the beginning
//
// => no move allowed when state is invalid, only reference can fix that. if that
//    fails, keep state invalid
pub struct Motors {
    all: AllMotor,
    x: Motor<SendAutoStatus>,
    y: Motor<SendAutoStatus>,
    z: Motor<SendAutoStatus>,
    e: Motor<SendAutoStatus>,
}

macro_rules! make_reference_motor {
    ($name:ident, $axis:ident) => {
        pub fn $name(&mut self, settings: &Settings) -> Result<()> {
            Motors::reference_motor(
                &mut self.$axis,
                settings.config().motors.$axis.endstop_direction,
                settings.motors().$axis().get_reference_speed(),
                settings.motors().$axis().get_reference_accel_decel(),
                settings.motors().$axis().get_reference_jerk(),
            )?;
            Ok(())
        }
    };
}

impl Motors {
    pub fn init(settings: &Settings, mut driver: Driver) -> Result<Self> {
        let cfg = settings.config();
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
        let motors = Motors { all, x, y, z, e };
        // FIXME init positioningmode, turningdirection, etc.
        Ok(motors)
    }

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

    make_reference_motor!(reference_x, x);
    make_reference_motor!(reference_y, y);
    make_reference_motor!(reference_z, z);

    pub fn reference_all(&mut self, settings: &Settings) -> Result<()> {
        self.reference_x(settings)?;
        self.reference_y(settings)?;
        self.reference_z(settings)?;
        Ok(())
    }

    // will only return a DriverError or MotorsError
    pub fn move_all(&mut self, m: &Movement) -> Result<()> {
        // set all quiet so setting of values goes faster
        // we can unwrap here and all following until we set respondmode to notquiet
        // again because there won't be any response anyways so there can't be
        // an error
        self.all
            .set_respond_mode(RespondMode::Quiet)?
            .wait()
            .unwrap();

        fn prepare_move_axis(
            motor: &mut Motor<SendAutoStatus>,
            am: &AxisMovement,
        ) -> Result<(), DriverError> {
            motor.set_travel_distance(am.distance)?.wait().unwrap();
            // if distance is set to 0, ignore setting the other values, it means
            // the motor won't move anyways
            if am.distance != 0 {
                motor.set_min_frequency(am.min_frequency)?.wait().unwrap();
                motor.set_max_frequency(am.max_frequency)?.wait().unwrap();
                motor
                    .set_accel_ramp_no_conversion(am.acceleration)?
                    .wait()
                    .unwrap();
                motor
                    .set_brake_ramp_no_conversion(am.deceleration)?
                    .wait()
                    .unwrap();
                motor
                    .set_max_accel_jerk(am.acceleration_jerk)?
                    .wait()
                    .unwrap();
                motor
                    .set_max_brake_jerk(am.deceleration_jerk)?
                    .wait()
                    .unwrap();
            }
            Ok(())
        }
        fn prepare_move_extruder(
            motor: &mut Motor<SendAutoStatus>,
            em: &ExtruderMovement,
        ) -> Result<(), DriverError> {
            motor
                .set_travel_distance(em.distance as i32)?
                .wait()
                .unwrap();
            // if distance is set to 0, ignore setting the other values, it means
            // the motor won't move anyways
            if em.distance != 0 {
                motor.set_rotation_direction(em.direction)?.wait().unwrap();
                motor.set_min_frequency(em.min_frequency)?.wait().unwrap();
                motor.set_max_frequency(em.max_frequency)?.wait().unwrap();
                motor
                    .set_accel_ramp_no_conversion(em.acceleration)?
                    .wait()
                    .unwrap();
                motor
                    .set_brake_ramp_no_conversion(em.deceleration)?
                    .wait()
                    .unwrap();
                motor
                    .set_max_accel_jerk(em.acceleration_jerk)?
                    .wait()
                    .unwrap();
                motor
                    .set_max_brake_jerk(em.deceleration_jerk)?
                    .wait()
                    .unwrap();
            }
            Ok(())
        }

        prepare_move_axis(&mut self.x, &m.x)?;
        prepare_move_axis(&mut self.y, &m.y)?;
        prepare_move_axis(&mut self.z, &m.z)?;
        prepare_move_extruder(&mut self.e, &m.e)?;

        // set respondmode to notquiet so we will receive the status once
        // the motors are finished
        self.all
            .set_respond_mode(RespondMode::NotQuiet)?
            .wait()
            .ignore()?;
        let errs: Vec<_> = self
            .all
            .start_motor()?
            .wait()
            .ignore()?
            .into_iter()
            .map(|t| (t.0, t.1.wait().ignore()))
            // map PositionErrors in status and DriverErrors to MotorError
            .map(|t| {
                let res = match t.1 {
                    Ok(s) => match s {
                        MotorStatus::PosError => Err(MotorError::PositionError),
                        _ => Ok(()),
                    },
                    Err(e) => Err(MotorError::from(e)),
                };
                (t.0, res)
            })
            .filter(|t| t.1.is_err())
            .collect();
        if errs.is_empty() {
            Ok(())
        } else {
            // invariant of MotorsError will be fulfilled because errs isn't empty
            let mut me = MotorsError {
                x: None,
                y: None,
                z: None,
                e: None,
            };
            // we can unwrap because we already know that these are the errors
            for (addr, err) in errs.into_iter().map(|t| (t.0, t.1.unwrap_err())) {
                // since the returnvalue of all.start_motors is a map of
                // address -> Result we need to map the address back to the actual
                // motor again
                match addr {
                    x if x == self.x.address() => me.x = Some(err),
                    y if y == self.y.address() => me.y = Some(err),
                    z if z == self.z.address() => me.z = Some(err),
                    e if e == self.e.address() => me.e = Some(err),
                    _ => unreachable!("Received error from address that doesn't exist in the driver, it should have thrown an error already")
                }
            }
            Err(me.into())
        }
    }
}
