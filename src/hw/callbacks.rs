//! Error and stop callbacks for the executor and pi threads
//!
//! This is a bit of a difficult one.
//! The pi and executor threads both need callbacks for when they encounter an
//! error. Additionally executor needs one for when executing the gcode is done,
//! but that is not too big of a problem since that is only needed when actually
//! printing.
//! Unfortunately this proves difficult due to two things:
//! 1. There is no HwCtrl thread. HwCtrl is only a struct that holds the means
//!    of communicating with the pi and executor threads.
//! 2. Ideally, the error callbacks would also stop the respectively other thread
//!    since an error means that coherent operation is possible anymore (e.g.
//!    continuing to move the motors while the pi thread is unable to read/write
//!    the hotend temperature would not be helpful)
//!
//! This means that each thread must be initialised with the means to stop the
//! other thread. But one of them has to be started first.
//! This becomes even more difficult if we consider that the executor thread
//! needs access to the pi anyways, so the pi thread must be started before
//! the executor thread
//!
//! Multiple solutions are possible:
//! 1. We could move all of the intialiationg logic, even that of the pi and
//!    executor modules, into one module and have it do all of the intialisation.
//!    This might seem like the obvious solutino but that will be difficult to
//!    maintian in the future.
//! 2. Make a seperate HwCtrl thread after all, which would only complicate things
//!    and we would still need a HwCtrl struct like we have now to be able to
//!    do some things in the thread it as called from and to have a stable api.
//! 3. Split the initialisation of the executor and pi threads. Only initialise
//!    some sort of stopper first which is only able to stop the given thread but
//!    nothing else. When this stopper is intialised the thread is not yet started
//!    i.e. there's nothing to stop yet but that's not a problem. Once we have
//!    all the stoppers we can then continue to start the threads with the respective
//!    stoppers.
//! 4. ... ?
//!
//! Of course we chose option 3 because it is the somewhat cleanest and easiest.

use std::error::Error;

pub trait ErrorCallback: Send {
    fn err<E: Error>(&self, err: E);
}

pub trait StopCallback: Send {
    fn stop(&self);
}

pub trait EStopCallback: Send {
    fn estop(&self);
}
