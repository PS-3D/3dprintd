use crate::APP_NAME;
use atty;
use std::io;
use tracing::{Level, Metadata};
use tracing_subscriber::{
    self, filter,
    fmt::{
        self,
        format::{Format, Pretty},
    },
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

pub mod target {
    pub const INTERNAL: &'static str = "ps3dprintd::internal";
    pub const PUBLIC: &'static str = "ps3dprintd::public";
}

fn filter_modules(metadata: &Metadata<'_>) -> bool {
    if let Some(module_path) = metadata.module_path() {
        module_path.starts_with(APP_NAME)
    } else {
        metadata.target().starts_with(APP_NAME)
    }
}

fn format_common<F, T>(format: Format<F, T>, level: Level) -> Format<F, T> {
    if level < Level::DEBUG {
        format.with_source_location(false).with_target(false)
    } else {
        format.with_thread_names(true)
    }
}

fn format_tty(level: Level) -> Format<Pretty, ()> {
    format_common(fmt::format().pretty(), level).without_time()
}

fn format_notty(level: Level) -> Format {
    format_common(fmt::format(), level).with_ansi(false)
}

pub fn setup(level: Level) {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(level)
        .with_writer(io::stderr);
    if atty::is(atty::Stream::Stderr) {
        subscriber
            .event_format(format_tty(level))
            .finish()
            .with(filter::filter_fn(filter_modules))
            .init()
    } else {
        subscriber
            .event_format(format_notty(level))
            .finish()
            .with(filter::filter_fn(filter_modules))
            .init()
    }
}
