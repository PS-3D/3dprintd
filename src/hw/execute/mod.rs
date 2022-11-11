mod control;
mod executor;
mod motors;

pub use self::control::{ExecutorCtrl, OutOfBoundsError};
use self::{
    super::{
        comms::EStopComms,
        decode::State as DecoderState,
        decode::{Decoder, FileDecoder, ThreadedDecoder},
        pi::PiCtrl,
    },
    executor::Executor,
    motors::Motors,
};
use crate::{
    comms::{Axis, ControlComms, ReferenceRunOptParameters},
    log::target,
    settings::Settings,
    util::send_err,
};
use anyhow::{Context, Error, Result};
use crossbeam::{
    channel::{self, Receiver, Sender, TryRecvError},
    select,
};
use std::{
    fs::File,
    mem,
    path::PathBuf,
    sync::{
        atomic::{AtomicI32, AtomicUsize, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
};
use tracing::{debug, info};

enum ExecutorCtrlComms {
    /// sends the already open file, the path to that file (for error messages)
    /// and an atomic that the currently executed line will be written into by
    /// the executor
    Print(File, PathBuf, Arc<AtomicUsize>),
    Stop,
    Play,
    Pause,
}

#[derive(Debug, Clone, Default)]
pub(self) struct SharedRawPos {
    x: Arc<AtomicI32>,
    y: Arc<AtomicI32>,
    z: Arc<AtomicI32>,
}

enum ExecutorManualComms {
    ReferenceAxis(Axis, ReferenceRunOptParameters),
    ReferenceZAxisHotend,
}

enum InnerState {
    Printing,
    Paused,
    Stopped(DecoderState),
}

pub(self) struct PrintingData {
    pub decoder: ThreadedDecoder<FileDecoder>,
    pub line: Arc<AtomicUsize>,
}

struct State {
    inner: InnerState,
    data: Option<PrintingData>,
}

impl State {
    pub fn new(z_hotend_location: f64) -> Self {
        Self {
            inner: InnerState::Stopped(DecoderState::new(z_hotend_location)),
            data: None,
        }
    }

    pub fn print(&mut self, settings: Settings, file: File, path: PathBuf, line: Arc<AtomicUsize>) {
        match &self.inner {
            InnerState::Stopped(_) => {
                let decoder_state = match mem::replace(&mut self.inner, InnerState::Printing) {
                    InnerState::Stopped(ds) => ds,
                    _ => unreachable!(),
                };
                let decoder = ThreadedDecoder::new(FileDecoder::with_state_and_file(
                    settings,
                    decoder_state,
                    file,
                    path,
                ))
                .expect("starting the decoder thread failed");
                self.data = Some(PrintingData { decoder, line })
            }
            _ => panic!("printer is already printing/paused"),
        }
    }

    pub fn stop(&mut self) {
        match self.inner {
            InnerState::Stopped(_) => (),
            _ => {
                let mut decoder_state = self.data.take().unwrap().decoder.state();
                decoder_state.reset();
                self.inner = InnerState::Stopped(decoder_state);
            }
        }
    }

    pub fn play(&mut self) {
        match self.inner {
            InnerState::Stopped(_) => panic!("can't continue, printer is stopped"),
            _ => self.inner = InnerState::Printing,
        }
    }

    pub fn pause(&mut self) {
        match self.inner {
            InnerState::Stopped(_) => panic!("can't continue, printer is stopped"),
            _ => self.inner = InnerState::Paused,
        }
    }

    pub fn decoder_mut(&mut self) -> Option<&mut PrintingData> {
        self.data.as_mut()
    }

    pub fn decoder_state_mut(&mut self) -> &mut DecoderState {
        match &mut self.inner {
            InnerState::Stopped(decoder_state) => decoder_state,
            _ => panic!("can't read decoder state, printer isn't stopped"),
        }
    }
}

fn executor_loop(
    settings: Settings,
    mut exec: Executor,
    executor_ctrl_recv: Receiver<ControlComms<ExecutorCtrlComms>>,
    executor_manual_recv: Receiver<ExecutorManualComms>,
    shared_z_pos_raw: Arc<AtomicI32>,
    error_send: Sender<ControlComms<Error>>,
) {
    let mut state = State::new(-(settings.config().motors.z.limit as f64));
    // has to be macro so break will work
    macro_rules! handle_ctrl_msg {
        ($msg:expr) => {{
            match $msg {
                ControlComms::Msg(c) => match c {
                    ExecutorCtrlComms::Print(file, path, line) => {
                        debug!(target: target::INTERNAL, "executor thread starting print");
                        state.print(settings.clone(), file, path, line);
                    }
                    ExecutorCtrlComms::Stop => {
                        debug!(target: target::INTERNAL, "executor thread stopping");
                        state.stop();
                    }
                    ExecutorCtrlComms::Play => {
                        debug!(target: target::INTERNAL, "executor thread contiuing");
                        state.play();
                    }
                    ExecutorCtrlComms::Pause => {
                        debug!(target: target::INTERNAL, "executor thread pausing");
                        state.pause();
                    }
                },
                ControlComms::Exit => {
                    debug!(target: target::INTERNAL, "received exit, exiting...");
                    break;
                }
            };
        }};
    }
    loop {
        // try to receive a message from the controlchannel, since it has priority
        match executor_ctrl_recv.try_recv() {
            Ok(msg) => {
                handle_ctrl_msg!(msg);
                // in case there is another control message, we want to receive
                // it
                continue;
            }
            Err(e) => match e {
                TryRecvError::Empty => (),
                TryRecvError::Disconnected => {
                    panic!("executor_ctrl_recv unexpectedly disconnected")
                }
            },
        }
        if let Some(printing_data) = state.decoder_mut() {
            if let Some(res) = printing_data.decoder.next() {
                let (action, code) = match res {
                    Ok(t) => t,
                    Err(e) => {
                        // FIXME alert hwctrl of error
                        error_send.send(ControlComms::Msg(e.into())).unwrap();
                        state.stop();
                        continue;
                    }
                };
                // FIXME maybe use Ordering::Relaxed since it doesn't really matter?
                printing_data
                    .line
                    .store(code.span().line(), Ordering::Release);
                debug!(target: target::PUBLIC, "Executing {}", code);
                send_err!(exec.exec(action), error_send)
            } else {
                // FIXME alert hwctrl of finish
                state.stop();
            }
        } else {
            // TODO run manual movement commands through decoder somehow
            select! {
                recv(executor_ctrl_recv) -> msg => handle_ctrl_msg!(msg.unwrap()),
                recv(executor_manual_recv) -> msg => match msg.unwrap() {
                    ExecutorManualComms::ReferenceAxis(axis, parameters) => send_err!(exec.exec_reference_axis(axis, parameters), error_send),
                    ExecutorManualComms::ReferenceZAxisHotend => {
                        let pos_steps = shared_z_pos_raw.load(Ordering::Acquire);
                        let pos_mm = settings.config().motors.z.steps_to_mm(pos_steps);
                        state.decoder_state_mut().set_z_hotend_location(pos_mm)
                    }
                }
            }
        }
    }
}

pub fn start(
    settings: Settings,
    pi_ctrl: PiCtrl,
    estop_recv: Receiver<ControlComms<EStopComms>>,
    error_send: Sender<ControlComms<Error>>,
) -> Result<(JoinHandle<()>, JoinHandle<()>, ExecutorCtrl)> {
    let (executor_ctrl_send, executor_ctrl_recv) = channel::unbounded();
    let (executor_manual_send, executor_manual_recv) = channel::unbounded();
    let (setup_send, setup_recv) = channel::bounded(1);
    let settings_clone = settings.clone();
    let shared_pos = SharedRawPos::default();
    let shared_pos_clone = shared_pos.clone();
    // do it this way all in the executorhread because we can't send motors between
    // threads. We then send the result of the setup via the above channel.
    // the setup is all in a function so we can use the ? operator for convenience
    let executor_handle = thread::Builder::new()
        .name(String::from("executor"))
        .spawn(move || {
            fn setup(
                settings: &Settings,
                estop_recv: Receiver<ControlComms<EStopComms>>,
                shared_pos: SharedRawPos,
            ) -> Result<(Motors, JoinHandle<()>)> {
                let (mut motors, mut estop) = Motors::new(settings.clone(), shared_pos)?;
                let estop_handle = thread::Builder::new()
                    .name(String::from("estop"))
                    .spawn(move || {
                        loop {
                            match estop_recv
                                .recv()
                                .expect("estop channel was unexpectedly closed")
                            {
                                // if there's an IO error writing, it's probably a good plan to
                                // panic
                                ControlComms::Msg(m) => match m {
                                    EStopComms::EStop => {
                                        info!(target: target::PUBLIC, "executing estop");
                                        estop.estop(2000).unwrap()
                                    }
                                },
                                ControlComms::Exit => {
                                    debug!(target: target::INTERNAL, "received exit, exiting...");
                                    break;
                                }
                            }
                        }
                    })
                    .context("Creating the estop thread failed")?;
                motors.init()?;
                Ok((motors, estop_handle))
            }
            let shared_z_pos_raw = Arc::clone(&shared_pos_clone.z);
            match setup(&settings, estop_recv, shared_pos_clone) {
                Ok((motors, estop_handle)) => {
                    let executor = Executor::new(settings.clone(), motors, pi_ctrl);
                    setup_send.send(Ok(estop_handle)).unwrap();
                    executor_loop(
                        settings,
                        executor,
                        executor_ctrl_recv,
                        executor_manual_recv,
                        shared_z_pos_raw,
                        error_send,
                    );
                }
                Err(e) => {
                    setup_send.send(Err(e)).unwrap();
                }
            }
        })
        .context("Creating the executor thread failed")?;
    let estop_handle = setup_recv.recv().unwrap()?;
    Ok((
        executor_handle,
        estop_handle,
        ExecutorCtrl::new(
            settings_clone,
            executor_ctrl_send,
            executor_manual_send,
            shared_pos,
        ),
    ))
}
