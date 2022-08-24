mod config;

pub use self::config::Config;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json;
use std::{
    fs::File,
    io::Write,
    sync::{Arc, RwLock},
};

#[derive(Debug, Serialize, Deserialize)]
struct AxisMotorSettings {
    reference_speed: Option<u32>,
    reference_accel_decel: Option<u32>,
    reference_jerk: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MotorSettings {
    x: AxisMotorSettings,
    y: AxisMotorSettings,
    z: AxisMotorSettings,
}

#[derive(Debug, Serialize, Deserialize)]
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
        let file = File::open(&cfg.general.settings_path)?;
        let inner: InnerSettings = serde_json::from_reader(file)?;
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
