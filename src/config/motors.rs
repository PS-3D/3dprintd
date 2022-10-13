use crate::comms::Axis;
use nanotec_stepper_driver::{RotationDirection, StepMode};
use num_traits::FromPrimitive;
use serde::{
    de::{Unexpected, Visitor},
    Deserialize, Deserializer,
};

struct StepModeVisitor();

impl<'de> Visitor<'de> for StepModeVisitor {
    type Value = StepMode;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "a valid step size",)
    }

    // FIXME add other visit types
    // has to be i64 due to toml only deserializing i64s
    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if v != 254 && v != 255 {
            match StepMode::from_i64(v) {
                Some(m) => return Ok(m),
                None => (),
            }
        }

        Err(serde::de::Error::invalid_value(
            Unexpected::Signed(v),
            &self,
        ))
    }
}

fn deserialize_step_size<'de, D>(deserializer: D) -> Result<StepMode, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_str(StepModeVisitor())
}

struct RotationDirectionVisitor();

impl<'de> Visitor<'de> for RotationDirectionVisitor {
    type Value = RotationDirection;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "either \"left\" or \"right\"")
    }

    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if v == "left" {
            Ok(RotationDirection::Left)
        } else if v == "right" {
            Ok(RotationDirection::Right)
        } else {
            Err(serde::de::Error::invalid_value(Unexpected::Str(v), &self))
        }
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_borrowed_str(v)
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_borrowed_str(v.as_str())
    }
}

fn deserialize_rotation_direction<'de, D>(deserializer: D) -> Result<RotationDirection, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_str(RotationDirectionVisitor())
}

struct U32LimitVisitor {
    lower: u32,
    higher: u32,
}

impl<'de> Visitor<'de> for U32LimitVisitor {
    type Value = u32;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            formatter,
            "a u32 x with {} <= x <= {}",
            self.lower, self.higher
        )
    }

    // FIXME add other visit types
    // has to be i64 due to toml only deserializing i64s
    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if self.lower as i64 <= v as i64 && v <= self.higher as i64 {
            Ok(v as u32)
        } else {
            Err(serde::de::Error::invalid_value(
                Unexpected::Signed(v),
                &self,
            ))
        }
    }
}

fn deserialize_quickstop_ramp<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_u32(U32LimitVisitor {
        lower: 0,
        higher: 3_000_000,
    })
}

fn deserialize_limit<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_u32(U32LimitVisitor {
        lower: 1,
        higher: 10_000,
    })
}

fn deserialize_speed<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_u32(U32LimitVisitor {
        lower: 1,
        higher: 1_000_000,
    })
}

fn deserialize_accel_decel<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_u32(U32LimitVisitor {
        lower: 1,
        higher: 3_000_000,
    })
}

fn deserialize_jerk<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_u32(U32LimitVisitor {
        lower: 1,
        higher: 100_000_000,
    })
}

struct BaudRateVisitor();

impl<'de> Visitor<'de> for BaudRateVisitor {
    type Value = u32;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "a valid baud rate for the motor")
    }

    // FIXME add other visit types
    // has to be i64 due to toml only deserializing i64s
    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let v = match v {
            110 => v,
            300 => v,
            600 => v,
            1200 => v,
            4800 => v,
            9600 => v,
            14400 => v,
            19200 => v,
            38400 => v,
            57600 => v,
            115200 => v,
            _ => {
                return Err(serde::de::Error::invalid_value(
                    Unexpected::Signed(v),
                    &self,
                ))
            }
        };
        Ok(v as u32)
    }
}

fn deserialize_baudrate<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_u32(BaudRateVisitor())
}

//

fn default_speed_limit() -> u32 {
    1_000_000
}

fn default_accel_limit() -> u32 {
    3_000_000
}

fn default_decel_limit() -> u32 {
    3_000_000
}

fn default_accel_jerk_limit() -> u32 {
    100_000_000
}

fn default_decel_jerk_limit() -> u32 {
    100_000_000
}

fn default_speed() -> u32 {
    1_000
}

fn default_accel_decel() -> u32 {
    50_000
}

fn default_jerk() -> u32 {
    100_000
}

fn default_baud_rate() -> u32 {
    115200
}

//

