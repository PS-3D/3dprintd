mod args;
mod motors;

use crate::APP_NAME;
use anyhow::Result;
use args::Args;
use clap::Parser;
use figment::{
    providers::{Format, Toml},
    Figment,
};
pub use motors::{AxisMotor, ExtruderMotor, Motors};
use rocket::config::{Config as RocketConfig, Ident};
use serde::Deserialize;
use std::{
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
};
use tracing::debug;

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
    pub api: Api,
    pub motors: Motors,
    #[serde(default)]
    pub pi: Pi,
    pub hotend: Hotend,
    pub bed: Bed,
}

//

pub fn config() -> Result<Config> {
    let args = Args::parse();
    debug!("Args are: {:?}", args);
    let cfg = Figment::new()
        .merge(Toml::file(&args.cfg))
        .merge(&args)
        .extract()?;
    // TODO add sanitycheck, e.g. to verify that the motor values aren't higher
    // than the limits etc.
    debug!("Config is: {:?}", cfg);
    Ok(cfg)
}
