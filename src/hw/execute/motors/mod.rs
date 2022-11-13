// makes everything a bit more clear instead of annotating the inputs
// isn't really that big of a deal since this flag should only be used
// in development anyways
#![cfg_attr(feature = "dev_no_motors", allow(unused_imports, unused_macros))]

pub mod error;

use self::error::{MotorError, MotorsError};
use super::{
    super::decode::{AxisMovement, ExtruderMovement, Movement},
    SharedRawPos,
};
use crate::{
    comms::ReferenceRunOptParameters,
    config::{AxisMotor as AxisMotorConfig, Config, ExtruderMotor as ExtruderMotorConfig},
    settings::Settings,
};
use anyhow::{ensure, Context, Result};
// we want to mask the EStop struct for the dev_no_motors build since otherwise
// that would make the build fail
#[cfg(not(feature = "dev_no_motors"))]
use nanotec_stepper_driver::EStop;
use nanotec_stepper_driver::{
    AllMotor, Driver, DriverError, ErrorCorrectionMode, Ignore, LimitSwitchBehavior,
    LimitSwitchBehaviorNormal, LimitSwitchBehaviorReference, Motor, MotorStatus, PositioningMode,
    RampType, Repetitions, RespondMode, ResponseHandle, SendAutoStatus,
};
use std::{
    sync::{
        atomic::{AtomicI32, Ordering},
        Arc,
    },
    time::Duration,
};

#[cfg(not(feature = "dev_no_motors"))]
struct AxisMotorWrap {
    motor: Motor<SendAutoStatus>,
    pos_steps: Arc<AtomicI32>,
}

// TODO maybe store state for all motors as in valid or invalid
// -> set invalid after encountered error,
// -> set invalid at the beginning
//
// => no move allowed when state is invalid, only reference can fix that. if that
//    fails, keep state invalid
#[cfg(not(feature = "dev_no_motors"))]
pub struct Motors {
    settings: Settings,
    all: AllMotor,
    x: AxisMotorWrap,
    y: AxisMotorWrap,
    z: AxisMotorWrap,
    e: Motor<SendAutoStatus>,
}

