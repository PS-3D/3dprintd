mod decoder;
pub mod error;

use self::decoder::Decoder;
use crate::{
    comms::{Action, ControlComms, DecoderComms},
    settings::Settings,
};
use anyhow::Result;
use crossbeam::{
    channel::{Receiver, Sender},
    select,
};
use std::{
    collections::VecDeque,
    io::Read,
    thread::{self, JoinHandle},
};

// FIXME make buffer only parts of the gcode from the file so we don't need
// to store all of it in memory and can print arbitrarily large files
struct PrintingState {
    pub buf: VecDeque<Action>,
}

enum InnerState {
    Printing,
    Paused,
    Stopped,
}

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

    pub fn printing_state_mut(&mut self) -> &mut PrintingState {
        match self.state {
            InnerState::Printing => self.printing_state.as_mut().unwrap(),
            InnerState::Paused => self.printing_state.as_mut().unwrap(),
            InnerState::Stopped => panic!("can't return state, is stopped"),
        }
    }
}

struct DecoderThread {
    pub decoder: Decoder,
    pub state: State,
}

impl DecoderThread {
    pub fn new(decoder: Decoder) -> Self {
        Self {
            decoder,
            state: State::new(),
        }
    }

    pub fn handle_msg(&mut self, msg: DecoderComms) -> Result<()> {
        match msg {
            DecoderComms::Print(mut file) => {
                let mut s = String::new();
                file.read_to_string(&mut s)?;
                let iter = gcode::parse(s.as_str());
                let mut actions = VecDeque::with_capacity(iter.size_hint().0);
                for code in iter {
                    if let Some(dq) = self.decoder.decode(code)? {
                        actions.extend(dq);
                    }
                }
                self.state.print(actions);
            }
            DecoderComms::Stop => {
                self.state.stop();
                self.decoder.reset();
            }
            DecoderComms::Play => self.state.play(),
            DecoderComms::Pause => self.state.pause(),
        };
        Ok(())
    }

    fn next(&mut self) -> Action {
        let print_state = self.state.printing_state_mut();
        // can't panic because there should always be something in the buffer,
        // if there is one
        let action = print_state.buf.pop_front().unwrap();
        // ensure there is something in the buffer:
        if print_state.buf.is_empty() {
            self.state.stop();
        }
        action
    }
}

fn decoder_loop(
    settings: Settings,
    decoder_recv: Receiver<ControlComms<DecoderComms>>,
    executor_send: Sender<ControlComms<Action>>,
) {
    let mut data = DecoderThread::new(Decoder::new(settings));
    loop {
        if data.state.is_printing() {
            select! {
                recv(decoder_recv) -> msg => match msg.unwrap() {
                    // FIXME do smth with result
                    ControlComms::Msg(m) => data.handle_msg(m).unwrap(),
                    ControlComms::Exit => break,
                },
                send(executor_send, ControlComms::Msg(data.next())) -> res => res.unwrap()
            }
        } else {
            match decoder_recv.recv().unwrap() {
                // FIXME do smth with result
                ControlComms::Msg(m) => data.handle_msg(m).unwrap(),
                ControlComms::Exit => break,
            }
        }
    }
}

pub fn start(
    settings: Settings,
    decoder_recv: Receiver<ControlComms<DecoderComms>>,
    executor_send: Sender<ControlComms<Action>>,
) -> JoinHandle<()> {
    let handle = thread::spawn(move || decoder_loop(settings, decoder_recv, executor_send));
    handle
}
