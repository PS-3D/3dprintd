mod control;
mod decoder;
pub mod error;
mod parser;

pub use self::{
    control::DecoderCtrl,
    parser::{GCode, GCodeSpan, ParserError, ParsingError},
};
use self::{decoder::Decoder as InnerDecoder, parser::Parser};
use super::{
    comms::{Action, ExecutorGCodeComms},
    GCodeError,
};
use crate::{
    comms::{ControlComms, OnewayAtomicF64Read},
    log::target,
    settings::Settings,
    util::send_err,
};
use anyhow::{Context, Error, Result};
use crossbeam::{
    channel::{self, Receiver, Sender},
    select,
};
use std::{
    collections::VecDeque,
    fs::File,
    path::PathBuf,
    thread::{self, JoinHandle},
};
use thiserror::Error;
use tracing::{debug, error};

enum DecoderComms {
    Start(Sender<ExecutorGCodeComms>, File, PathBuf),
    Stop,
    Pause,
    Play,
}

const BUFSIZE: usize = 512;

#[derive(Debug)]
struct DecoderStateData {
    pub parser: Parser<File>,
    pub buf: VecDeque<(Action, GCode)>,
}

#[derive(Debug)]
enum InnerDecoderState {
    Printing,
    Paused,
    Stopped,
}

#[derive(Debug)]
struct DecoderState {
    state: InnerDecoderState,
    data: Option<DecoderStateData>,
}

impl DecoderState {
    pub fn new() -> Self {
        Self {
            state: InnerDecoderState::Stopped,
            data: None,
        }
    }

    pub fn start(&mut self, parser: Parser<File>) {
        match self.state {
            InnerDecoderState::Printing => panic!("can't print, already printing"),
            InnerDecoderState::Paused => panic!("can't print, is paused"),
            InnerDecoderState::Stopped => {
                self.state = InnerDecoderState::Printing;
                self.data = Some(DecoderStateData {
                    parser,
                    buf: VecDeque::new(),
                })
            }
        }
    }

    pub fn stop(&mut self) {
        self.state = InnerDecoderState::Stopped;
        self.data = None;
    }

    pub fn play(&mut self) {
        match self.state {
            InnerDecoderState::Printing => (),
            InnerDecoderState::Paused => self.state = InnerDecoderState::Printing,
            InnerDecoderState::Stopped => panic!("can't play, is stopped"),
        }
    }

    pub fn pause(&mut self) {
        match self.state {
            InnerDecoderState::Printing => self.state = InnerDecoderState::Paused,
            InnerDecoderState::Paused => (),
            InnerDecoderState::Stopped => panic!("can't pause, is stopped"),
        }
    }

    pub fn data(&self) -> Option<&DecoderStateData> {
        self.data.as_ref()
    }

    pub fn data_mut(&mut self) -> Option<&mut DecoderStateData> {
        self.data.as_mut()
    }
}

