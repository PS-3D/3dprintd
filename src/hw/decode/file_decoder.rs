use super::{
    inner_decoder::Decoder as InnerDecoder,
    parser::{GCode, Parser},
    Action, Decoder, DecoderError, State,
};
use crate::settings::Settings;
use anyhow::Result;
use std::{collections::VecDeque, fs::File, io::Error as IoError, path::PathBuf};

const BUFSIZE: usize = 512;

pub struct FileDecoder {
    parser: Parser<File>,
    buf: VecDeque<(Action, GCode)>,
    decoder: InnerDecoder,
}

impl FileDecoder {
    /// # Errors
    /// Returns an [`io::Error`][IoError] if opening the given path fails
    pub fn with_state(settings: Settings, state: State, path: PathBuf) -> Result<Self, IoError> {
        Ok(Self {
            parser: Parser::new(File::open(&path)?, path),
            buf: VecDeque::with_capacity(BUFSIZE),
            decoder: InnerDecoder::with_state(settings, state),
        })
    }

    /// # Errors
    /// Returns an [`io::Error`][IoError] if opening the given path fails
    pub fn with_state_and_file(
        settings: Settings,
        state: State,
        file: File,
        path: PathBuf,
    ) -> Self {
        Self {
            parser: Parser::new(file, path),
            buf: VecDeque::with_capacity(BUFSIZE),
            decoder: InnerDecoder::with_state(settings, state),
        }
    }

    fn check_buffer(&mut self) -> Result<(), DecoderError> {
        if self.buf.is_empty() {
            // TODO opitmise
            for codes in self.parser.try_n(BUFSIZE).into_iter() {
                for code in codes.into_iter() {
                    if let Some(actions) = self.decoder.decode(code)? {
                        self.buf.extend(actions);
                    }
                }
            }
        }
        Ok(())
    }
}

impl Decoder for FileDecoder {
    fn state(self) -> State {
        self.decoder.state()
    }
}

impl Iterator for FileDecoder {
    type Item = Result<(Action, GCode), DecoderError>;

    /// Tries to get the next (Action, GCode) tuple and if necessary reads it from
    /// the file/stream and decodes it
    ///
    fn next(&mut self) -> Option<Self::Item> {
        if let Err(e) = self.check_buffer() {
            return Some(Err(e));
        }
        self.buf.pop_front().map(|a| Ok(a))
    }
}
