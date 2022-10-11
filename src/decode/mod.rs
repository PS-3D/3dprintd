mod decoder;
pub mod error;

use self::{decoder::Decoder as InnerDecoder, error::StateError};
use crate::{
    comms::{Action, ControlComms, DecoderComms, ExecutorCtrl, ExecutorGCodeComms, GCodeSpan},
    settings::Settings,
    util::ensure_own,
};
use anyhow::{ensure, Context, Result};
use crossbeam::{
    channel::{self, Receiver, Sender},
    select,
};
use gcode::Span;
use serde::Serialize;
use std::{
    collections::VecDeque,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, RwLock,
    },
    thread::{self, JoinHandle},
};

#[derive(Debug, Serialize)]
pub struct PrintingStateInfo {
    pub path: PathBuf,
    pub line: usize,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum StateInfo {
    Printing(PrintingStateInfo),
    Paused(PrintingStateInfo),
    Stopped,
}

// FIXME make buffer only parts of the gcode from the file so we don't need
// to store all of it in memory and can print arbitrarily large files
#[derive(Debug)]
struct PrintingState {
    // FIXME move this buffer out of state and directly into the decode thread
    // this is only really possible if we incrementally parse the gcode file
    // we can then store the location in the file here
    pub buf: VecDeque<(Action, Span)>,
    // path of the file that is currently being printed
    pub path: PathBuf,
    pub executor_gcode_send: Sender<ExecutorGCodeComms>,
    // line of gcode that the executor is currently executing
    // the other end of that atomic is in the executor thread
    // this one should be read-only
    pub current_line: Arc<AtomicUsize>,
}

#[derive(Debug)]
enum InnerState {
    Printing,
    Paused,
    Stopped,
}

#[derive(Debug)]
struct State {
    state: InnerState,
    printing_state: Option<PrintingState>,
}

impl State {
    pub fn new() -> Self {
        Self {
            state: InnerState::Stopped,
            printing_state: None,
        }
    }

    pub fn info(&self) -> StateInfo {
        macro_rules! construct_printing_paused {
            ($variant:ident) => {{
                let printing_state = self.printing_state.as_ref().unwrap();
                StateInfo::$variant(PrintingStateInfo {
                    path: printing_state.path.clone(),
                    line: printing_state.current_line.load(Ordering::Acquire),
                })
            }};
        }
        match self.state {
            InnerState::Printing => construct_printing_paused!(Printing),
            InnerState::Paused => construct_printing_paused!(Paused),
            InnerState::Stopped => StateInfo::Stopped,
        }
    }

    pub fn print(
        &mut self,
        actions: VecDeque<(Action, Span)>,
        path: PathBuf,
        executor_gcode_send: Sender<ExecutorGCodeComms>,
    ) -> &Arc<AtomicUsize> {
        match self.state {
            InnerState::Printing => panic!("can't print, already printing"),
            InnerState::Paused => panic!("can't print, is paused"),
            InnerState::Stopped => {
                self.state = InnerState::Printing;
                self.printing_state = Some(PrintingState {
                    buf: actions,
                    path,
                    executor_gcode_send,
                    current_line: Arc::new(AtomicUsize::new(1)),
                });
                &self.printing_state.as_ref().unwrap().current_line
            }
        }
    }

    pub fn stop(&mut self) {
        self.state = InnerState::Stopped;
        self.printing_state = None;
    }

    pub fn play(&mut self) {
        match self.state {
            InnerState::Printing => (),
            InnerState::Paused => self.state = InnerState::Printing,
            InnerState::Stopped => panic!("can't play, is stopped"),
        }
    }

    pub fn pause(&mut self) {
        match self.state {
            InnerState::Printing => self.state = InnerState::Paused,
            InnerState::Paused => (),
            InnerState::Stopped => panic!("can't pause, is stopped"),
        }
    }

    pub fn is_printing(&self) -> bool {
        match self.state {
            InnerState::Printing => true,
            _ => false,
        }
    }

    pub fn is_stopped(&self) -> bool {
        match self.state {
            InnerState::Stopped => true,
            _ => false,
        }
    }

