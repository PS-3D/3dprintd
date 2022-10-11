use nanotec_stepper_driver::DriverError;
use std::fmt::Display;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MotorError {
    #[error(transparent)]
    DriverError(#[from] DriverError),
    #[error("position error occured while driving the motor")]
    PositionError,
}

/// Able to contain errors for all motors
///
/// Be aware of the invariant that at least one of the fields should contain an
/// error.
#[derive(Debug, Error)]
pub struct MotorsError {
    pub x: Option<MotorError>,
    pub y: Option<MotorError>,
    pub z: Option<MotorError>,
    pub e: Option<MotorError>,
}

impl Display for MotorsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "at least one motor reported an error:\n")?;
        if let Some(x) = &self.x {
            write!(f, "    x: {}\n", x)?;
        }
        if let Some(y) = &self.y {
            write!(f, "    y: {}\n", y)?;
        }
        if let Some(z) = &self.z {
            write!(f, "    z: {}\n", z)?;
        }
        if let Some(e) = &self.e {
            write!(f, "    x: {}\n", e)?;
        }
        Ok(())
    }
}
