use super::{super::decode::Action, motors::Motors};
use crate::{
    comms::{Axis, ReferenceRunOptParameters},
    hw::pi::PiCtrl,
    log::target,
    settings::Settings,
};
use anyhow::Result;
use std::{thread, time::Duration};
use tracing::debug;

pub struct Executor {
    settings: Settings,
    motors: Motors,
    pi_ctrl: PiCtrl,
}

impl Executor {
    pub fn new(settings: Settings, motors: Motors, pi_ctrl: PiCtrl) -> Self {
        Self {
            settings,
            motors,
            pi_ctrl,
        }
    }

    fn exec_wait(&self, time: Duration) {
        thread::sleep(time);
    }

    fn exec_hotend_target(&self, target: Option<u16>) -> Result<()> {
        // shouldn't panic because decoder should check the target
        self.pi_ctrl.try_set_hotend_target(target).unwrap();
        Ok(())
    }

    fn exec_bed_target(&self, target: Option<u16>) -> Result<()> {
        // shouldn't panic because decoder should check the target
        self.pi_ctrl.try_set_bed_target(target).unwrap();
        Ok(())
    }

    fn exec_wait_hotend_target(&self) -> Result<()> {
        // shouldn't panic because nothing else should change the target
        self.pi_ctrl.try_wait_hotend_target().unwrap();
        Ok(())
    }

    fn exec_wait_bed_target(&self) -> Result<()> {
        // shouldn't panic because nothing else should change the target
        self.pi_ctrl.try_wait_bed_target().unwrap();
        Ok(())
    }

    fn exec_wait_bed_min_temp(&self, temp: Option<u16>) -> Result<()> {
        // shouldn't panic because decoder should check the temp
        // shouldn't panic because nothing else should change the target
        self.pi_ctrl.try_wait_min_bed_temp(temp).unwrap().unwrap();
        Ok(())
    }

    pub fn exec_reference_axis(
        &mut self,
        axis: Axis,
        parameters: ReferenceRunOptParameters,
    ) -> Result<()> {
        match axis {
            Axis::X => self.motors.reference_x(&self.settings, parameters),
            Axis::Y => self.motors.reference_y(&self.settings, parameters),
            Axis::Z => self.motors.reference_z(&self.settings, parameters),
        }
    }

    pub fn exec(&mut self, action: Action) -> Result<()> {
        debug!(target: target::INTERNAL, "Executing {:?}", action);
        match action {
            Action::MoveAll(m) => self.motors.move_all(&m, self.settings.config()),
            Action::ReferenceAxis(a, params) => self.exec_reference_axis(a, params),
            Action::HotendTarget(t) => self.exec_hotend_target(t),
            Action::BedTarget(t) => self.exec_bed_target(t),
            // FIXME add timeouts for temp waits, otherwise it might wait forever
            //       or add error checking
            Action::WaitHotendTarget => self.exec_wait_hotend_target(),
            Action::WaitBedTarget => self.exec_wait_bed_target(),
            Action::WaitBedMinTemp(t) => self.exec_wait_bed_min_temp(t),
            Action::Wait(d) => Ok(self.exec_wait(d)),
        }
    }
}