fn prepare_move_axis(
    motor: &mut Motor<SendAutoStatus>,
    am: &AxisMovement,
) -> Result<(), DriverError> {
    motor.set_travel_distance(am.distance)?.wait().unwrap();
    // if distance is set to 0, ignore setting the other values, it means
    // the motor won't move anyways
    if am.distance != 0 {
        // don't set min frequency, since that is alwyas the same and we already
        // set it
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

macro_rules! make_reference_motor {
    ($name:ident, $axis:ident) => {
        pub fn $name(
            &mut self,
            settings: &Settings,
            params: ReferenceRunOptParameters,
        ) -> Result<()> {
            Motors::reference_motor(
                &mut self.$axis.motor,
                params
                    .speed
                    .unwrap_or(settings.motors().$axis().get_reference_speed()),
                params
                    .accel_decel
                    .unwrap_or(settings.motors().$axis().get_reference_accel_decel()),
                params
                    .jerk
                    .unwrap_or(settings.motors().$axis().get_reference_jerk()),
            )?;
            self.$axis.pos_steps.store(0, Ordering::Release);
            Ok(())
        }
    };
}

macro_rules! make_move_motor {
    ($name:ident, $axis:ident) => {
        pub fn $name(&mut self, m: &AxisMovement) -> Result<(), MotorError> {
            self.$axis
                .motor
                .set_respond_mode(RespondMode::Quiet)?
                .wait()
                .unwrap();
            prepare_move_axis(&mut self.$axis.motor, m)?;
            // set respondmode to notquiet so we will receive the status once
            // the motor is finished
            self.$axis
                .motor
                .set_respond_mode(RespondMode::NotQuiet)?
                .wait()
                .ignore()?;
            let status = self
                .$axis
                .motor
                .start_motor()?
                .wait()
                .ignore()?
                .wait()
                .ignore()?;
            match status {
                MotorStatus::PosError => Err(MotorError::PositionError),
                _ => Ok(()),
            }
        }
    };
}

#[cfg(not(feature = "dev_no_motors"))]
impl Motors {
    pub(super) fn new(settings: Settings, shared_pos: SharedRawPos) -> Result<(Self, EStop)> {
        let cfg = settings.config();
        let iface = serialport::new(cfg.motors.port.as_str(), cfg.motors.baud_rate)
            .timeout(Duration::from_secs(cfg.motors.timeout))
            .open()
            .context("Serialport to the motors couldn't be opened")?;
        let mut driver = Driver::new(iface)?;
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
        let x = x.start_sending_auto_status().ignore()?.wait().ignore()?;
        let y = y.start_sending_auto_status().ignore()?.wait().ignore()?;
        let z = z.start_sending_auto_status().ignore()?.wait().ignore()?;
        let e = e.start_sending_auto_status().ignore()?.wait().ignore()?;
        let estop = driver.new_estop();
        let motors = Self {
            settings,
            all,
            x: AxisMotorWrap {
                motor: x,
                pos_steps: shared_pos.x,
            },
            y: AxisMotorWrap {
                motor: y,
                pos_steps: shared_pos.y,
            },
            z: AxisMotorWrap {
                motor: z,
                pos_steps: shared_pos.z,
            },
            e,
        };
        Ok((motors, estop))
    }

    pub fn init(&mut self) -> Result<()> {
        fn init_axis(motor: &mut Motor<SendAutoStatus>, config: &AxisMotorConfig) -> Result<()> {
            motor.set_step_mode(config.step_size)?.wait().ignore()?;
            // Maybe change normal limit switch behavior to all stop or all ignore?
            motor
                .set_limit_switch_behavior(LimitSwitchBehavior {
                    internal_reference: LimitSwitchBehaviorReference::FreeTravelBackwards,
                    internal_normal: LimitSwitchBehaviorNormal::Ignore,
                    external_reference: LimitSwitchBehaviorReference::FreeTravelBackwards,
                    external_normal: LimitSwitchBehaviorNormal::Stop,
                })?
                .wait()
                .ignore()?;
            motor
                .set_error_correction_mode(ErrorCorrectionMode::Off)?
                .wait()
                .ignore()?;
            // TODO maybe set swing out time?
            // TODO maybe set max encoder deviation?
            // TODO maybe change ramptype and see how it performs
            motor
                .set_ramp_type(RampType::Trapezoidal)?
                .wait()
                .ignore()?;
            // TODO maybe set waiting time to switch off brake voltage?
            // TODO maybe set waiting time for motor movement?
            // TODO maybe set waiting time for switching off motor current?
            motor
                .set_quickstop_ramp_no_conversion(config.quickstop_ramp)?
                .wait()
                .ignore()?;
            motor
                .set_positioning_mode(PositioningMode::Absolute)?
                .wait()
                .ignore()?;
            // set min frequency here so we dont have to set it later, which
            // saves commands to send to the motor
            motor.set_min_frequency(1)?.wait().ignore()?;
            motor
                .set_rotation_direction(config.endstop_direction)?
                .wait()
                .ignore()?;
            motor
                .set_rotation_direction_change(false)?
                .wait()
                .ignore()?;
            motor.set_repetitions(Repetitions::N(1))?.wait().ignore()?;
            motor.set_continuation_record(None)?.wait().ignore()?;
            Ok(())
        }
        fn init_extruder(
            motor: &mut Motor<SendAutoStatus>,
            config: &ExtruderMotorConfig,
        ) -> Result<()> {
            motor.set_step_mode(config.step_size)?.wait().ignore()?;
            // Maybe change normal limit switch behavior to all stop or all ignore?
            motor
                .set_limit_switch_behavior(LimitSwitchBehavior {
                    internal_reference: LimitSwitchBehaviorReference::FreeTravelBackwards,
                    internal_normal: LimitSwitchBehaviorNormal::Ignore,
                    external_reference: LimitSwitchBehaviorReference::FreeTravelBackwards,
                    external_normal: LimitSwitchBehaviorNormal::Stop,
                })?
                .wait()
                .ignore()?;
            motor
                .set_error_correction_mode(ErrorCorrectionMode::Off)?
                .wait()
                .ignore()?;
            // TODO maybe set swing out time?
            // TODO maybe set max encoder deviation?
            // TODO maybe change ramptype and see how it performs
            motor
                .set_ramp_type(RampType::Trapezoidal)?
                .wait()
                .ignore()?;
            // TODO maybe set waiting time to switch off brake voltage?
            // TODO maybe set waiting time for motor movement?
            // TODO maybe set waiting time for switching off motor current?
            motor
                .set_quickstop_ramp_no_conversion(config.quickstop_ramp)?
                .wait()
                .ignore()?;
            motor
                .set_positioning_mode(PositioningMode::Absolute)?
                .wait()
                .ignore()?;
            motor.set_min_frequency(1)?.wait().ignore()?;
            motor
                .set_rotation_direction_change(false)?
                .wait()
                .ignore()?;
            motor.set_repetitions(Repetitions::N(1))?.wait().ignore()?;
            motor.set_continuation_record(None)?.wait().ignore()?;
            Ok(())
        }
        let cfg = &self.settings.config().motors;
        init_axis(&mut self.x.motor, &cfg.x)?;
        init_axis(&mut self.y.motor, &cfg.y)?;
        init_axis(&mut self.z.motor, &cfg.z)?;
        init_extruder(&mut self.e, &cfg.e)?;
        Ok(())
    }

    fn reference_motor(
        motor: &mut Motor<SendAutoStatus>,
        speed: u32,
        accel: u32,
        jerk: u32,
    ) -> Result<()> {
        motor
            .set_positioning_mode(PositioningMode::ExternalReference)?
            .wait()
            .ignore()?;
        // don't set min frequency, since that is alwyas the same and we already
        // set it
        motor.set_max_frequency(speed)?.wait().ignore()?;
        motor.set_accel_ramp_no_conversion(accel)?.wait().ignore()?;
        motor.set_brake_ramp_no_conversion(accel)?.wait().ignore()?;
        motor.set_max_accel_jerk(jerk)?.wait().ignore()?;
        motor.set_max_brake_jerk(jerk)?.wait().ignore()?;
        let status = motor.start_motor()?.wait().ignore()?.wait().ignore()?;
        // reset values to what they were before, see also init_motor
        motor
            .set_positioning_mode(PositioningMode::Absolute)?
            .wait()
            .ignore()?;
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

    fn update_xzy(&self, m: &Movement) {
        macro_rules! update_axis {
            ($axis:ident) => {{
                self.$axis
                    .pos_steps
                    .store(m.$axis.distance, Ordering::Release)
            }};
        }
        update_axis!(x);
        update_axis!(y);
        update_axis!(z);
    }

    // will only return a DriverError or MotorsError
    pub fn move_all(&mut self, m: &Movement, config: &Config) -> Result<()> {
        // set all quiet so setting of values goes faster
        // we can unwrap here and all following until we set respondmode to notquiet
        // again because there won't be any response anyways so there can't be
        // an error
        self.all
            .set_respond_mode(RespondMode::Quiet)?
            .wait()
            .unwrap();

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
                // don't set min frequency, since that is alwyas the same and we already
                // set it
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

        prepare_move_axis(&mut self.x.motor, &m.x)?;
        prepare_move_axis(&mut self.y.motor, &m.y)?;
        prepare_move_axis(&mut self.z.motor, &m.z)?;
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
            self.update_xzy(m);
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
                    x if x == self.x.motor.address() => me.x = Some(err),
                    y if y == self.y.motor.address() => me.y = Some(err),
                    z if z == self.z.motor.address() => me.z = Some(err),
                    e if e == self.e.address() => me.e = Some(err),
                    _ => unreachable!("Received error from address that doesn't exist in the driver, it should have thrown an error already")
                }
            }
            Err(me.into())
        }
    }

    make_move_motor!(move_x, x);
    make_move_motor!(move_y, y);
    make_move_motor!(move_z, z);
}