    pub fn is_paused(&self) -> bool {
        match self.state {
            InnerState::Paused => true,
            _ => false,
        }
    }

    pub fn printing_state_mut(&mut self) -> Option<&mut PrintingState> {
        self.printing_state.as_mut()
    }

    pub fn printing_state(&self) -> Option<&PrintingState> {
        self.printing_state.as_ref()
    }
}

// should only be locked in this order:
// 1. state
// 2. decoder
//
// if decoder is locked for writing, state must be locked for writing as well
#[derive(Debug, Clone)]
pub struct DecoderCtrl {
    state: Arc<RwLock<State>>,
    decoder: Arc<RwLock<InnerDecoder>>,
    decoder_send: Sender<ControlComms<DecoderComms>>,
    executor_ctrl_send: Sender<ControlComms<ExecutorCtrl>>,
    executor_manual_send: Sender<Action>,
}

impl DecoderCtrl {
    fn send_decoder_state_change(&self, msg: DecoderComms) {
        self.decoder_send.send(ControlComms::Msg(msg)).unwrap();
    }

    pub fn state_info(&self) -> StateInfo {
        self.state.read().unwrap().info()
    }

    /// Tries to start a print
    ///
    /// Should only be used by the API thread, not the decoder thread
    pub fn try_print<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut state = self.state.write().unwrap();
        ensure!(state.is_stopped(), StateError::NotStopped);
        let mut file = File::open(path.as_ref()).context("Failed to open gcode file")?;
        let mut s = String::new();
        file.read_to_string(&mut s)?;
        // FIXME treat parsing errors
        let iter = gcode::parse(s.as_str());
        let mut actions = VecDeque::with_capacity(iter.size_hint().0);
        let mut decoder = self.decoder.write().unwrap();
        for code in iter {
            if let Some(dq) = decoder.decode(code)? {
                actions.extend(dq);
            }
        }
        let (executor_gcode_send, executor_gcode_recv) = channel::bounded(16);
        let cur_line = state.print(actions, path.as_ref().to_path_buf(), executor_gcode_send);
        self.executor_ctrl_send
            .send(ControlComms::Msg(ExecutorCtrl::GCode(
                executor_gcode_recv,
                Arc::clone(cur_line),
            )))
            .unwrap();
        self.send_decoder_state_change(DecoderComms::Started);
        Ok(())
    }

    pub fn stop(&self) {
        let mut state = self.state.write().unwrap();
        let mut decoder = self.decoder.write().unwrap();
        state.stop();
        decoder.reset();
        self.executor_ctrl_send
            .send(ControlComms::Msg(ExecutorCtrl::Manual))
            .unwrap();
        self.send_decoder_state_change(DecoderComms::Stopped);
    }

    pub fn try_play(&self) -> Result<(), StateError> {
        let mut state = self.state.write().unwrap();
        ensure_own!(!state.is_stopped(), StateError::Stopped);
        state.play();
        self.send_decoder_state_change(DecoderComms::Played);
        Ok(())
    }

    /// Tries to pause a print
    ///
    /// Should only be used by the API thread, not the decoder thread
    pub fn try_pause(&self) -> Result<(), StateError> {
        let mut state = self.state.write().unwrap();
        ensure_own!(!state.is_stopped(), StateError::Stopped);
        state.pause();
        self.send_decoder_state_change(DecoderComms::Paused);
        Ok(())
    }

    pub fn exit(&self) {
        self.decoder_send.send(ControlComms::Exit).unwrap();
    }
}

struct DecoderThread {
    state: Arc<RwLock<State>>,
    decoder: Arc<RwLock<InnerDecoder>>,
    executor_manual_send: Sender<Action>,
}

impl DecoderThread {
    fn new(settings: Settings, executor_manual_send: Sender<Action>) -> Self {
        Self {
            state: Arc::new(RwLock::new(State::new())),
            decoder: Arc::new(RwLock::new(InnerDecoder::new(settings))),
            executor_manual_send,
        }
    }

