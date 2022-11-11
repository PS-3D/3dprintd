use rocket::request::FromParam;
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub struct ReferenceRunOptParameters {
    pub speed: Option<u32>,
    pub accel_decel: Option<u32>,
    pub jerk: Option<u32>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl FromParam<'_> for Axis {
    type Error = &'static str;

    fn from_param(param: &str) -> Result<Self, Self::Error> {
        match param {
            "x" => Ok(Self::X),
            "y" => Ok(Self::Y),
            "z" => Ok(Self::Z),
            _ => Err("not a valid axis, must be x, y, or z"),
        }
    }
}

#[derive(Debug)]
pub enum ControlComms<T> {
    Msg(T),
    Exit,
}
