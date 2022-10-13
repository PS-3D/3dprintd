use super::{super::comms::ExecutorGCodeComms, DecoderComms};
use crate::comms::ControlComms;
use anyhow::{Context, Result};
use crossbeam::channel::Sender;
use std::{fs::File, path::PathBuf};

#[derive(Debug, Clone)]
pub struct DecoderCtrl {
    decoder_send: Sender<ControlComms<DecoderComms>>,
}

impl DecoderCtrl {
    pub(super) fn new(decoder_send: Sender<ControlComms<DecoderComms>>) -> Self {
        Self { decoder_send }
    }

    fn send_decoder_state_change(&self, msg: DecoderComms) {
        self.decoder_send.send(ControlComms::Msg(msg)).unwrap();
    }

    pub fn print(
        &self,
        path: PathBuf,
        executor_gcode_send: Sender<ExecutorGCodeComms>,
    ) -> Result<()> {
        let file = File::open(&path).context("Failed to open gcode file")?;
        self.send_decoder_state_change(DecoderComms::Start(executor_gcode_send, file, path));
        Ok(())
    }

    pub fn stop(&self) {
        self.send_decoder_state_change(DecoderComms::Stop);
    }

    pub fn play(&self) {
        self.send_decoder_state_change(DecoderComms::Play);
    }

    pub fn pause(&self) {
        self.send_decoder_state_change(DecoderComms::Pause);
    }

    pub fn exit(&self) {
        self.decoder_send.send(ControlComms::Exit).unwrap();
    }
}
