mod error;
mod pi;

pub use self::error::{ExitError, PiCtrlError, WaitTempError};
use self::pi::RevPi;
use crate::{
    comms::ControlComms,
    log::target,
    settings::Settings,
    util::{ensure_own, send_err},
};
use anyhow::{Context, Error, Result};
use crossbeam::channel::{self, Receiver, Sender, TryRecvError};
use once_cell::sync::OnceCell;
use std::{
    collections::BTreeMap,
    mem::{self, ManuallyDrop},
    sync::{
        atomic::{AtomicU16, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use tracing::debug;

type WaitTempComms = Result<(), WaitTempError>;

#[derive(Debug)]
enum InnerPiComms {
    SetHotendTarget(Option<u16>),
    SetBedTarget(Option<u16>),
    WaitHotendTarget(Sender<WaitTempComms>),
    WaitBedTarget(Sender<WaitTempComms>),
    WaitMinBedTemp(Option<u16>, Sender<WaitTempComms>),
    Stop,
    EStop,
}

type PiComms = ControlComms<InnerPiComms>;

/// Atomically stores a target temperature
///
/// This assumes that the target temp will never be 0, allowing 0 to be used
/// as the value for [`None`].
#[derive(Debug, Clone)]
struct AtomicTargetTemp(Arc<AtomicU16>);

impl AtomicTargetTemp {
    fn opt_to_u16(opt: Option<u16>) -> u16 {
        match opt {
            Some(temp) if temp > 0 => temp,
            Some(_) => panic!("given target temp must be at least 1"),
            None => 0,
        }
    }

    /// # Panics
    /// Panics if the value inside the option is 0
    pub fn new(target: Option<u16>) -> Self {
        let temp = Self::opt_to_u16(target);
        Self(Arc::new(AtomicU16::new(temp)))
    }

    pub fn load(&self) -> Option<u16> {
        match self.0.load(Ordering::Acquire) {
            temp if temp > 0 => Some(temp),
            _ => None,
        }
    }

    /// # Panics
    /// Panics if the value inside the option is 0
    pub fn store(&self, target: Option<u16>) {
        self.0.store(Self::opt_to_u16(target), Ordering::Release)
    }
}

// not implementing clone since that could lead to the pi thread being
// stopped twice due to implementing drop. though this makes intuitive sense
// anyways, one pithread, one control for it
//
// hotend_target and bed_target should only be used for reading, setting it is
// done via pi_send. This is done so the pi thread can notify any threads waiting
// for a certain target temp to be reached that the target changed and cannot be
// reached if that is the case.
#[derive(Debug)]
pub struct PiCtrl {
    settings: Settings,
    pi_handle: ManuallyDrop<JoinHandle<()>>,
    pi_send: Sender<PiComms>,
    hotend_target: AtomicTargetTemp,
    bed_target: AtomicTargetTemp,
}

impl PiCtrl {
    fn new(
        settings: Settings,
        pi_handle: JoinHandle<()>,
        pi_send: Sender<PiComms>,
        hotend_target: AtomicTargetTemp,
        bed_target: AtomicTargetTemp,
    ) -> Self {
        Self {
            settings,
            pi_handle: ManuallyDrop::new(pi_handle),
            pi_send,
            hotend_target,
            bed_target,
        }
    }

    pub fn hotend_target(&self) -> Option<u16> {
        self.hotend_target.load()
    }

    pub fn bed_target(&self) -> Option<u16> {
        self.bed_target.load()
    }

    pub fn try_set_hotend_target(&self, target: Option<u16>) -> Result<(), PiCtrlError> {
        if let Some(temp) = target.as_ref() {
            let cfg = &self.settings.config().hotend;
            ensure_own!(
                cfg.lower_limit <= *temp && *temp <= cfg.upper_limit,
                PiCtrlError::TargetOutOfBounds(*temp, cfg.lower_limit, cfg.upper_limit)
            );
        }
        self.pi_send
            .send(ControlComms::Msg(InnerPiComms::SetHotendTarget(target)))
            .unwrap();
        Ok(())
    }

    fn ensure_bed_target_in_range(&self, target: &Option<u16>) -> Result<(), PiCtrlError> {
        if let Some(temp) = target.as_ref() {
            let cfg = &self.settings.config().bed;
            ensure_own!(
                cfg.lower_limit <= *temp && *temp <= cfg.upper_limit,
                PiCtrlError::TargetOutOfBounds(*temp, cfg.lower_limit, cfg.upper_limit)
            );
        }
        Ok(())
    }

    pub fn try_set_bed_target(&self, target: Option<u16>) -> Result<()> {
        self.ensure_bed_target_in_range(&target)?;
        self.pi_send
            .send(ControlComms::Msg(InnerPiComms::SetBedTarget(target)))
            .unwrap();
        Ok(())
    }

    pub fn try_wait_hotend_target(&self) -> Result<(), WaitTempError> {
        let (notify_send, notify_recv) = channel::bounded(1);
        self.pi_send
            .send(ControlComms::Msg(InnerPiComms::WaitHotendTarget(
                notify_send,
            )))
            .unwrap();
        notify_recv.recv().unwrap()
    }

    pub fn try_wait_bed_target(&self) -> Result<(), WaitTempError> {
        let (notify_send, notify_recv) = channel::bounded(1);
        self.pi_send
            .send(ControlComms::Msg(InnerPiComms::WaitBedTarget(notify_send)))
            .unwrap();
        notify_recv.recv().unwrap()
    }

    pub fn try_wait_min_bed_temp(
        &self,
        min_temp: Option<u16>,
    ) -> Result<Result<(), WaitTempError>, PiCtrlError> {
        self.ensure_bed_target_in_range(&min_temp)?;
        let (notify_send, notify_recv) = channel::bounded(1);
        self.pi_send
            .send(ControlComms::Msg(InnerPiComms::WaitMinBedTemp(
                min_temp,
                notify_send,
            )))
            .unwrap();
        Ok(notify_recv.recv().unwrap())
    }

    pub fn stop(&self) {
        self.pi_send
            .send(ControlComms::Msg(InnerPiComms::Stop))
            .unwrap()
    }

    pub fn estop(&self) {
        self.pi_send
            .send(ControlComms::Msg(InnerPiComms::EStop))
            .unwrap()
    }
}

impl Drop for PiCtrl {
    fn drop(&mut self) {
        self.pi_send.send(ControlComms::Exit).unwrap();
        // safety:
        // since we are in drop, self.pi_handle will not be used again
        unsafe { ManuallyDrop::take(&mut self.pi_handle) }
            .join()
            .unwrap();
    }
}

// can't use a set for the waiting pools because Sender doesn't implement hash
// nor eq nor ord
struct PiThreadData {
    pi: RevPi,
    hotend_target: AtomicTargetTemp,
    hotend_waiting: Vec<Sender<WaitTempComms>>,
    bed_target: AtomicTargetTemp,
    bed_waiting: Vec<Sender<WaitTempComms>>,
    bed_min_waiting: BTreeMap<Option<u16>, Sender<WaitTempComms>>,
}

impl PiThreadData {
    pub fn new() -> Result<Self> {
        Ok(Self {
            pi: RevPi::new()?,
            hotend_target: AtomicTargetTemp::new(None),
            hotend_waiting: Vec::new(),
            bed_target: AtomicTargetTemp::new(None),
            bed_waiting: Vec::new(),
            bed_min_waiting: BTreeMap::new(),
        })
    }

    pub fn get_targets(&self) -> (AtomicTargetTemp, AtomicTargetTemp) {
        (self.hotend_target.clone(), self.bed_target.clone())
    }

    #[cfg(not(feature = "dev_no_pi"))]
    pub fn update_hotend_heat(&mut self) -> Result<()> {
        // FIXME TODO
        Ok(())
    }

    #[cfg(feature = "dev_no_pi")]
    pub fn update_hotend_heat(&mut self) -> Result<()> {
        for notify_send in self.hotend_waiting.drain(..) {
            notify_send.send(Ok(())).unwrap();
        }
        Ok(())
    }

    #[cfg(not(feature = "dev_no_pi"))]
    pub fn update_bed_heat(&mut self) -> Result<()> {
        // FIXME TODO
        Ok(())
    }

    #[cfg(feature = "dev_no_pi")]
    pub fn update_bed_heat(&mut self) -> Result<()> {
        for notify_send in self.bed_waiting.drain(..) {
            notify_send.send(Ok(())).unwrap();
        }
        for notify_send in mem::replace(&mut self.bed_min_waiting, BTreeMap::new()).into_values() {
            notify_send.send(Ok(())).unwrap();
        }
        Ok(())
    }

    fn notify_waiting_target_changed<I: IntoIterator<Item = Sender<WaitTempComms>>>(waiting: I) {
        for notify_send in waiting.into_iter() {
            notify_send.send(Err(WaitTempError::TargetChanged)).unwrap();
        }
    }

    fn notify_hotend_target_changed(&mut self) {
        Self::notify_waiting_target_changed(self.hotend_waiting.drain(..))
    }

    pub fn set_hotend_target(&mut self, target: Option<u16>) {
        self.hotend_target.store(target);
        self.notify_hotend_target_changed();
    }

    fn notify_bed_target_changed(&mut self) {
        Self::notify_waiting_target_changed(self.bed_waiting.drain(..));
        Self::notify_waiting_target_changed(
            mem::replace(&mut self.bed_min_waiting, BTreeMap::new()).into_values(),
        )
    }

    pub fn set_bed_target(&mut self, target: Option<u16>) {
        self.bed_target.store(target);
        self.notify_bed_target_changed();
    }

    pub fn add_hotend_waiting(&mut self, notify_send: Sender<WaitTempComms>) {
        // TODO check if actual temp is already at target
        self.hotend_waiting.push(notify_send)
    }

    pub fn add_bed_waiting(&mut self, notify_send: Sender<WaitTempComms>) {
        // TODO check if actual temp is already at target
        self.bed_waiting.push(notify_send)
    }

    pub fn add_bed_min_waiting(
        &mut self,
        min_temp: Option<u16>,
        notify_send: Sender<WaitTempComms>,
    ) {
        // TODO check if actual temp is already above given temp
        self.bed_min_waiting.insert(min_temp, notify_send);
    }

    // FIXME add atomic bool that acts like an enable/disable
    pub fn estop(&mut self) -> Result<(), Vec<Error>> {
        let mut errors = Vec::with_capacity(3);
        if let Err(e) = self.pi.write_hotend_heat(false) {
            errors.push(e.into());
        }
        if let Err(e) = self.pi.write_hotend_fan(false) {
            errors.push(e.into());
        }
        if let Err(e) = self.pi.write_bed_heat(false) {
            errors.push(e.into());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// see also [`ExitError`]
    pub fn exit(mut self) -> Result<(), ExitError> {
        let res = self.estop();
        self.set_hotend_target(None);
        self.set_bed_target(None);
        res.map_err(|e| ExitError(e))
    }
}

fn pi_loop(
    settings: Settings,
    mut data: PiThreadData,
    pi_recv: Receiver<PiComms>,
    error_send: Sender<ControlComms<Error>>,
) {
    loop {
        match pi_recv.try_recv() {
            Ok(msg) => {
                match msg {
                    ControlComms::Msg(msg) => {
                        debug!(target: target::INTERNAL, "received {:?}, executing...", msg);
                        match msg {
                            InnerPiComms::SetHotendTarget(target) => data.set_hotend_target(target),
                            InnerPiComms::SetBedTarget(target) => data.set_bed_target(target),
                            InnerPiComms::WaitHotendTarget(notify_send) => {
                                data.add_hotend_waiting(notify_send)
                            }
                            InnerPiComms::WaitBedTarget(notify_send) => {
                                data.add_bed_waiting(notify_send)
                            }
                            InnerPiComms::WaitMinBedTemp(min_temp, notify_send) => {
                                data.add_bed_min_waiting(min_temp, notify_send)
                            }
                            InnerPiComms::Stop => {
                                data.set_hotend_target(None);
                                data.set_bed_target(None);
                            }
                            InnerPiComms::EStop => {
                                if let Err(es) = data.estop() {
                                    for e in es {
                                        error_send.send(ControlComms::Msg(e)).unwrap();
                                    }
                                }
                            }
                        }
                    }
                    ControlComms::Exit => {
                        debug!(target: target::INTERNAL, "received exit, exiting...");
                        send_err!(data.exit(), error_send);
                        break;
                    }
                }
                // continue to see if there are more messages in the channel
                continue;
            }
            Err(e) => match e {
                TryRecvError::Disconnected => panic!("pi channel unexepectedly disconnected"),
                TryRecvError::Empty => (),
            },
        }
        thread::sleep(Duration::from_millis(settings.config().pi.check_interval));
        send_err!(data.update_hotend_heat(), error_send);
        send_err!(data.update_bed_heat(), error_send);
    }
}

#[derive(Debug, Clone)]
pub struct PiStopper {
    unstarted_data: OnceCell<Receiver<PiComms>>,
    pi_send: Sender<PiComms>,
}

impl PiStopper {
    pub(self) fn init() -> Self {
        let (pi_send, pi_recv) = channel::unbounded();
        Self {
            unstarted_data: OnceCell::with_value(pi_recv),
            pi_send,
        }
    }

    pub(self) fn start_pi(
        &mut self,
        settings: Settings,
        error_send: Sender<ControlComms<Error>>,
    ) -> Result<PiCtrl> {
        let pi_recv = self
            .unstarted_data
            .take()
            .expect("can't start pi thread twice");
        let pi_send = self.pi_send.clone();
        let pi_thread_data = PiThreadData::new()?;
        let (hotend_target, bed_target) = pi_thread_data.get_targets();
        let settings_clone = settings.clone();
        let handle = thread::Builder::new()
            .name(String::from("pi"))
            .spawn(move || pi_loop(settings_clone, pi_thread_data, pi_recv, error_send))
            .context("Creating the pi thread failed")?;
        Ok(PiCtrl::new(
            settings,
            handle,
            pi_send,
            hotend_target,
            bed_target,
        ))
    }

    fn started(&self) -> &Sender<PiComms> {
        // FIXME don't panic, no reason why the thread not being started
        // should inhibit stopping it
        if self.unstarted_data.get().is_none() {
            &self.pi_send
        } else {
            panic!("executor thread is not yet started")
        }
    }

    pub fn stop(&self) {
        self.started()
            .send(ControlComms::Msg(InnerPiComms::Stop))
            .unwrap()
    }

    pub fn estop(&self) {
        self.started()
            .send(ControlComms::Msg(InnerPiComms::EStop))
            .unwrap()
    }
}

pub fn init() -> (
    PiStopper,
    impl FnOnce(Settings, Sender<ControlComms<Error>>) -> Result<PiCtrl>,
) {
    let pi_stopper = PiStopper::init();
    let mut pi_stopper_clone = pi_stopper.clone();
    (pi_stopper, move |settings, error_send| {
        pi_stopper_clone.start_pi(settings, error_send)
    })
}
