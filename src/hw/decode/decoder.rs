use super::{
    super::comms::{Action, AxisMovement, ExtruderMovement, GCode, Movement},
    error::GCodeError,
};
use crate::{
    comms::{Axis, OnewayAtomicF64Read, OnewayAtomicF64Write, ReferenceRunOptParameters},
    log::target,
    settings::Settings,
    util::{bail_own, ensure_own},
};
use anyhow::Result;
use gcode::Mnemonic;
use nanotec_stepper_driver::StepMode;
use std::{collections::VecDeque, time::Duration};
use tracing::trace;

type GCodeResult<T> = Result<T, GCodeError>;

#[derive(Debug, PartialEq, Eq)]
enum CoordMode {
    Absolute,
    Relative,
}

#[derive(Debug, PartialEq, Eq)]
enum Unit {
    Millimeters,
    Inches,
}

impl Unit {
    pub fn in_mm(&self, val: f64) -> f64 {
        match self {
            Self::Millimeters => val,
            Self::Inches => val * 25.4,
        }
    }
}

macro_rules! assert_code {
    ($code:expr, $mnemonic:ident, $major:literal, $minor:literal) => {
        assert_eq!($code.mnemonic(), Mnemonic::$mnemonic);
        assert_eq!($code.major_number(), $major);
        assert_eq!($code.minor_number(), $minor);
    };
}

fn extract_temp_from_code(
    code: GCode,
    lower_limit: u16,
    upper_limit: u16,
) -> GCodeResult<(Option<u16>, GCode)> {
    ensure_own!(
        !code.arguments().is_empty(),
        GCodeError::MissingArguments(code)
    );
    let mut temp = None;
    for arg in code.arguments() {
        match arg.letter {
            'S' => {
                ensure_own!(temp.is_none(), GCodeError::DuplicateArgument(*arg, code));
                temp = Some(arg.value as u16)
            }
            _ => bail_own!(GCodeError::UnknownArgument(*arg, code)),
        };
    }
    let temp = temp.unwrap();
    if temp == 0 {
        Ok((None, code))
    } else {
        ensure_own!(
            lower_limit <= temp && temp <= upper_limit,
            GCodeError::TempOutOfBounds(code.clone(), lower_limit, upper_limit)
        );
        Ok((Some(temp), code))
    }
}

// (distance_in_mm / translation) * (360/1.8) * microsteps_per_step
// conversion from StepMode to f64 can't happen directly so we have to
// do it this way
fn mm_to_steps(mm: f64, translation: &f64, step_size: &StepMode) -> f64 {
    ((mm / translation) * (360.0 / 1.8) * (*step_size as u8) as f64).round()
}

// FIXME maybe change to fixed point?
#[derive(Debug)]
pub struct Decoder {
    settings: Settings,
    feedrate: Option<f64>,
    prog_x: f64,
    prog_y: f64,
    prog_z: f64,
    prog_e: f64,
    actual_x: f64,
    actual_y: f64,
    actual_z: f64,
    steps_x: u32,
    steps_y: u32,
    // not u32, because z position operates in the negative since the
    // endstop is at the positive end of the z-axis
    steps_z: i32,
    xyz_coord_mode: CoordMode,
    e_coord_mode: CoordMode,
    unit: Unit,
    hotend_target_temp: Option<u16>,
    bed_target_temp: Option<u16>,
    z_hotend_location: OnewayAtomicF64Read,
}

impl Decoder {
    pub fn new(settings: Settings, z_hotend_location: OnewayAtomicF64Read) -> Self {
        let actual_z = z_hotend_location.read();
        let steps_z = mm_to_steps(
            actual_z,
            &settings.config().motors.z.translation,
            &settings.config().motors.z.step_size,
        ) as i32;
        Self {
            settings,
            feedrate: None,
            prog_x: 0.0,
            prog_y: 0.0,
            prog_z: 0.0,
            prog_e: 0.0,
            actual_x: 0.0,
            actual_y: 0.0,
            actual_z,
            steps_x: 0,
            steps_y: 0,
            steps_z,
            xyz_coord_mode: CoordMode::Absolute,
            e_coord_mode: CoordMode::Relative,
            unit: Unit::Millimeters,
            hotend_target_temp: None,
            bed_target_temp: None,
            z_hotend_location: OnewayAtomicF64Read::new(actual_z),
        }
    }

