mod config;

pub use self::config::Config;
use anyhow::{Context, Error, Result};
use log::warn;
use serde::{Deserialize, Serialize};
use serde_json;
use std::{
    fs::File,
    io::{self, Read, Write},
    sync::{Arc, RwLock},
};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct InnerAxisMotorSettings {
    reference_speed: Option<u32>,
    reference_accel_decel: Option<u32>,
    reference_jerk: Option<u32>,
}

#[derive(Debug)]
// must be public because of type of AxisMotorSettings
pub struct AxisMotorSettings<F, FM, C>
where
    F: Fn(&InnerMotorSettings) -> &InnerAxisMotorSettings,
    FM: Fn(&mut InnerMotorSettings) -> &mut InnerAxisMotorSettings,
    C: Fn(&config::Motors) -> &config::AxisMotor,
{
    f: F,
    fm: FM,
    c: C,
    config: Arc<Config>,
    settings: Arc<RwLock<InnerSettings>>,
}

macro_rules! get_settings_motor {
    ($self:ident, $setting:ident, $config:ident) => {{
        ($self.f)(&$self.settings.read().unwrap().motors)
            .$setting
            .unwrap_or(($self.c)(&$self.config.motors).$config)
    }};
}

impl<F, FM, C> AxisMotorSettings<F, FM, C>
where
    F: Fn(&InnerMotorSettings) -> &InnerAxisMotorSettings,
    FM: Fn(&mut InnerMotorSettings) -> &mut InnerAxisMotorSettings,
    C: Fn(&config::Motors) -> &config::AxisMotor,
{
    pub fn get_reference_speed(&self) -> u32 {
        get_settings_motor!(self, reference_speed, default_reference_speed)
    }

    pub fn get_reference_accel_decel(&self) -> u32 {
        get_settings_motor!(self, reference_accel_decel, default_reference_accel)
    }

    pub fn get_reference_jerk(&self) -> u32 {
        get_settings_motor!(self, reference_jerk, default_reference_jerk)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
// must be public because of type of AxisMotorSettings
pub struct InnerMotorSettings {
    x: InnerAxisMotorSettings,
    y: InnerAxisMotorSettings,
    z: InnerAxisMotorSettings,
}

#[derive(Debug)]
pub struct MotorSettings {
    config: Arc<Config>,
    settings: Arc<RwLock<InnerSettings>>,
}

macro_rules! make_ams {
    ($axis:ident) => {
        pub fn $axis(
            &self,
        ) -> AxisMotorSettings<
            impl Fn(&InnerMotorSettings) -> &InnerAxisMotorSettings,
            impl Fn(&mut InnerMotorSettings) -> &mut InnerAxisMotorSettings,
            impl Fn(&config::Motors) -> &config::AxisMotor,
        > {
            AxisMotorSettings {
                f: |s| &s.$axis,
                fm: |s| &mut s.$axis,
                c: |c| &c.$axis,
                config: self.config.clone(),
                settings: self.settings.clone(),
            }
        }
    };
}

impl MotorSettings {
    make_ams!(x);
    make_ams!(y);
    make_ams!(z);
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct InnerSettings {
    motors: InnerMotorSettings,
}

#[derive(Debug, Clone)]
pub struct Settings {
    config: Arc<Config>,
    settings: Arc<RwLock<InnerSettings>>,
}

impl Settings {
    fn new(cfg: Config) -> Result<Self> {
        // if the file doesn't exist, we don't want to error out, we can just
        // use the default values
        let file = match File::open(&cfg.general.settings_path) {
            Ok(f) => Some(f),
            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    None
                } else {
                    // TODO check if it might work better with tracing/log
                    return Err(Error::from(e)).context("Failed to open settings-file");
                }
            }
        };
        let inner: InnerSettings = {
            if let Some(mut f) = file {
                // if file did exist, it still might be empty, in which case
                // we also need to use default values, serde_jsone doesn't do that
                //
                // size chosen more or less arbitrarily, should fit any settings
                // file and isn't too big
                let mut contents = String::with_capacity(512);
                f.read_to_string(&mut contents)?;
                if !contents.trim().is_empty() {
                    serde_json::from_str(&contents)?
                } else {
                    warn!("settings-file is empty");
                    Default::default()
                }
            } else {
                warn!("there was no settings-file found at the given location");
                // if file didn't exist, use default vaules
                Default::default()
            }
        };
        Ok(Self {
            config: Arc::new(cfg),
            settings: Arc::new(RwLock::new(inner)),
        })
    }

    pub fn save(&self) -> Result<()> {
        let mut file = File::create(&self.config.general.settings_path)
            .context("Failed to open settings-file for writing")?;
        serde_json::to_writer(&file, self.settings.as_ref())?;
        file.flush().map_err(|e| e.into())
    }

    pub fn motors(&self) -> MotorSettings {
        MotorSettings {
            config: self.config.clone(),
            settings: self.settings.clone(),
        }
    }

    pub fn config(&self) -> &Config {
        self.config.as_ref()
    }
}

pub fn settings() -> Result<Settings> {
    let cfg = config::config()?;
    Ok(Settings::new(cfg)?)
}
