use anyhow::Result;
use revpi::revpi_from_json;

revpi_from_json!(InnerRevPi, "config.rsc");

pub struct RevPi {
    inner: InnerRevPi,
}

impl RevPi {
    pub fn new() -> Result<Self> {
        Ok(Self {
            inner: InnerRevPi::new()?,
        })
    }

    pub fn write_hotend_heat(&self, state: bool) -> Result<()> {
        self.inner.set_hotend_heating(state).map_err(|e| e.into())
    }

    pub fn write_hotend_fan(&self, state: bool) {
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