#[derive(Debug, Error)]
pub enum DecoderError {
    #[error("Error while parsing: {}", .0)]
    ParserError(#[from] ParserError),
    #[error("Error while decoding: {}", .0)]
    GCodeError(#[from] GCodeError),
}

struct Decoder {
    state: DecoderState,
    decoder: InnerDecoder,
}

impl Decoder {
    pub fn new(settings: Settings, z_hotend_location: OnewayAtomicF64Read) -> Self {
        Self {
            state: DecoderState::new(),
            decoder: InnerDecoder::new(settings, z_hotend_location),
        }
    }

    fn check_buffer(&mut self) -> Result<(), DecoderError> {
        let state_data = self.state.data_mut().unwrap();
        if state_data.buf.is_empty() {
            // TODO opitmise
            for codes in state_data.parser.try_n(BUFSIZE).into_iter() {
                for code in codes.into_iter() {
                    if let Some(actions) = self.decoder.decode(code)? {
                        state_data.buf.extend(actions);
                    }
                }
            }
        }
        Ok(())
    }

    /// Tries to get the next (Action, GCode) tuple and if necessary reads it from
    /// the file/stream and decodes it
    ///
    /// # Panics
    /// if the state isn't printing
    pub fn next(&mut self) -> Option<Result<(Action, GCode), DecoderError>> {
        if let Err(e) = self.check_buffer() {
            return Some(Err(e));
        }
        let state_data = self.state.data_mut().unwrap();
        state_data.buf.pop_front().map(|a| Ok(a))
    }

    pub fn start(&mut self, file: File, path: PathBuf) {
        self.state.start(Parser::new(file, path))
    }

    pub fn stop(&mut self) {
        self.state.stop();
        self.decoder.reset()
    }

    pub fn play(&mut self) {
        self.state.play()
    }

    pub fn pause(&mut self) {
        self.state.pause()
    }
}

fn decoder_loop(
    mut decoder: Decoder,
    decoder_recv: Receiver<ControlComms<DecoderComms>>,
    error_send: Sender<ControlComms<Error>>,
) {
    // we need this buf so that we always have something to send
    // if the status changes but we weren't notified of it yet and need to
    // send a message to the executor.
    // this basically adds one more element to the buffered messages between
    // the decoder and executor
    let mut buf = None;
    // we need the channel here even tho we have it in DecoderThread.decoder, because
    // we can't really lock decoder everytime we want to send a message to the
    // executor thread. we have to do that with state already
    let mut executor_gcode_send_opt: Option<Sender<ExecutorGCodeComms>> = None;
    macro_rules! handle_ctrl_msg {
        ($msg:expr) => {{
            match $msg {
                ControlComms::Msg(m) => match m {
                    DecoderComms::Start(executor_gcode_send, file, path) => {
                        decoder.start(file, path);
                        executor_gcode_send_opt = Some(executor_gcode_send);
                    }
                    DecoderComms::Stop => {
                        decoder.stop();
                        executor_gcode_send_opt = None;
                        buf = None;
                    }
                    // if we played or paused, we don't really need to do anything
                    _ => continue,
                },
                ControlComms::Exit => {
                    debug!(target: target::INTERNAL, "received exit, exiting...");
                    break;
                }
            }
        }};
    }
    loop {
        if let Some(executor_gcode_send) = executor_gcode_send_opt.as_ref() {
            if buf.is_none() {
                match decoder.next() {
                    Some(Ok(a)) => buf = Some(a),
                    // FIXME stop on error and change HwCtrl state
                    Some(Err(e)) => {
                        error!(target: target::PUBLIC, "{}", e);
                        error_send.send(ControlComms::Msg(e.into())).unwrap()
                    }
                    _ => (),
                }
                // if buf is still None, that's the end of the gcode
                if buf.is_none() {
                    // send an Exit message in the gcode channel, signalling the end
                    // of the gcode
                    // ignore possibly disconnected receiver since that would mean
                    // the exec thread got stopped but we will do that anyways
                    // FIXME change HwCtrl state when gcode is done
                    let _ = executor_gcode_send.send(ControlComms::Exit);
                    executor_gcode_send_opt = None;
                    // we don't need to try the rest of the loop, buf is none anyways
                    continue;
                }
            }
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
        } else {
            handle_ctrl_msg!(decoder_recv.recv().unwrap())
        }
    }
}

/// Starts the decode thread
pub fn start(
    settings: Settings,
    z_hotend_location: OnewayAtomicF64Read,
    error_send: Sender<ControlComms<Error>>,
) -> Result<(JoinHandle<()>, DecoderCtrl)> {
    let (decoder_send, decoder_recv) = channel::unbounded();
    let decoder_thread = Decoder::new(settings, z_hotend_location);
    let decoder_ctrl = DecoderCtrl::new(decoder_send);
    let handle = thread::Builder::new()
        .name(String::from("decoder"))
        .spawn(move || decoder_loop(decoder_thread, decoder_recv, error_send))
        .context("Creating the decoder thread failed")?;
    Ok((handle, decoder_ctrl))
}