    pub fn get_z_hotend_location_write(&self) -> OnewayAtomicF64Write {
        self.z_hotend_location.get_write()
    }

    fn g0_1(&mut self, code: GCode) -> GCodeResult<(Action, GCode)> {
        ensure_own!(
            !code.arguments().is_empty(),
            GCodeError::MissingArguments(code)
        );
        let mut x = None;
        let mut y = None;
        let mut z = None;
        let mut e = None;
        let mut f = None;
        for arg in code.arguments().iter() {
            let letter = match arg.letter {
                'X' => &mut x,
                'Y' => &mut y,
                'Z' => &mut z,
                'E' => &mut e,
                'F' => &mut f,
                _ => bail_own!(GCodeError::UnknownArgument(*arg, code)),
            };
            ensure_own!(letter.is_none(), GCodeError::DuplicateArgument(*arg, code));
            *letter = Some(self.unit.in_mm(arg.value as f64));
        }
        let mut x = x.unwrap_or_default();
        let mut y = y.unwrap_or_default();
        let mut z = z.unwrap_or_default();
        let mut e = e.unwrap_or_default();

        fn calc_rel(new_coord: &mut f64, prog_coord: &mut f64) {
            let rel_coord = *new_coord - *prog_coord;
            *prog_coord = *new_coord;
            *new_coord = rel_coord;
        }

        // make x, y and z relative so we can calculate with them
        if self.xyz_coord_mode == CoordMode::Absolute {
            calc_rel(&mut x, &mut self.prog_x);
            calc_rel(&mut y, &mut self.prog_y);
            calc_rel(&mut z, &mut self.prog_z);
        } else {
            self.prog_x += x;
            self.prog_x += y;
            self.prog_x += z;
        }
        // make e relative so we can calculate with it
        if self.e_coord_mode == CoordMode::Absolute {
            calc_rel(&mut e, &mut self.prog_e);
        } else {
            self.prog_e += e;
        }

        let cfg = self.settings.config();

        let actual_x_new = self.actual_x + x;
        let actual_y_new = self.actual_y + y;
        let actual_z_new = self.actual_z + z;
        // check lower limit
        ensure_own!(actual_x_new >= 0.0, GCodeError::PosOutOfBounds(code));
        ensure_own!(actual_y_new >= 0.0, GCodeError::PosOutOfBounds(code));
        ensure_own!(
            actual_z_new >= self.z_hotend_location.read(),
            GCodeError::PosOutOfBounds(code)
        );
        // check upper limits
        ensure_own!(
            actual_x_new <= cfg.motors.x.limit as f64,
            GCodeError::PosOutOfBounds(code)
        );
        ensure_own!(
            actual_y_new <= cfg.motors.y.limit as f64,
            GCodeError::PosOutOfBounds(code)
        );
        ensure_own!(actual_z_new <= 0.0, GCodeError::PosOutOfBounds(code));
        self.actual_x = actual_x_new;
        self.actual_y = actual_y_new;
        self.actual_z = actual_z_new;

        // save the feedrate for the next instructions
        // unfortunately this seems to be widely used in gcode
        if let Some(f) = f {
            self.feedrate = Some(f as f64);
        }
        let f = self
            .feedrate
            .ok_or(GCodeError::MissingArguments(code.clone()))?;

        // CALCULATION

        // distance in mm
        let s = (x * x + y * y + z * z).sqrt();
        // time in s
        let t = s / (f / 60.0);
        // distance in steps
        let x = mm_to_steps(x, &cfg.motors.x.translation, &cfg.motors.x.step_size);
        let y = mm_to_steps(y, &cfg.motors.y.translation, &cfg.motors.y.step_size);
        let z = mm_to_steps(z, &cfg.motors.z.translation, &cfg.motors.z.step_size);
        let e = mm_to_steps(e, &cfg.motors.e.translation, &cfg.motors.e.step_size);

        // speed in steps/second
        // distance_in_steps / time
        let mut v_x = (x / t).round();
        let mut v_y = (y / t).round();
        let mut v_z = (z / t).round();
        let mut v_e = (e / t).round();

        macro_rules! limit {
            ($axis:ident.$limit_name:ident, $limit_axis:ident, $limit_1:ident, $limit_2:ident, $limit_3:ident) => {{
                if $limit_axis > cfg.motors.$axis.$limit_name as f64 {
                    let limit_new = cfg.motors.$axis.$limit_name as f64;
                    $limit_1 = (($limit_1 / $limit_axis) * limit_new).round();
                    $limit_2 = (($limit_2 / $limit_axis) * limit_new).round();
                    $limit_3 = (($limit_3 / $limit_axis) * limit_new).round();
                    $limit_axis = limit_new;
                }
            }};
        }

        // fix speed if it hits any of the limits
        // afterwards it shouldn't be hitting any limit
        limit!(x.speed_limit, v_x, v_y, v_z, v_e);
        limit!(y.speed_limit, v_y, v_x, v_z, v_e);
        limit!(z.speed_limit, v_z, v_x, v_y, v_e);
        limit!(e.speed_limit, v_e, v_x, v_y, v_z);

        macro_rules! calc_by_choosing {
            ($limit_name:ident, $last_x:ident, $last_y:ident, $last_z:ident, $last_e:ident) => {{
                let mut x = cfg.motors.x.$limit_name as f64;
                let t = $last_x / x;
                let mut y = ($last_y / t).round();
                let mut z = ($last_z / t).round();
                let mut e = ($last_e / t).round();

                limit!(y.$limit_name, y, x, z, e);
                limit!(z.$limit_name, z, x, y, e);
                limit!(e.$limit_name, e, x, y, z);
                (x, y, z, e)
            }};
        }

        // accel in steps/s^2
        let (a0_x, a0_y, a0_z, a0_e) = calc_by_choosing!(accel_limit, v_x, v_y, v_z, v_e);
        // accel jerk in steps/s^3
        let (j0_x, j0_y, j0_z, j0_e) = calc_by_choosing!(accel_jerk_limit, a0_x, a0_y, a0_z, a0_e);
        // decel in steps/s^2
        let (a1_x, a1_y, a1_z, a1_e) = calc_by_choosing!(decel_limit, v_x, v_y, v_z, v_e);
        // decel jerk in steps/s^3
        let (j1_x, j1_y, j1_z, j1_e) = calc_by_choosing!(decel_jerk_limit, a1_x, a1_y, a1_z, a1_e);

        self.steps_x += x as u32;
        self.steps_y += y as u32;
        self.steps_z += z as i32;

        let mut e_direction = cfg.motors.e.positive_direction;
        if e < 0.0 {
            e_direction = e_direction.reverse();
        }

        let movement = Movement {
            x: AxisMovement {
                distance: self.steps_x as i32,
                min_frequency: 1,
                max_frequency: v_x as u32,
                acceleration: a0_x as u32,
                deceleration: a1_x as u32,
                acceleration_jerk: j0_x as u32,
                deceleration_jerk: j1_x as u32,
            },
            y: AxisMovement {
                distance: self.steps_y as i32,
                min_frequency: 1,
                max_frequency: v_y as u32,
                acceleration: a0_y as u32,
                deceleration: a1_y as u32,
                acceleration_jerk: j0_y as u32,
                deceleration_jerk: j1_y as u32,
            },
            z: AxisMovement {
                distance: self.steps_z,
                min_frequency: 1,
                max_frequency: v_z as u32,
                acceleration: a0_z as u32,
                deceleration: a1_z as u32,
                acceleration_jerk: j0_z as u32,
                deceleration_jerk: j1_z as u32,
            },
            e: ExtruderMovement {
                direction: e_direction,
                distance: e as u32,
                min_frequency: 1,
                max_frequency: v_e as u32,
                acceleration: a0_e as u32,
                deceleration: a1_e as u32,
                acceleration_jerk: j0_e as u32,
                deceleration_jerk: j1_e as u32,
            },
        };

        // TODO check code output of macros

        Ok((Action::MoveAll(movement), code))
    }

