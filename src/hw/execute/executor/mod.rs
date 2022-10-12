use super::{super::comms::Action, motors::Motors};
use crate::{
    comms::{Axis, OnewayAtomicF64Read, OnewayAtomicF64Write},
    settings::Settings,
};
use anyhow::Result;
use std::{thread, time::Duration};

pub struct Executor {
    settings: Settings,
    motors: Motors,
    z_hotend_location: OnewayAtomicF64Write,
}

impl Executor {
    pub fn new(settings: Settings, motors: Motors) -> (Self, OnewayAtomicF64Read) {
        let z_hotend_location_write =
            OnewayAtomicF64Write::new(-(settings.config().motors.z.limit as f64));
        let z_hotend_location_read = z_hotend_location_write.get_read();
        (
            Self {
                settings,
                motors,
                z_hotend_location: z_hotend_location_write,
            },
            z_hotend_location_read,
        )
    }

    fn exec_wait(&self, time: Duration) {
        thread::sleep(time);
    }

    fn exec_reference_z_hotend(&self) -> Result<()> {
        self.z_hotend_location.write(self.motors.z_pos_mm());
        Ok(())
    }

    pub fn exec(&mut self, action: Action) -> Result<()> {
        match action {
            Action::MoveAll(m) => self.motors.move_all(&m, self.settings.config()),
            Action::ReferenceAll => self.motors.reference_all(&self.settings),
            Action::ReferenceAxis(a) => match a {
                Axis::X => self.motors.reference_x(&self.settings),
                Axis::Y => self.motors.reference_y(&self.settings),
                Axis::Z => self.motors.reference_z(&self.settings),
            },
            Action::ReferenceZHotend => self.exec_reference_z_hotend(),
            Action::HotendTemp(t) => todo!(),
            Action::BedTemp(t) => todo!(),
            // FIXME add timeouts for temp waits, otherwise it might wait forever
            //       or add error checking
            Action::WaitHotendTemp(t) => todo!(),
            Action::WaitBedTemp(t) => todo!(),
            Action::Wait(d) => Ok(self.exec_wait(d)),
        }
    }
}
