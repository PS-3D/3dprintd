mod motors;

use crate::{args::Args, APP_NAME};
use anyhow::Result;
use figment::{
    providers::{Format, Toml},
    Figment,
};
pub use motors::{AxisMotor, ExtruderMotor, Motors};
use rocket::config::{Config as RocketConfig, Ident};
use serde::{
    de::{Error as DeError, Unexpected, Visitor},
    Deserialize, Deserializer,
};
use std::{
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
};
use tracing::Level;

//

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct General {
    // FIXME force to be absolute
    pub settings_path: PathBuf,
}

impl Default for General {
    fn default() -> Self {
        Self {
            settings_path: PathBuf::from(format!("/var/lib/{}/settings.json", APP_NAME)),
        }
    }
}

//

struct LogLevelVisitor();

impl<'de> Visitor<'de> for LogLevelVisitor {
    type Value = Level;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            formatter,
            "one of \"error\", \"warn\", \"info\", \"debug\" or \"trace\""
        )
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(match v {
            "error" => Level::ERROR,
            "warn" => Level::WARN,
            "info" => Level::INFO,
            "debug" => Level::DEBUG,
            "trace" => Level::TRACE,
            _ => return Err(DeError::invalid_value(Unexpected::Str(v), &self)),
        })
    }
}

fn deserialize_log_level<'de, D>(deserializerd: D) -> Result<Level, D::Error>
where
    D: Deserializer<'de>,
{
    deserializerd.deserialize_str(LogLevelVisitor())
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Log {
    #[serde(deserialize_with = "deserialize_log_level")]
    pub level: Level,
}

impl Default for Log {
    fn default() -> Self {
        Self { level: Level::WARN }
    }
}

//

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Api {
    pub address: IpAddr,
    pub port: u16,
    pub workers: usize,
}

impl Default for Api {
    fn default() -> Self {
        Self {
            address: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port: 8080,
            workers: 2,
        }
    }
}

impl From<&Api> for RocketConfig {
    fn from(api: &Api) -> Self {
        let mut cfg = Self::default();
        cfg.address = api.address;
        cfg.port = api.port;
        cfg.workers = api.workers;
        cfg.ident = Ident::try_new(APP_NAME).unwrap();
        cfg
    }
}

impl From<Api> for RocketConfig {
    fn from(api: Api) -> Self {
        api.into()
    }
}

//

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Pi {
    // interval in which to check the values in milliseconds
    pub check_interval: u64,
}

impl Default for Pi {
    fn default() -> Self {
        Self { check_interval: 1 }
    }
}

//

#[derive(Debug, Deserialize)]
pub struct Hotend {
    // temp limit in Celsius
    pub upper_limit: u16,
    // temp limit in Celsius
    // should be somewhere around 40, since temperatures below that might
    // be reached naturally and as such might lead to problems
    // FIXME maybe add default?
    pub lower_limit: u16,
}

//

#[derive(Debug, Deserialize)]
pub struct Bed {
    // temp limit in Celsius
    pub upper_limit: u16,
    // lower templimit in Celsius
    // should be somewhere around 40, since temperatures below that might
    // be reached naturally and as such might lead to problems
    // FIXME maybe add default?
    pub lower_limit: u16,
}

//

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: General,
    #[serde(default)]
    pub log: Log,
    #[serde(default)]
    pub api: Api,
    pub motors: Motors,
    #[serde(default)]
    pub pi: Pi,
    pub hotend: Hotend,
    pub bed: Bed,
}

//

pub fn config(args: &Args) -> Result<Config> {
    let cfg = Figment::new()
        .merge(Toml::file(&args.cfg))
        .merge(&args)
        .extract()?;
    // TODO add sanitycheck, e.g. to verify that the motor values aren't higher
    // than the limits etc.
    Ok(cfg)
}