    fn get_ctrl(
        &self,
        decoder_send: Sender<ControlComms<DecoderComms>>,
        executor_ctrl_send: Sender<ControlComms<ExecutorCtrl>>,
    ) -> DecoderCtrl {
        DecoderCtrl {
            state: Arc::clone(&self.state),
            decoder: Arc::clone(&self.decoder),
            decoder_send,
            executor_ctrl_send,
            executor_manual_send: self.executor_manual_send.clone(),
        }
    }

    fn try_get_next(&self) -> Result<(Action, GCodeSpan), StateError> {
        let mut state = self.state.write().unwrap();
        let print_state = state.printing_state_mut().ok_or(StateError::NotPrinting)?;
        // can't panic because there should always be something in the buffer,
        // if there is one
        let (action, span) = print_state.buf.pop_front().unwrap();
        let path = print_state.path.clone();
        // ensure there is something in the buffer:
        if print_state.buf.is_empty() {
            // send an Exit message in the gcode channel, signalling the end
            // of the gcode
            print_state
                .executor_gcode_send
                .send(ControlComms::Exit)
                .unwrap();
            state.stop();
        }
        Ok((action, GCodeSpan { path, inner: span }))
    }

    fn try_get_exec_gcode_send(&self) -> Option<Sender<ExecutorGCodeComms>> {
        let state = self.state.read().unwrap();
        Some(state.printing_state()?.executor_gcode_send.clone())
    }
}

fn decoder_loop(decoder: DecoderThread, decoder_recv: Receiver<ControlComms<DecoderComms>>) {
    // we need this buf so that we always have something to send
    // if the status changes but we weren't notified of it yet and need to
    // send a message to the executor.
    // this basically adds one more element to the buffered messages between
    // the decoder and executor
    let mut buf = None;
    // we need the channel here even tho we have it in DecoderThread.decoder, because
    // we can't really lock decoder everytime we want to send a message to the
    // executor thread. we have to do that with state already
    let mut executor_gcode_send_opt = None;
    macro_rules! handle_ctrl_msg {
        ($msg:expr) => {{
            match $msg {
                ControlComms::Msg(m) => match m {
                    DecoderComms::Started => {
                        executor_gcode_send_opt = decoder.try_get_exec_gcode_send()
                    }
                    DecoderComms::Stopped => {
                        executor_gcode_send_opt = None;
                        buf = None;
                    }
                    // if we played or paused, we don't really need to do anything
                    _ => continue,
                },
                ControlComms::Exit => break,
            }
        }};
    }
    loop {
        if let Some(executor_gcode_send) = executor_gcode_send_opt.as_ref() {
            if buf.is_none() {
                buf = decoder.try_get_next().ok();
            }
            if buf.is_some() {
                select! {
                    recv(decoder_recv) -> msg => handle_ctrl_msg!(msg.unwrap()),
                    // the take().unwrap() can't fail, we wouldn't be in here if
                    // the buf were None
                    send(executor_gcode_send, ControlComms::Msg(buf.take().unwrap())) -> res => {
                        // if res is an error, it means that the other end of the
                        // gcode channel was disconnected, meaning we stopped printing
                        // and the other end of the channel was closed before
                        // the notificaction reached us
                        if res.is_err() {
                            executor_gcode_send_opt = None;
                            buf = None;
                        }
                    }
                }
            }
        } else {
            handle_ctrl_msg!(decoder_recv.recv().unwrap())
        }
    }
}

/// Starts the decode thread
pub fn start(
    settings: Settings,
    executor_ctrl_send: Sender<ControlComms<ExecutorCtrl>>,
    executor_manual_send: Sender<Action>,
) -> (JoinHandle<()>, DecoderCtrl) {
    let (decoder_send, decoder_recv) = channel::unbounded();
    let decoder_thread = DecoderThread::new(settings, executor_manual_send);
    let decoder_ctrl = decoder_thread.get_ctrl(decoder_send, executor_ctrl_send);
    let handle = thread::spawn(move || decoder_loop(decoder_thread, decoder_recv));
    (handle, decoder_ctrl)
}
