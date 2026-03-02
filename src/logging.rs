use std::sync::OnceLock;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::reload::Handle;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter, Registry};

static FILTER_RELOAD_HANDLE: OnceLock<Handle<EnvFilter, Registry>> = OnceLock::new();

pub fn init(default_level: &str) {
    let filter = parse_filter(default_level);
    let (filter_layer, reload_handle) = tracing_subscriber::reload::Layer::new(filter);
    let _ = FILTER_RELOAD_HANDLE.set(reload_handle);

    let _ = tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt::layer().with_target(false).compact())
        .try_init();
}

pub fn set_level(level: &str) {
    let Some(handle) = FILTER_RELOAD_HANDLE.get() else {
        return;
    };

    let _ = handle.reload(parse_filter(level));
}

fn parse_filter(level: &str) -> EnvFilter {
    EnvFilter::try_new(level.trim())
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap_or_else(|_| EnvFilter::new("info"))
}