#[derive(Debug, Deserialize)]
pub struct AxisMotor {
    pub address: u8,
    // translation of the axis, i.e. mm moved per rotation
    pub translation: f64,
    // microsteps per 1.8°
    #[serde(
        default = "StepMode::default",
        deserialize_with = "deserialize_step_size"
    )]
    pub step_size: StepMode,
    // used for 1.5.45 Setting the quick stop ramp (without conversion)
    // [0,3_000_000]
    // used for estop etc.
    #[serde(deserialize_with = "deserialize_quickstop_ramp")]
    pub quickstop_ramp: u32,
    // limit of the axis in mm
    // limited to 10000 mm, more or less arbitrary
    // chosen in mm rather than steps so steps can be adjusted independently
    // FIXME convert to f64?
    #[serde(deserialize_with = "deserialize_limit")]
    pub limit: u32,
    #[serde(
        default = "default_speed_limit",
        deserialize_with = "deserialize_speed"
    )]
    pub speed_limit: u32,
    #[serde(
        default = "default_accel_limit",
        deserialize_with = "deserialize_accel_decel"
    )]
    pub accel_limit: u32,
    #[serde(
        default = "default_decel_limit",
        deserialize_with = "deserialize_accel_decel"
    )]
    pub decel_limit: u32,
    #[serde(
        default = "default_accel_jerk_limit",
        deserialize_with = "deserialize_jerk"
    )]
    pub accel_jerk_limit: u32,
    #[serde(
        default = "default_decel_jerk_limit",
        deserialize_with = "deserialize_jerk"
    )]
    pub decel_jerk_limit: u32,
    #[serde(deserialize_with = "deserialize_rotation_direction")]
    pub endstop_direction: RotationDirection,
    #[serde(default = "default_speed", deserialize_with = "deserialize_speed")]
    pub default_reference_speed: u32,
    #[serde(
        default = "default_accel_decel",
        deserialize_with = "deserialize_accel_decel"
    )]
    pub default_reference_accel: u32,
    #[serde(default = "default_jerk", deserialize_with = "deserialize_jerk")]
    pub default_reference_jerk: u32,
}

#[derive(Debug, Deserialize)]
pub struct ExtruderMotor {
    pub address: u8,
    // turning direction in order to push the filament forward, i.e. positive direction
    #[serde(deserialize_with = "deserialize_rotation_direction")]
    pub positive_direction: RotationDirection,
    // translation of the axis, i.e. mm moved per rotation
    pub translation: f64,
    // microsteps per 1.8°
    #[serde(
        default = "StepMode::default",
        deserialize_with = "deserialize_step_size"
    )]
    pub step_size: StepMode,
    // used for 1.5.45 Setting the quick stop ramp (without conversion)
    // [0,3_000_000]
    // used for estop etc.
    #[serde(deserialize_with = "deserialize_quickstop_ramp")]
    pub quickstop_ramp: u32,
    #[serde(
        default = "default_speed_limit",
        deserialize_with = "deserialize_speed"
    )]
    pub speed_limit: u32,
    #[serde(
        default = "default_accel_limit",
        deserialize_with = "deserialize_accel_decel"
    )]
    pub accel_limit: u32,
    #[serde(
        default = "default_decel_limit",
        deserialize_with = "deserialize_accel_decel"
    )]
    pub decel_limit: u32,
    #[serde(
        default = "default_accel_jerk_limit",
        deserialize_with = "deserialize_jerk"
    )]
    pub accel_jerk_limit: u32,
    #[serde(
        default = "default_decel_jerk_limit",
        deserialize_with = "deserialize_jerk"
    )]
    pub decel_jerk_limit: u32,
}

// can't implement default because port must not have a default since it could
// in theory break stuff
// same goes for motor addresses
#[derive(Debug, Deserialize)]
pub struct Motors {
    pub port: String,
    #[serde(
        default = "default_baud_rate",
        deserialize_with = "deserialize_baudrate"
    )]
    pub baud_rate: u32,
    // TODO maybe make optional?
    // timeout of the serialport in seconds
    pub timeout: u64,
    pub x: AxisMotor,
    pub y: AxisMotor,
    pub z: AxisMotor,
    pub e: ExtruderMotor,
}

impl Motors {
    pub fn axis(&self, axis: &Axis) -> &AxisMotor {
        match axis {
            Axis::X => &self.x,
            Axis::Y => &self.y,
            Axis::Z => &self.z,
        }
    }
}