    /// Executes G0 command (does the same as [`g1`][Self::g1])
    ///
    /// Supported arguments: `X`, `Y`, `Z`, `E` and `F`
    ///
    /// # Errors
    /// At least one argument must be present, otherwise [`GCodeError::MissingArguments`]
    /// will be returned. Same if `F` is not present and has not been present
    /// before.
    fn g0(&mut self, code: GCode) -> GCodeResult<(Action, GCode)> {
        assert_code!(code, General, 0, 0);
        self.g0_1(code)
    }

    /// Executes G1 command (does the same as [`g0`][Self::g0])
    ///
    /// Supported arguments: `X`, `Y`, `Z`, `E` and `F`
    ///
    /// # Errors
    /// At least one argument must be present, otherwise [`GCodeError::MissingArguments`]
    /// will be returned. Same if `F` is not present and has not been present
    /// before.
    fn g1(&mut self, code: GCode) -> GCodeResult<(Action, GCode)> {
        assert_code!(code, General, 1, 0);
        self.g0_1(code)
    }

    /// Executes G4 command
    ///
    /// Supported arguments: `P` and `S`
    ///
    /// # Errors
    /// At least one argument must be present, otherwise [`GCodeError::MissingArguments`]
    /// will be returned.
    fn g4(&mut self, code: GCode) -> GCodeResult<(Action, GCode)> {
        assert_code!(code, General, 4, 0);
        ensure_own!(
            !code.arguments().is_empty(),
            GCodeError::MissingArguments(code)
        );
        let mut millis = None;
        let mut secs = None;
        for arg in code.arguments().iter() {
            match arg.letter {
                'P' => {
                    ensure_own!(millis.is_none(), GCodeError::DuplicateArgument(*arg, code));
                    millis = Some(Duration::from_millis(arg.value as u64));
                }
                'S' => {
                    ensure_own!(secs.is_none(), GCodeError::DuplicateArgument(*arg, code));
                    secs = Some(Duration::from_secs(arg.value as u64));
                }
                _ => bail_own!(GCodeError::UnknownArgument(*arg, code)),
            }
        }
        let combined = millis.unwrap_or_default() + secs.unwrap_or_default();
        Ok((Action::Wait(combined), code))
    }

