use gcode::{GCode, Word};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GCodeError {
    #[error("at least one argument is missing from this code: {}", .0)]
    MissingArguments(GCode),
    #[error("unknown code: {}", .0)]
    UnknownCode(GCode),
    #[error("unknown argument {} in code {}", .0, .1)]
    UnknownArgument(Word, GCode),
    #[error("duplicate argument {} in code {}", .0, .1)]
    DuplicateArgument(Word, GCode),
    #[error("code {} would cause the printer to go out of bounds", .0)]
    OutOfBounds(GCode),
}
