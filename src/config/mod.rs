mod args;

use anyhow::Result;
use args::Args;
use clap::Parser;
use figment::{
    providers::{Format, Toml},
    Figment,
};
use log::debug;
use rocket::config::Config as RocketConfig;
use serde::Deserialize;
use std::net::{IpAddr, Ipv4Addr};

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
pub struct Config {
    pub api: Api,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api: Api::default(),
        }
    }
}

//

pub fn config() -> Result<Config> {
    let args = Args::parse();
    debug!("Args are: {:?}", args);
    let cfg = Figment::new()
        .merge(Toml::file(&args.cfg))
        .merge(&args)
        .extract()?;
    debug!("Config is: {:?}", cfg);
    Ok(cfg)
}