    /// Executes G20 command
    ///
    /// Supported arguments: None
    ///
    /// Warning: Since this software is sane, it uses mm internally, so it would
    /// be wise to just use mm in general
    fn g20(&mut self, code: GCode) -> GCodeResult<()> {
        assert_code!(code, General, 20, 0);
        ensure_own!(
            code.arguments().is_empty(),
            GCodeError::UnknownArgument(*code.arguments().first().unwrap(), code)
        );
        self.unit = Unit::Inches;
        Ok(())
    }

    /// Executes G21 command
    ///
    /// Supported arguments: None
    fn g21(&mut self, code: GCode) -> GCodeResult<()> {
        assert_code!(code, General, 21, 0);
        ensure_own!(
            code.arguments().is_empty(),
            GCodeError::UnknownArgument(*code.arguments().first().unwrap(), code)
        );
        self.unit = Unit::Millimeters;
        Ok(())
    }

    /// Executes G28 command
    ///
    /// Supported arguments: `X`, `Y` and `Z`
    ///
    /// No arguments will assume all arguments present.
    ///
    /// Won't actually home Z axis, only X and Y, since the Z axis endstop is at
    /// the bottom and homing it might destroy the manual homing measurement.
    // FIXME maybe we could home the z axis by setting the power down to where
    //       it wouldn't hurt the print head and then slowly move the bed
    //       into the printhead and then zeroeing?
    // FIXME drive given axis to origin
    fn g28(&mut self, code: GCode) -> GCodeResult<VecDeque<(Action, GCode)>> {
        assert_code!(code, General, 28, 0);
        let mut x = false;
        let mut y = false;
        if code.arguments().is_empty() {
            x = true;
            y = true;
        } else {
            for arg in code.arguments().iter() {
                let letter = match arg.letter {
                    'X' => &mut x,
                    'Y' => &mut y,
                    _ => bail_own!(GCodeError::UnknownArgument(*arg, code)),
                };
                *letter = true;
            }
        }
        let mut actions = VecDeque::with_capacity(2);
        // Can't use ReferenceAll because that would home Z axis as well.
        if x {
            actions.push_back((
                Action::ReferenceAxis(Axis::X, ReferenceRunOptParameters::default()),
                code.clone(),
            ));
        }
        if y {
            actions.push_back((
                Action::ReferenceAxis(Axis::Y, ReferenceRunOptParameters::default()),
                code,
            ));
        }
        Ok(actions)
    }

