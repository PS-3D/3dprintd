use super::motors::Motors;
use crate::{
    comms::{Action, Axis},
    settings::Settings,
};
use anyhow::Result;
use std::{thread, time::Duration};

pub struct Executor {
    settings: Settings,
    motors: Motors,
}

impl Executor {
    pub fn new(settings: Settings, motors: Motors) -> Self {
        Self { settings, motors }
    }

    fn exec_wait(&self, time: Duration) {
        thread::sleep(time);
    }

    pub fn exec(&mut self, action: Action) -> Result<()> {
        match action {
            Action::MoveAll(m) => self.motors.move_all(&m),
            Action::ReferenceAll => self.motors.reference_all(&self.settings),
            Action::ReferenceAxis(a) => match a {
                Axis::X => self.motors.reference_x(&self.settings),
                Axis::Y => self.motors.reference_y(&self.settings),
                Axis::Z => self.motors.reference_z(&self.settings),
            },
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
