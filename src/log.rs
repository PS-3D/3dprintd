use crate::APP_NAME;
use tracing::{Level, Metadata};
use tracing_subscriber::{self, filter, layer::SubscriberExt, util::SubscriberInitExt};

fn filter_modules(metadata: &Metadata<'_>) -> bool {
    if let Some(module_path) = metadata.module_path() {
        module_path.starts_with(APP_NAME)
    } else {
        metadata.target().starts_with(APP_NAME)
    }
}

pub fn setup(level: Level) {
    tracing_subscriber::fmt()
        .with_max_level(level)
        .finish()
        .with(filter::filter_fn(filter_modules))
        .init()
}
