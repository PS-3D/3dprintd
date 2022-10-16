use crate::log::target;
use anyhow::Result;
use gcode::{full_parse_with_callbacks, Callbacks, GCode as InnerGCode, Mnemonic, Span, Word};
use std::{
    collections::VecDeque,
    fmt::{self, Display},
    io::{BufRead, BufReader, Error as IoError, Lines, Read},
    path::PathBuf,
    sync::Arc,
};
use thiserror::Error;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct GCodeSpan {
    path: Arc<PathBuf>,
    line: usize,
}

impl Display for GCodeSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.path.display(), self.line)
    }
}

impl GCodeSpan {
    fn new(path: Arc<PathBuf>, line: usize) -> Self {
        Self { path, line }
    }

    pub fn line(&self) -> usize {
        self.line
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

#[derive(Debug, Clone)]
pub struct GCode {
    code: InnerGCode,
    line_offset: usize,
    origin: Arc<PathBuf>,
}

impl Display for GCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} from {}:{}",
            self.code,
            self.origin.display(),
            self.code.span().line + self.line_offset + 1
        )
    }
}

impl GCode {
    fn new(code: InnerGCode, line_offset: usize, origin: Arc<PathBuf>) -> Self {
        Self {
            code,
            line_offset,
            origin,
        }
    }

    pub fn mnemonic(&self) -> Mnemonic {
        self.code.mnemonic()
    }

    pub fn major_number(&self) -> u32 {
        self.code.major_number()
    }

    pub fn minor_number(&self) -> u32 {
        self.code.minor_number()
    }

    pub fn arguments(&self) -> &[Word] {
        self.code.arguments()
    }

    pub fn span(&self) -> GCodeSpan {
        GCodeSpan {
            line: self.code.span().line + self.line_offset + 1,
            path: Arc::clone(&self.origin),
        }
    }
}

#[derive(Debug, Error)]
pub enum ParsingError {
    #[error("unknown content at {}", .0)]
    UnknownContent(GCodeSpan),
    #[error("unexpected line number at {}", .0)]
    UnexpectedLineNumber(GCodeSpan),
    #[error("argument without command at {}", .0)]
    ArgumentWithoutCommand(GCodeSpan),
    #[error("number without a letter at {}", .0)]
    NumberWithoutLetter(GCodeSpan),
    #[error("letter without a number at {}", .0)]
    LetterWithoutNumebr(GCodeSpan),
}

// at the beginning path must be Some.
// when an error occurs, path will become None and err will become Some
// when that error actually gets taken out err will become none again, but
// path will stay none signalling that an error has previously occured in the
// file that is currently being processed
#[derive(Debug)]
struct UnforgivingCallbacks {
    path: Option<Arc<PathBuf>>,
    err: Option<ParsingError>,
}

impl UnforgivingCallbacks {
    pub fn new(path: Arc<PathBuf>) -> Self {
        Self {
            path: Some(path),
            err: None,
        }
    }

    pub fn check_err(&mut self) -> Option<ParsingError> {
        self.err.take()
    }
}

macro_rules! try_set_err {
    ($self:ident, $err:ident, $span:ident) => {{
        if $self.err.is_none() {
            $self.err = Some(ParsingError::$err(GCodeSpan::new(
                $self.path.take().unwrap(),
                $span.line,
            )))
        }
    }};
}

impl Callbacks for &mut UnforgivingCallbacks {
    fn unknown_content(&mut self, _text: &str, span: Span) {
        try_set_err!(self, UnknownContent, span)
    }

    fn gcode_buffer_overflowed(
        &mut self,
        _mnemonic: gcode::Mnemonic,
        _major_number: u32,
        _minor_number: u32,
        _arguments: &[gcode::Word],
        _span: Span,
    ) {
        panic!("gcode buffer overflowed, even though it is a Vec")
    }

    fn gcode_argument_buffer_overflowed(
        &mut self,
        _mnemonic: gcode::Mnemonic,
        _major_number: u32,
        _minor_number: u32,
        _argument: gcode::Word,
    ) {
        panic!("gcode argument buffer overflowed, even though it is a Vec")
    }

    fn comment_buffer_overflow(&mut self, _comment: gcode::Comment<'_>) {
        panic!("comment buffer overflowed, even though it is a Vec")
    }

    fn unexpected_line_number(&mut self, _line_number: f32, span: Span) {
        try_set_err!(self, UnexpectedLineNumber, span)
    }

    fn argument_without_a_command(&mut self, _letter: char, _value: f32, span: Span) {
        try_set_err!(self, ArgumentWithoutCommand, span)
    }

    fn number_without_a_letter(&mut self, _value: &str, span: Span) {
        try_set_err!(self, NumberWithoutLetter, span)
    }

    fn letter_without_a_number(&mut self, _value: &str, span: Span) {
        try_set_err!(self, LetterWithoutNumebr, span)
    }
}

#[derive(Debug, Error)]
pub enum ParserError {
    #[error(transparent)]
    IoError(#[from] IoError),
    #[error(transparent)]
    ParsingError(#[from] ParsingError),
}

#[derive(Debug)]
pub struct Parser<R: Read> {
    reader: Lines<BufReader<R>>,
    next_line: usize,
    callbacks: UnforgivingCallbacks,
    path: Arc<PathBuf>,
    prev_err: bool,
}

impl<R: Read> Parser<R> {
    pub fn new(reader: R, path: PathBuf) -> Self {
        let path = Arc::new(path);
        Self {
            reader: BufReader::new(reader).lines(),
            next_line: 1,
            callbacks: UnforgivingCallbacks::new(Arc::clone(&path)),
            path,
            prev_err: false,
        }
    }

    /// Tries to parse the next n lines from the gcode file
    ///
    /// # Panics
    /// Will panic if it gets called after an error was previously thrown
    pub fn try_n(&mut self, n: usize) -> Result<VecDeque<GCode>, ParserError> {
        assert!(!self.prev_err, "error previously occured in this parser");
        // could in theory be more than n but it's bound to be ~n
        let mut codes = VecDeque::with_capacity(n);
        let line_n_start = self.next_line;
        let line_n_end = line_n_start + n;
        self.next_line = line_n_end;
        debug!(
            target: target::INTERNAL,
            "Parsing lines {} to {} of {}",
            line_n_start,
            line_n_end,
            self.path.display()
        );
        for i in line_n_start..self.next_line {
            if let Some(line) = self.reader.next() {
                let line = match line {
                    Ok(line) => line,
                    Err(e) => {
                        self.prev_err = true;
                        return Err(e.into());
                    }
                };
                codes.extend(
                    full_parse_with_callbacks(&line, &mut self.callbacks)
                        .next()
                        .unwrap()
                        .gcodes()
                        .into_iter()
                        .map(|code| GCode::new(code.clone(), i, Arc::clone(&self.path))),
                );
                if let Some(e) = self.callbacks.check_err() {
                    self.prev_err = true;
                    return Err(e.into());
                }
            } else {
                return Ok(codes);
            }
        }
        Ok(codes)
    }
}
