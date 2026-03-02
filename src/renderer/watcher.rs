use crate::errors::Result;
use anyhow::Context;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{self, Receiver};

pub fn start_shader_watcher(shader_crate: &Path) -> Result<(RecommendedWatcher, Receiver<()>)> {
    let (tx, rx) = mpsc::channel::<()>();
    let mut watcher = RecommendedWatcher::new(
        move |result| match result {
            Ok(_) => {
                let _ = tx.send(());
            }
            Err(error) => {
                tracing::warn!(error = %error, "shader watcher event error");
            }
        },
        Config::default(),
    )
    .context("failed to create shader watcher")?;

    watcher
        .watch(shader_crate, RecursiveMode::Recursive)
        .with_context(|| format!("failed to watch shader crate {}", shader_crate.display()))?;

    Ok((watcher, rx))
}
