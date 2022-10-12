mod decoder;
pub mod error;

use self::decoder::Decoder as InnerDecoder;
use super::{
    comms::{Action, DecoderComms, ExecutorGCodeComms, GCodeSpan},
    state::StateError,
};
use crate::{
    comms::{ControlComms, OnewayAtomicF64Read},
    settings::Settings,
};
use anyhow::{Context, Result};
use crossbeam::{
    channel::{self, Receiver, Sender},
    select,
};
use gcode::Span;
use std::{
    collections::VecDeque,
    fs::File,
    io::Read,
    path::PathBuf,
    sync::{Arc, RwLock},
    thread::{self, JoinHandle},
};

// FIXME make buffer only parts of the gcode from the file so we don't need
// to store all of it in memory and can print arbitrarily large files
#[derive(Debug)]
struct DecoderStateData {
    // FIXME move this buffer out of state and directly into the decode thread
    // this is only really possible if we incrementally parse the gcode file
    // we can then store the location in the file here
    pub buf: VecDeque<(Action, Span)>,
    pub path: PathBuf,
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

    pub fn print(&mut self, actions: VecDeque<(Action, Span)>, path: PathBuf) {
        match self.state {
            InnerDecoderState::Printing => panic!("can't print, already printing"),
            InnerDecoderState::Paused => panic!("can't print, is paused"),
            InnerDecoderState::Stopped => {
                self.state = InnerDecoderState::Printing;
                self.data = Some(DecoderStateData { buf: actions, path })
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

// should only be locked in this order:
// 1. state
// 2. decoder
//
// if decoder is locked for writing, state must be locked for writing as well
#[derive(Debug, Clone)]
pub struct DecoderCtrl {
    state: Arc<RwLock<DecoderState>>,
    decoder: Arc<RwLock<InnerDecoder>>,
    decoder_send: Sender<ControlComms<DecoderComms>>,
}

impl DecoderCtrl {
    fn send_decoder_state_change(&self, msg: DecoderComms) {
        self.decoder_send.send(ControlComms::Msg(msg)).unwrap();
    }

    pub fn print(
        &self,
        path: PathBuf,
        executor_gcode_send: Sender<ExecutorGCodeComms>,
    ) -> Result<()> {
        let mut state = self.state.write().unwrap();
        let mut file = File::open(&path).context("Failed to open gcode file")?;
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
        state.print(actions, path);
        self.send_decoder_state_change(DecoderComms::Started(executor_gcode_send));
        Ok(())
    }

    pub fn stop(&self) {
        let mut state = self.state.write().unwrap();
        let mut decoder = self.decoder.write().unwrap();
        state.stop();
        decoder.reset();
        self.send_decoder_state_change(DecoderComms::Stopped);
    }

    pub fn play(&self) {
        self.state.write().unwrap().play();
        self.send_decoder_state_change(DecoderComms::Played);
    }

    pub fn pause(&self) {
        self.state.write().unwrap().pause();
        self.send_decoder_state_change(DecoderComms::Paused);
    }

    pub fn exit(&self) {
        self.decoder_send.send(ControlComms::Exit).unwrap();
    }
}

struct DecoderThread {
    state: Arc<RwLock<DecoderState>>,
    decoder: Arc<RwLock<InnerDecoder>>,
}

impl DecoderThread {
    fn new(settings: Settings, z_hotend_location: OnewayAtomicF64Read) -> Self {
        Self {
            state: Arc::new(RwLock::new(DecoderState::new())),
            decoder: Arc::new(RwLock::new(InnerDecoder::new(settings, z_hotend_location))),
        }
    }

    fn get_ctrl(&self, decoder_send: Sender<ControlComms<DecoderComms>>) -> DecoderCtrl {
        DecoderCtrl {
            state: Arc::clone(&self.state),
            decoder: Arc::clone(&self.decoder),
            decoder_send,
        }
    }

    fn try_get_next(&self) -> Result<Option<(Action, GCodeSpan)>, StateError> {
        let mut state = self.state.write().unwrap();
        let state_data = state.data_mut().ok_or(StateError::NotPrinting)?;
        let next = state_data.buf.pop_front().map(|(action, span)| {
            (
                action,
                GCodeSpan {
                    path: state_data.path.clone(),
                    inner: span,
                },
            )
        });
        // if there is nothing left in the buffer, we need to stop
        if next.is_none() {
            state.stop();
        }
        Ok(next)
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
    let mut executor_gcode_send_opt: Option<Sender<ExecutorGCodeComms>> = None;
    macro_rules! handle_ctrl_msg {
        ($msg:expr) => {{
            match $msg {
                ControlComms::Msg(m) => match m {
                    DecoderComms::Started(executor_gcode_send) => {
                        executor_gcode_send_opt = Some(executor_gcode_send)
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
                buf = decoder.try_get_next().ok().flatten();
                // if buf is still None, that's the end of the gcode
                if buf.is_none() {
                    // send an Exit message in the gcode channel, signalling the end
                    // of the gcode
                    // ignore possibly disconnected receiver since that would mean
                    // the exec thread got stopped but we will do that anyways
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
) -> Result<(JoinHandle<()>, DecoderCtrl)> {
    let (decoder_send, decoder_recv) = channel::unbounded();
    let decoder_thread = DecoderThread::new(settings, z_hotend_location);
    let decoder_ctrl = decoder_thread.get_ctrl(decoder_send);
    let handle = thread::Builder::new()
        .name(String::from("decoder"))
        .spawn(move || decoder_loop(decoder_thread, decoder_recv))
        .context("Creating the decoder thread failed")?;
    Ok((handle, decoder_ctrl))
}
