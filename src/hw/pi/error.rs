use anyhow::Error;
use std::fmt::Display;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WaitTempError {
    #[error("The target temperature changed to an incompatible temperature")]
    TargetChanged,
}

/// Thrown if an error occurs while trying to exit the pi thread
///
/// Since that is kinda critical, we want to e.g. try and turn off the bed, even
/// tho trying to turn off the hotend failed
#[derive(Debug, Error)]
pub struct ExitError(Vec<Error>);

impl Display for ExitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "while trying to exit, the following errors occured:\n\n",)?;
        for e in self.0.iter() {
            write!(f, "{}\n--------------------------------------------------------------------------------\n", e)?;
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum PiCtrlError {
    #[error("target temperature is out of bounds, was {}, must be in range [{};{}]", .0, .1, .2)]
    TargetOutOfBounds(u16, u16, u16),
}
