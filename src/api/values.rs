use crate::comms::ControlComms;
use anyhow::Error;
use crossbeam::channel::Receiver;
use indexmap::IndexMap;
use log::error;
use serde::Serialize;
use std::{
    cmp,
    sync::{Arc, Mutex, MutexGuard},
    thread::{self, JoinHandle},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug)]
struct ErrorWrap {
    time: SystemTime,
    error: Error,
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    id: u64,
    time: u64,
    text: String,
}

impl From<(&u64, &ErrorWrap)> for ApiError {
    fn from((id, wrap): (&u64, &ErrorWrap)) -> Self {
        // calculate unix timestamp
        let time = wrap
            .time
            .duration_since(UNIX_EPOCH)
            .expect("error somehow occured before epoch")
            .as_secs();
        let text = format!("{}", wrap.error);
        Self {
            id: *id,
            time,
            text,
        }
    }
}

#[derive(Debug, Default)]
struct InnerErrors {
    errors: IndexMap<u64, ErrorWrap>,
    next_id: u64,
}

#[derive(Debug, Clone)]
pub struct Errors(Arc<Mutex<InnerErrors>>);

impl Errors {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(InnerErrors::default())))
    }

    fn insert_inner(&self, inner: &mut MutexGuard<InnerErrors>, error: Error) -> u64 {
        let id = inner.next_id;
        inner.errors.insert(
            id,
            ErrorWrap {
                time: SystemTime::now(),
                error,
            },
        );
        inner.next_id += 1;
        id
    }

    // FIXME add limit to errors
    pub fn insert(&self, error: Error) -> u64 {
        let mut inner = self.0.lock().unwrap();
        self.insert_inner(&mut inner, error)
    }

    pub fn insert_get(&self, error: Error) -> ApiError {
        let mut inner = self.0.lock().unwrap();
        let id = self.insert_inner(&mut inner, error);
        // shouldn't panic, we just inserted the error and didn't open the lock
        inner
            .errors
            .get(&id)
            .map(|wrap| (&id, wrap).into())
            .unwrap()
    }

    pub fn get_last(&self) -> Option<ApiError> {
        let inner = self.0.lock().unwrap();
        inner.errors.last().map(Into::into)
    }

    pub fn get_page(&self, page: usize, size: usize) -> Vec<ApiError> {
        let inner = self.0.lock().unwrap();
        let start = page * size;
        if inner.errors.is_empty() || start >= inner.errors.len() {
            Vec::new()
        } else {
            let len = cmp::min(size, inner.errors.len() - start);
            inner
                .errors
                .iter()
                .skip(start)
                .take(len)
                .map(Into::into)
                .collect()
        }
    }

    pub fn get(&self, id: u64) -> Option<ApiError> {
        let inner = self.0.lock().unwrap();
        inner.errors.get(&id).map(|wrap| (&id, wrap).into())
    }
}

pub fn start(error_recv: Receiver<ControlComms<Error>>) -> (JoinHandle<()>, Errors) {
    let errors = Errors::new();
    let errors_clone = errors.clone();
    let handle = thread::spawn(move || loop {
        match error_recv.recv().unwrap() {
            ControlComms::Msg(e) => {
                error!("{}", e);
                errors.insert(e);
            }
            ControlComms::Exit => break,
        }
    });
    (handle, errors_clone)
}
