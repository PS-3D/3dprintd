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
struct AxisMotorSettings {
    reference_speed: Option<u32>,
    reference_accel_decel: Option<u32>,
    reference_jerk: Option<u32>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct MotorSettings {
    x: AxisMotorSettings,
    y: AxisMotorSettings,
    z: AxisMotorSettings,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct InnerSettings {
    motors: MotorSettings,
}

#[derive(Debug, Clone)]
pub struct Settings {
    config: Arc<Config>,
    settings: Arc<RwLock<InnerSettings>>,
}

macro_rules! get_option {
    ($self:expr, $($ss:ident).+, $($cs:ident).+) => {{
        $self.settings.read().unwrap().$($ss).*.unwrap_or($self.config.$($cs).*)
    }};
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

    fn save(&self) -> Result<()> {
        let mut file = File::open(&self.config.general.settings_path)?;
        serde_json::to_writer(&file, self.settings.as_ref())?;
        file.flush().map_err(|e| e.into())
    }

    pub fn get_motor_x_reference_speed(&self) -> u32 {
        get_option!(
            self,
            motors.x.reference_speed,
            motors.x.default_reference_speed
        )
    }

    pub fn get_motor_y_reference_speed(&self) -> u32 {
        get_option!(
            self,
            motors.y.reference_speed,
            motors.y.default_reference_speed
        )
    }

    pub fn get_motor_z_reference_speed(&self) -> u32 {
        get_option!(
            self,
            motors.z.reference_speed,
            motors.z.default_reference_speed
        )
    }

    pub fn get_motor_x_reference_accel_decel(&self) -> u32 {
        get_option!(
            self,
            motors.x.reference_accel_decel,
            motors.x.default_reference_accel
        )
    }

    pub fn get_motor_y_reference_accel_decel(&self) -> u32 {
        get_option!(
            self,
            motors.y.reference_accel_decel,
            motors.y.default_reference_accel
        )
    }

    pub fn get_motor_z_reference_accel_decel(&self) -> u32 {
        get_option!(
            self,
            motors.z.reference_accel_decel,
            motors.z.default_reference_accel
        )
    }

    pub fn get_motor_x_reference_jerk(&self) -> u32 {
        get_option!(
            self,
            motors.x.reference_jerk,
            motors.x.default_reference_jerk
        )
    }

    pub fn get_motor_y_reference_jerk(&self) -> u32 {
        get_option!(
            self,
            motors.y.reference_jerk,
            motors.y.default_reference_jerk
        )
    }

    pub fn get_motor_z_reference_jerk(&self) -> u32 {
        get_option!(
            self,
            motors.z.reference_jerk,
            motors.z.default_reference_jerk
        )
    }

    pub fn config(&self) -> &Config {
        self.config.as_ref()
    }
}

pub fn settings() -> Result<Settings> {
    let cfg = config::config()?;
    Ok(Settings::new(cfg)?)
}