#[cfg(feature = "dev_no_motors")]
pub struct EStop {}

#[cfg(feature = "dev_no_motors")]
impl EStop {
    pub fn estop(&mut self, _millis: u64) -> Result<()> {
        Ok(())
    }
}

#[cfg(feature = "dev_no_motors")]
pub struct Motors {
    _shared_pos: SharedRawPos,
}

#[cfg(feature = "dev_no_motors")]
impl Motors {
    pub(super) fn new(_settings: Settings, shared_pos: SharedRawPos) -> Result<(Self, EStop)> {
        Ok((
            Self {
                _shared_pos: shared_pos,
            },
            EStop {},
        ))
    }

    pub fn init(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn reference_x(
        &mut self,
        _settings: &Settings,
        _params: ReferenceRunOptParameters,
    ) -> Result<()> {
        Ok(())
    }

    pub fn reference_y(
        &mut self,
        _settings: &Settings,
        _params: ReferenceRunOptParameters,
    ) -> Result<()> {
        Ok(())
    }

    pub fn reference_z(
        &mut self,
        _settings: &Settings,
        _params: ReferenceRunOptParameters,
    ) -> Result<()> {
        Ok(())
    }

    pub fn move_all(&mut self, _m: &Movement, _config: &Config) -> Result<()> {
        Ok(())
    }

    pub fn move_x(&mut self, _m: &AxisMovement) -> Result<(), MotorError> {
        Ok(())
    }

    pub fn move_y(&mut self, _m: &AxisMovement) -> Result<(), MotorError> {
        Ok(())
    }

    pub fn move_z(&mut self, _m: &AxisMovement) -> Result<(), MotorError> {
        Ok(())
    }
}
