mod cache;
mod config;
mod errors;
mod image_pipeline;
mod logging;
mod rotation;
mod scheduler;
mod sources;
mod state;
mod wallpaper;

use crate::cache::CacheManager;
use crate::config::load_from_path;
use crate::errors::Result;
use crate::rotation::RotationManager;
use crate::scheduler::{Scheduler, SchedulerEvent};
use crate::sources::{build_sources, ImageCandidate, ImageSource, Origin};
use crate::state::{PersistedState, StateStore};
use anyhow::Context;
use std::collections::HashSet;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    let config_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("bgm.hcl"));

    let config = load_from_path(&config_path)?;
    logging::init(&config.log_level);
    info!(path = %config_path.display(), "loaded config");

    let cache = Arc::new(CacheManager::new(&config)?);
    if let Err(error) = cache.cleanup() {
        warn!(error = %error, "cache cleanup failed");
    }

    let mut sources = build_sources(&config, cache.clone())?;
    let backend = wallpaper::default_backend();
    let state_store = StateStore::new(config.state_file.clone());

    let persisted_state = match state_store.load() {
        Ok(state) => state,
        Err(error) => {
            warn!(error = %error, "failed to load state, starting fresh");
            PersistedState::default()
        }
    };

    let mut candidates = refresh_all_sources(&mut sources).await?;
    let mut rotation = RotationManager::new();
    rotation.rebuild_pool(candidates.clone());
    rotation.restore_state(&persisted_state);

    let mut last_image_id = persisted_state.last_image_id.clone();
    if let Some(next_id) = try_switch_once(&mut rotation, cache.as_ref(), &*backend, &config).await? {
        last_image_id = Some(next_id);
        persist_state(&state_store, &rotation, last_image_id.clone())?;
    }

    let mut scheduler = Scheduler::new(config.timer, config.remote_update_timer);
    info!("bgm is running");

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("ctrl-c received, stopping bgm");
                persist_state(&state_store, &rotation, last_image_id.clone())?;
                break;
            }
            event = scheduler.next_event() => {
                match event {
                    SchedulerEvent::SwitchImage => {
                        match try_switch_once(&mut rotation, cache.as_ref(), &*backend, &config).await {
                            Ok(Some(next_id)) => {
                                last_image_id = Some(next_id);
                                if let Err(error) = persist_state(&state_store, &rotation, last_image_id.clone()) {
                                    warn!(error = %error, "failed to persist state after wallpaper switch");
                                }
                            }
                            Ok(None) => {
                                warn!("no image available for switch");
                            }
                            Err(error) => {
                                warn!(error = %error, "failed to switch wallpaper");
                            }
                        }
                    }
                    SchedulerEvent::RefreshRemote => {
                        match refresh_all_sources(&mut sources).await {
                            Ok(updated) => {
                                candidates = updated;
                                rotation.rebuild_pool(candidates.clone());
                                info!(pool_size = rotation.pool_size(), "refresh complete");
                            }
                            Err(error) => warn!(error = %error, "source refresh failed"),
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

async fn refresh_all_sources(sources: &mut [Box<dyn ImageSource>]) -> Result<Vec<ImageCandidate>> {
    let mut candidates = Vec::new();
    let mut unique = HashSet::new();

    for source in sources.iter_mut() {
        match source.refresh().await {
            Ok(items) => {
                info!(source = source.name(), count = items.len(), "source refresh");
                for item in items {
                    if unique.insert(item.id.clone()) {
                        candidates.push(item);
                    }
                }
            }
            Err(error) => {
                warn!(source = source.name(), error = %error, "source refresh failed");
            }
        }
    }

    candidates.sort_by(|a, b| {
        let a_key = a
            .mtime
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let b_key = b
            .mtime
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        b_key
            .cmp(&a_key)
            .then_with(|| a.local_path.cmp(&b.local_path))
    });

    info!(count = candidates.len(), "total merged candidates");
    Ok(candidates)
}

async fn try_switch_once(
    rotation: &mut RotationManager,
    cache: &CacheManager,
    backend: &dyn wallpaper::WallpaperBackend,
    config: &config::BgmConfig,
) -> Result<Option<String>> {
    if rotation.pool_size() == 0 {
        return Ok(None);
    }

    let candidate = match rotation.next() {
        Some(candidate) => candidate,
        None => return Ok(None),
    };

    let screen = backend.screen_spec().context("failed to resolve screen size")?;
    let processed = image_pipeline::prepare_for_screen(
        &candidate.local_path,
        screen,
        cache,
        config.image_format,
        config.jpeg_quality,
    )
    .with_context(|| format!("failed to process {}", candidate.local_path.display()))?;

    backend
        .set_wallpaper(&processed)
        .with_context(|| format!("failed to set wallpaper {}", processed.display()))?;

    info!(
        id = %candidate.id,
        source = %origin_name(candidate.origin),
        input = %candidate.local_path.display(),
        output = %processed.display(),
        "wallpaper updated"
    );
    Ok(Some(candidate.id))
}

fn origin_name(origin: Origin) -> &'static str {
    match origin {
        Origin::File => "file",
        Origin::Directory => "directory",
        Origin::Rss => "rss",
    }
}

fn persist_state(
    state_store: &StateStore,
    rotation: &RotationManager,
    last_image_id: Option<String>,
) -> Result<()> {
    let mut persisted = rotation.export_state();
    persisted.last_image_id = last_image_id;
    state_store.save(&persisted)?;
    Ok(())
}
