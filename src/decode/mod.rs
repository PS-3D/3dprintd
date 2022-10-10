mod decoder;
pub mod error;

use self::{
    decoder::Decoder as InnerDecoder,
    error::{DecoderError, StateError},
};
use crate::{
    comms::{Action, ControlComms, DecoderComms},
    settings::Settings,
    util::ensure_own,
};
use anyhow::{ensure, Context, Result};
use crossbeam::{
    channel::{Receiver, Sender},
    select,
};
use std::{
    collections::VecDeque,
    fs::File,
    io::Read,
    path::Path,
    sync::{Arc, RwLock},
    thread::{self, JoinHandle},
};

// FIXME make buffer only parts of the gcode from the file so we don't need
// to store all of it in memory and can print arbitrarily large files
#[derive(Debug)]
struct PrintingState {
    pub buf: VecDeque<Action>,
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

    pub fn print(&mut self, actions: VecDeque<Action>) {
        match self.state {
            InnerState::Printing => panic!("can't print, already printing"),
            InnerState::Paused => panic!("can't print, is paused"),
            InnerState::Stopped => {
                self.state = InnerState::Printing;
                self.printing_state = Some(PrintingState { buf: actions });
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

    pub fn printing_state_mut(&mut self) -> &mut PrintingState {
        match self.state {
            InnerState::Printing => self.printing_state.as_mut().unwrap(),
            InnerState::Paused => self.printing_state.as_mut().unwrap(),
            InnerState::Stopped => panic!("can't return state, is stopped"),
        }
    }
}

// should only be locked in this order:
// 1. state
// 2. decoder
//
// if decoder is locked for writing, state must be locked for writing as well
#[derive(Debug)]
pub struct Decoder {
    state: RwLock<State>,
    decoder: RwLock<InnerDecoder>,
}

impl Decoder {
    fn new(settings: Settings) -> Self {
        Self {
            decoder: RwLock::new(InnerDecoder::new(settings)),
            state: RwLock::new(State::new()),
        }
    }

    /// Tries to start a print
    ///
    /// Should only be used by the API thread, not the decoder thread
    pub fn try_print<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut state = self.state.write().unwrap();
        ensure!(state.is_stopped(), StateError::NotStopped);
        let mut file = File::open(path).context("Failed to open gcode file")?;
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
        state.print(actions);
        Ok(())
    }

    pub fn stop(&self) {
        let mut state = self.state.write().unwrap();
        let mut decoder = self.decoder.write().unwrap();
        state.stop();
        decoder.reset();
    }

    pub fn try_play(&self) -> Result<(), StateError> {
        let mut state = self.state.write().unwrap();
        ensure_own!(!state.is_stopped(), StateError::Stopped);
        state.play();
        Ok(())
    }

    /// Tries to pause a print
    ///
    /// Should only be used by the API thread, not the decoder thread
    pub fn try_pause(&self) -> Result<(), StateError> {
        let mut state = self.state.write().unwrap();
        ensure_own!(!state.is_stopped(), StateError::Stopped);
        state.pause();
        Ok(())
    }

    fn try_next(&self) -> Result<Action, StateError> {
        let mut state = self.state.write().unwrap();
        ensure_own!(state.is_printing(), StateError::NotPrinting);
        let print_state = state.printing_state_mut();
        // can't panic because there should always be something in the buffer,
        // if there is one
        let action = print_state.buf.pop_front().unwrap();
        // ensure there is something in the buffer:
        if print_state.buf.is_empty() {
            state.stop();
        }
        Ok(action)
    }
}

fn decoder_loop(
    decoder: Arc<Decoder>,
    decoder_recv: Receiver<ControlComms<DecoderComms>>,
    executor_send: Sender<ControlComms<Action>>,
) {
    // we need this buf so that we always have something to send
    // if the status changes but we weren't notified of it yet and need to
    // send a message to the executor.
    // this basically adds one more element to the buffered messages between
    // the decoder and executor
    let mut buf = None;
    loop {
        if buf.is_none() {
            buf = decoder.try_next().ok();
        }
        if buf.is_some() {
            select! {
                recv(decoder_recv) -> msg => match msg.unwrap() {
                    ControlComms::Msg(m) => match m {
                        DecoderComms::StateChanged => continue,
                    },
                    ControlComms::Exit => break,
                },
                // the take().unwrap() can't fail, we wouldn't be in here if
                // the buf were None
                send(executor_send, ControlComms::Msg(buf.take().unwrap())) -> res => res.unwrap()
            }
        } else {
            match decoder_recv.recv().unwrap() {
                ControlComms::Msg(m) => match m {
                    DecoderComms::StateChanged => continue,
                },
                ControlComms::Exit => break,
            }
        }
    }
}

pub fn start(
    settings: Settings,
    decoder_recv: Receiver<ControlComms<DecoderComms>>,
    executor_send: Sender<ControlComms<Action>>,
) -> (JoinHandle<()>, Arc<Decoder>) {
    let decoder = Arc::new(Decoder::new(settings));
    let decoder_clone = decoder.clone();
    let handle = thread::spawn(move || decoder_loop(decoder, decoder_recv, executor_send));
    (handle, decoder_clone)
}