    /// Executes G90 command
    ///
    /// Supported arguments: None
    fn g90(&mut self, code: GCode) -> GCodeResult<()> {
        assert_code!(code, General, 90, 0);
        ensure_own!(
            code.arguments().is_empty(),
            GCodeError::UnknownArgument(*code.arguments().first().unwrap(), code)
        );
        self.xyz_coord_mode = CoordMode::Absolute;
        Ok(())
    }

    /// Executes G91 command
    ///
    /// Supported arguments: None
    fn g91(&mut self, code: GCode) -> GCodeResult<()> {
        assert_code!(code, General, 91, 0);
        ensure_own!(
            code.arguments().is_empty(),
            GCodeError::UnknownArgument(*code.arguments().first().unwrap(), code)
        );
        self.xyz_coord_mode = CoordMode::Relative;
        Ok(())
    }

    /// Executes G92 command
    ///
    /// Supported arguments: `X`, `Y`, `Z` and `E`
    ///
    /// # Errors
    /// At least one argument must be present, otherwise [`GCodeError::MissingArguments`]
    /// will be returned
    fn g92(&mut self, code: GCode) -> GCodeResult<()> {
        assert_code!(code, General, 92, 0);
        ensure_own!(
            !code.arguments().is_empty(),
            GCodeError::MissingArguments(code)
        );
        let mut x = None;
        let mut y = None;
        let mut z = None;
        let mut e = None;
        for arg in code.arguments() {
            let letter = match arg.letter {
                'X' => &mut x,
                'Y' => &mut y,
                'Z' => &mut z,
                'E' => &mut e,
                _ => bail_own!(GCodeError::UnknownArgument(*arg, code)),
            };
            ensure_own!(letter.is_none(), GCodeError::DuplicateArgument(*arg, code));
            *letter = Some(self.unit.in_mm(arg.value as f64));
        }
        self.prog_x = x.unwrap_or(self.prog_x);
        self.prog_y = y.unwrap_or(self.prog_y);
        self.prog_z = z.unwrap_or(self.prog_z);
        self.prog_e = e.unwrap_or(self.prog_e);
        Ok(())
    }

    /// Executes M82 command
    ///
    /// Supported arguments: None
    fn m82(&mut self, code: GCode) -> GCodeResult<()> {
        assert_code!(code, Miscellaneous, 82, 0);
        ensure_own!(
            code.arguments().is_empty(),
            GCodeError::UnknownArgument(*code.arguments().first().unwrap(), code)
        );
        self.e_coord_mode = CoordMode::Absolute;
        Ok(())
    }

    /// Executes M83 command
    ///
    /// Supported arguments: None
    fn m83(&mut self, code: GCode) -> GCodeResult<()> {
        assert_code!(code, Miscellaneous, 83, 0);
        ensure_own!(
            code.arguments().is_empty(),
            GCodeError::UnknownArgument(*code.arguments().first().unwrap(), code)
        );
        self.e_coord_mode = CoordMode::Relative;
        Ok(())
    }

    /// Executes M104 command
    ///
    /// Supported arguments: `S`
    fn m104(&mut self, code: GCode) -> GCodeResult<(Action, GCode)> {
        assert_code!(code, Miscellaneous, 104, 0);
        let cfg = &self.settings.config().hotend;
        let (target, code) = extract_temp_from_code(code, cfg.lower_limit, cfg.upper_limit)?;
        self.hotend_target_temp = target;
        Ok((Action::HotendTarget(self.hotend_target_temp), code))
    }

    /// Executes M109 command
    ///
    /// Supported arguments: `S`
    fn m109(&mut self, code: GCode) -> GCodeResult<VecDeque<(Action, GCode)>> {
        assert_code!(code, Miscellaneous, 109, 0);
        let cfg = &self.settings.config().hotend;
        let (target, code) = extract_temp_from_code(code, cfg.lower_limit, cfg.upper_limit)?;
        self.hotend_target_temp = target;
        let mut dq = VecDeque::with_capacity(2);
        dq.push_back((Action::HotendTarget(self.hotend_target_temp), code.clone()));
        dq.push_back((Action::WaitHotendTarget, code));
        Ok(dq)
    }

    /// Executes M140 command
    ///
    /// Supported arguments: `S`
    fn m140(&mut self, code: GCode) -> GCodeResult<(Action, GCode)> {
        assert_code!(code, Miscellaneous, 140, 0);
        let cfg = &self.settings.config().bed;
        let (target, code) = extract_temp_from_code(code, cfg.lower_limit, cfg.upper_limit)?;
        self.bed_target_temp = target;
        Ok((Action::BedTarget(self.bed_target_temp), code))
    }

