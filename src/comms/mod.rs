use rocket::request::FromParam;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

#[derive(Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl FromParam<'_> for Axis {
    type Error = &'static str;

    fn from_param(param: &str) -> Result<Self, Self::Error> {
        match param {
            "x" => Ok(Self::X),
            "y" => Ok(Self::Y),
            "z" => Ok(Self::Z),
            _ => Err("not a valid axis, must be x, y, or z"),
        }
    }
}

#[derive(Debug)]
pub enum ControlComms<T> {
    Msg(T),
    Exit,
}

#[derive(Debug, Clone)]
pub struct OnewayAtomicF64Read(Arc<AtomicU64>);

impl OnewayAtomicF64Read {
    pub fn new(val: f64) -> Self {
        Self(Arc::new(AtomicU64::new(u64::from_ne_bytes(
            val.to_ne_bytes(),
        ))))
    }

    pub fn get_write(&self) -> OnewayAtomicF64Write {
        OnewayAtomicF64Write(Arc::clone(&self.0))
    }

    pub fn read(&self) -> f64 {
        // FIXME maybe use Ordering::Relaxed since it doesn't really matter?
        f64::from_ne_bytes(self.0.load(Ordering::Acquire).to_ne_bytes())
    }
}

#[derive(Debug, Clone)]
pub struct OnewayAtomicF64Write(Arc<AtomicU64>);

impl OnewayAtomicF64Write {
    pub fn new(val: f64) -> Self {
        Self(Arc::new(AtomicU64::new(u64::from_ne_bytes(
            val.to_ne_bytes(),
        ))))
    }

    pub fn get_read(&self) -> OnewayAtomicF64Read {
        OnewayAtomicF64Read(Arc::clone(&self.0))
    }

    pub fn write(&self, val: f64) {
        // FIXME maybe use Ordering::Relaxed since it doesn't really matter?
        self.0
            .store(u64::from_ne_bytes(val.to_ne_bytes()), Ordering::Release)
    }

    pub fn read(&self) -> f64 {
        // FIXME maybe use Ordering::Relaxed since it doesn't really matter?
        f64::from_ne_bytes(self.0.load(Ordering::Acquire).to_ne_bytes())
    }
}
