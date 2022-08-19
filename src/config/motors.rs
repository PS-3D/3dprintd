use nanotec_stepper_driver::RotationDirection;
use serde::{
    de::{Unexpected, Visitor},
    Deserialize, Deserializer,
};

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

fn deserialize_endstop_direction<'de, D>(deserializer: D) -> Result<RotationDirection, D::Error>
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
                Unexpected::Unsigned(v as u64),
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
                    Unexpected::Unsigned(v as u64),
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

#[derive(Debug, Deserialize)]
pub struct AxisMotor {
    pub address: u8,
    // used for 1.5.45 Setting the quick stop ramp (without conversion)
    // [0,3_000_000]
    // used for estop etc.
    #[serde(deserialize_with = "deserialize_quickstop_ramp")]
    pub quickstop_ramp: u32,
    #[serde(deserialize_with = "deserialize_endstop_direction")]
    pub endstop_direction: RotationDirection,
}

#[derive(Debug, Deserialize)]
pub struct ExtruderMotor {
    pub address: u8,
    // used for 1.5.45 Setting the quick stop ramp (without conversion)
    // [0,3_000_000]
    // used for estop etc.
    #[serde(deserialize_with = "deserialize_quickstop_ramp")]
    pub quickstop_ramp: u32,
}

//

fn default_baud_rate() -> u32 {
    115200
}

//

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
