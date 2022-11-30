// makes everything a bit more clear instead of annotating the imports etc.
// isn't really that big of a deal since this flag should only be used
// in development anyways
#![cfg_attr(feature = "dev_no_pi", allow(unused_imports))]

use anyhow::{Context, Result};
use revpi::revpi_from_json;

#[cfg(not(feature = "dev_no_pi"))]
revpi_from_json!(InnerRevPi, "config.rsc");

#[cfg(not(feature = "dev_no_pi"))]
pub struct RevPi {
    inner: InnerRevPi,
}

#[cfg(not(feature = "dev_no_pi"))]
impl RevPi {
    pub fn new() -> Result<Self> {
        Ok(Self {
            inner: InnerRevPi::new().context("Opening /dev/piControl0 failed")?,
        })
    }

    pub fn write_hotend_heat(&self, state: bool) -> Result<()> {
        self.inner.set_hotend_heating(state).map_err(|e| e.into())
    }

    pub fn write_hotend_fan(&self, state: bool) -> Result<()> {
        todo!()
    }

    pub fn write_bed_heat(&self, state: bool) -> Result<()> {
        self.inner.set_bed_heating(state).map_err(|e| e.into())
    }

    pub fn read_x_endstop(&self) -> Result<bool> {
        self.inner.get_x_endstop().map_err(|e| e.into())
    }

    pub fn read_y_endstop(&self) -> Result<bool> {
        self.inner.get_y_endstop().map_err(|e| e.into())
    }

    pub fn read_z_endstop(&self) -> Result<bool> {
        self.inner.get_z_endstop().map_err(|e| e.into())
    }

    pub fn read_estop(&self) -> Result<bool> {
        self.inner.get_estop().map_err(|e| e.into())
    }

    pub fn read_hotend_temp(&self) -> f64 {
        todo!()
    }

    pub fn read_bed_temp(&self) -> f64 {
        todo!()
    }
}

#[cfg(feature = "dev_no_pi")]
pub struct RevPi {}

#[cfg(feature = "dev_no_pi")]
impl RevPi {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }

    pub fn write_hotend_heat(&self, _state: bool) -> Result<()> {
        Ok(())
    }

    pub fn write_hotend_fan(&self, _state: bool) -> Result<()> {
        Ok(())
    }

    pub fn write_bed_heat(&self, _state: bool) -> Result<()> {
        Ok(())
    }

    pub fn read_x_endstop(&self) -> Result<bool> {
        Ok(false)
    }

    pub fn read_y_endstop(&self) -> Result<bool> {
        Ok(false)
    }

    pub fn read_z_endstop(&self) -> Result<bool> {
        Ok(false)
    }

    pub fn read_estop(&self) -> Result<bool> {
        Ok(false)
    }

    pub fn read_hotend_temp(&self) -> f64 {
        42.0
    }

    pub fn read_bed_temp(&self) -> f64 {
        42.0
    }
}
