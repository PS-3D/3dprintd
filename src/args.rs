use crate::{util::bail_own, APP_NAME};
use clap::Parser;
use figment::{
    value::{Dict, Map, Value},
    Metadata, Profile, Provider,
};
use std::net::IpAddr;
use tracing::Level;

fn parse_count_loglevel(arg: &str) -> Result<Level, String> {
    Ok(match arg {
        "error" => Level::ERROR,
        "warn" => Level::WARN,
        "info" => Level::INFO,
        "debug" => Level::DEBUG,
        "trace" => Level::TRACE,
        _ => bail_own!(String::from(
            "Allowed log-levels are error, warn, info, debug and trace"
        )),
    })
}

#[derive(Parser, Debug)]
#[clap(version)]
pub struct Args {
    /// Overrides the path to the config file
    #[clap(short, long, default_value_t = format!("/etc/{}/config.toml", APP_NAME))]
    pub cfg: String,
    /// Overrides port on which to run the api, default is taken from the config
    /// file or is 8080
    #[clap(short, long)]
    pub port: Option<u16>,
    /// Overrides the address on which to run the api, default is taken from the
    /// config file or is 127.0.0.1
    #[clap(short, long)]
    pub address: Option<IpAddr>,
    /// Overrides the log-level set in the config file. If none is set there, "warn"
    /// is used. Must be "error", "warn", "info", "debug" or "trace"
    #[clap(short, long, value_parser = parse_count_loglevel)]
    pub log_level: Option<Level>,
}

impl Provider for Args {
    fn metadata(&self) -> Metadata {
        Metadata::named("program argument(s)").interpolater(|_, path| match path {
            ["log", "level"] => String::from("-l/--log-level"),
            ["api", "port"] => String::from("-p/--port"),
            ["api", "address"] => String::from("-a/--address"),
            _ => unreachable!(),
        })
    }

    fn data(&self) -> Result<Map<Profile, Dict>, figment::Error> {
        let mut log = Map::new();
        if let Some(l) = &self.log_level {
            log.insert(
                String::from("level"),
                Value::from(format!("{}", l).to_ascii_lowercase()),
            );
        }
        let mut api = Map::new();
        if let Some(p) = self.port {
            api.insert(String::from("port"), Value::from(p));
        }
        if let Some(a) = &self.address {
            // done this way instead of making address a string directly so that
            // if it doesn't have the right format, clap will throw an error and
            // not figment since having a config error if the arg is wrong
            // might be a little cryptic
            api.insert(String::from("address"), Value::from(format!("{}", a)));
        }
        let mut vals = Map::new();
        vals.insert(String::from("log"), Value::from(log));
        vals.insert(String::from("api"), Value::from(api));
        let mut map = Map::new();
        map.insert(Profile::Global, vals);
        Ok(map)
    }
}

pub fn args() -> Args {
    Args::parse()
}
