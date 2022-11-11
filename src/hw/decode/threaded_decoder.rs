use super::{parser::GCode, Action, Decoder, DecoderError, State};
use anyhow::Result;
use crossbeam::channel::{self, Receiver, Select, Sender};
use std::{
    io::Error as IoError,
    marker::PhantomData,
    thread::{self, JoinHandle},
};

const BUFSIZE: usize = 32;

enum DecoderExitComms {
    Exit,
    State(Sender<State>),
}

fn decoder_loop<D: Decoder>(
    mut decoder: D,
    gcode_send: Sender<Result<(Action, GCode), DecoderError>>,
    decoder_exit_recv: Receiver<DecoderExitComms>,
) {
    let mut sel = Select::new();
    let index_gcode_send = sel.send(&gcode_send);
    let index_decoder_exit_recv = sel.recv(&decoder_exit_recv);
    loop {
        match sel.ready() {
            i if i == index_gcode_send => {
                if let Some(next) = decoder.next() {
                    gcode_send.send(next).unwrap();
                } else {
                    break;
                }
            }
            // we can just break since we recv after the loop anyways
            i if i == index_decoder_exit_recv => break,
            _ => unreachable!(),
        }
    }
    // gotta wait for the exit msg since the decoder state might still get requested
    match decoder_exit_recv.recv().unwrap() {
        DecoderExitComms::Exit => (),
        DecoderExitComms::State(state_send) => state_send.send(decoder.state()).unwrap(),
    }
}

pub struct ThreadedDecoder<D: Decoder + Send + 'static> {
    // needs to be in an Option in order to implement drop and Decoder::state
    // If Decoder::state gets called we take the handle out of the option and call
    // handle.join. Afterwards drop will still be called, but there we only send
    // the exit message if thread_handle is some, which it isn't.
    // if the decoder gets just dropped normally we move it out there and call join.
    // this ensures we don't send 2 messages to the decoder thread and then crash
    thread_handle: Option<JoinHandle<()>>,
    gcode_recv: Receiver<Result<(Action, GCode), DecoderError>>,
    decoder_exit_send: Sender<DecoderExitComms>,
    marker: PhantomData<D>,
}

impl<D: Decoder + Send + 'static> ThreadedDecoder<D> {
    /// # Errors
    /// Returns an [`io::Error`][IoError] if the creation of the thread fails
    pub fn new(decoder: D) -> Result<Self, IoError> {
        let (gcode_send, gcode_recv) = channel::bounded(BUFSIZE);
        let (decoder_exit_send, decoder_exit_recv) = channel::bounded(1);
        let thread_handle = thread::Builder::new()
            .name(String::from("decoder"))
            .spawn(move || decoder_loop(decoder, gcode_send, decoder_exit_recv))?;
        Ok(Self {
            thread_handle: Some(thread_handle),
            gcode_recv,
            decoder_exit_send,
            marker: PhantomData,
        })
    }
}

impl<D: Decoder + Send + 'static> Decoder for ThreadedDecoder<D> {
    fn state(mut self) -> State {
        let (state_send, state_recv) = channel::bounded(1);
        self.decoder_exit_send
            .send(DecoderExitComms::State(state_send))
            .unwrap();
        // take the handle out of the option in order for drop to not call this
        // again
        // see also struct definition
        self.thread_handle.take().unwrap().join().unwrap();
        state_recv.recv().unwrap()
    }
}

impl<D: Decoder + Send + 'static> Iterator for ThreadedDecoder<D> {
    type Item = Result<(Action, GCode), DecoderError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.gcode_recv.recv() {
            Ok(r) => Some(r),
            Err(_) => None,
        }
    }
}

impl<D: Decoder + Send + 'static> Drop for ThreadedDecoder<D> {
    fn drop(&mut self) {
        // if this is none, the handle was already joined in Decoder::state and
        // we don't need to stop the decoder thread anymore
        // see also struct definition
        if let Some(thread_handle) = self.thread_handle.take() {
            self.decoder_exit_send.send(DecoderExitComms::Exit).unwrap();
            thread_handle.join().unwrap();
        }
    }
}
