use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StateError {
    #[error("printer isn't printing")]
    NotPrinting,
    #[error("printer isn't paused")]
    NotPaused,
    #[error("printer isn't stopped")]
    NotStopped,
    #[error("printer is printing")]
    Printing,
    #[error("printer is paused")]
    Paused,
    #[error("printer is stopped")]
    Stopped,
}

#[derive(Debug)]
pub enum StateInfo {
    Printing(PathBuf),
    Paused(PathBuf),
    Stopped,
}

#[derive(Debug)]
pub struct PrintingState {
    // path of the file that is currently being printed
    pub path: PathBuf,
}

#[derive(Debug)]
enum InnerState {
    Printing,
    Paused,
    Stopped,
}

#[derive(Debug)]
pub struct State {
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
                StateInfo::$variant(printing_state.path.clone())
            }};
        }
        match self.state {
            InnerState::Printing => construct_printing_paused!(Printing),
            InnerState::Paused => construct_printing_paused!(Paused),
            InnerState::Stopped => StateInfo::Stopped,
        }
    }

    pub fn print(&mut self, path: PathBuf) {
        match self.state {
            InnerState::Printing => panic!("can't print, already printing"),
            InnerState::Paused => panic!("can't print, is paused"),
            InnerState::Stopped => {
                self.state = InnerState::Printing;
                self.printing_state = Some(PrintingState { path });
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