    /// Executes M190 command
    ///
    /// Supported arguments: `S`
    fn m190(&mut self, code: GCode) -> GCodeResult<(Action, GCode)> {
        assert_code!(code, Miscellaneous, 190, 0);
        let cfg = &self.settings.config().bed;
        let (temp, code) = extract_temp_from_code(code, cfg.lower_limit, cfg.upper_limit)?;
        Ok((Action::WaitBedMinTemp(temp), code))
    }

    // Necessary GCode TODO:
    // G28
    //
    // Optional GCode TODO:
    // G10
    // G11
    // G2
    // G3
    // G10, for offsets
    // G29?
    //
    // Not-possible GCodes:
    // G30
    // G32

    // Optional MCode TODO:
    // M0
    // M1
    // M108
    // M116

    /// Decodes a single line of gcode
    ///
    /// Returns the Action to execute, if there is any. If the given code was
    /// one that doesn't need an Action, like `G90`, `None` is returned.
    ///
    /// `code` must contain a supported G-, M- or TCode, otherwise an Error will
    /// be thrown.
    pub fn decode(&mut self, code: GCode) -> GCodeResult<Option<VecDeque<(Action, GCode)>>> {
        trace!(
            target: target::INTERNAL,
            feedrate = self.feedrate,
            "Decoding {}",
            code,
        );
        macro_rules! vecdq {
            [$action:expr] => {{
                let mut dq = VecDeque::with_capacity(1);
                dq.push_back($action);
                dq
            }};
        }
        // since we don't implement any minor numbers:
        ensure_own!(code.minor_number() == 0, GCodeError::UnknownCode(code));
        match code.mnemonic() {
            Mnemonic::General => match code.major_number() {
                0 => self.g0(code).map(|a| Some(vecdq![a])),
                1 => self.g1(code).map(|a| Some(vecdq![a])),
                4 => self.g4(code).map(|a| Some(vecdq![a])),
                20 => self.g20(code).map(|_| None),
                21 => self.g21(code).map(|_| None),
                28 => self.g28(code).map(|dq| Some(dq)),
                90 => self.g90(code).map(|_| None),
                91 => self.g91(code).map(|_| None),
                92 => self.g92(code).map(|_| None),
                _ => bail_own!(GCodeError::UnknownCode(code)),
            },
            Mnemonic::Miscellaneous => match code.major_number() {
                82 => self.m82(code).map(|_| None),
                83 => self.m83(code).map(|_| None),
                // M84 doesn't really need to do anything either, the motors can't
                // do that afaik
                84 => Ok(None),
                104 => self.m104(code).map(|a| Some(vecdq![a])),
                // M106 and M107 don't need to do anything because control of
                // the fan happens automatically because why wouldn't it?
                // (safer for the machine and all...)
                106 => Ok(None),
                // see M106
                107 => Ok(None),
                109 => self.m109(code).map(|dq| Some(dq)),
                140 => self.m140(code).map(|a| Some(vecdq![a])),
                190 => self.m190(code).map(|a| Some(vecdq![a])),
                _ => bail_own!(GCodeError::UnknownCode(code)),
            },
            Mnemonic::ToolChange => match code.major_number() {
                // T0 doesn't need to do anything, we can't change tools anyways
                0 => Ok(None),
                _ => bail_own!(GCodeError::UnknownCode(code)),
            },
            _ => bail_own!(GCodeError::UnknownCode(code)),
        }
        // FIXME https://github.com/rust-lang/rust/issues/91345
        .map(|ok| {
            trace!(target: target::INTERNAL, "Decoded to {:?}", ok);
            ok
        })
    }

    /// Will reset values like the feedrate which should only persist in one
    /// run
    // FIXME actual_pos might not match the actual real position of the printer,
    // which might then cause it to error out once the next gcode is started
    pub fn reset(&mut self) {
        self.feedrate = None;
        self.prog_x = self.actual_x;
        self.prog_y = self.actual_y;
        self.prog_z = self.actual_z;
        self.prog_e = 0.0;
        self.xyz_coord_mode = CoordMode::Absolute;
        self.e_coord_mode = CoordMode::Relative;
        self.unit = Unit::Millimeters;
    }
}
