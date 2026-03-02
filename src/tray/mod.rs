use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub enum TrayEvent {
    NextWallpaper,
    Exit,
}

#[derive(Debug)]
pub struct SessionStats {
    images_shown: AtomicU64,
    manual_skips: AtomicU64,
    started_at: Instant,
}

impl SessionStats {
    pub fn new() -> Self {
        Self {
            images_shown: AtomicU64::new(0),
            manual_skips: AtomicU64::new(0),
            started_at: Instant::now(),
        }
    }

    pub fn inc_images_shown(&self) {
        self.images_shown.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_manual_skips(&self) {
        self.manual_skips.fetch_add(1, Ordering::Relaxed);
    }

    pub fn images_shown(&self) -> u64 {
        self.images_shown.load(Ordering::Relaxed)
    }

    pub fn manual_skips(&self) -> u64 {
        self.manual_skips.load(Ordering::Relaxed)
    }

    pub fn running_duration(&self) -> Duration {
        self.started_at.elapsed()
    }
}

pub(crate) fn format_running_duration(duration: Duration) -> String {
    if duration < Duration::from_secs(60) {
        return "<1m".to_string();
    }

    let total_minutes = duration.as_secs() / 60;
    let total_hours = total_minutes / 60;
    let minutes = total_minutes % 60;

    if total_hours > 72 {
        let days = total_hours / 24;
        let hours = total_hours % 24;
        return format!("{days}d {hours}h {minutes}m");
    }

    if total_hours == 0 {
        return format!("{minutes}m");
    }

    format!("{total_hours}h {minutes}m")
}

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{spawn, try_acquire_single_instance};

#[cfg(not(windows))]
pub struct TrayController;

#[cfg(not(windows))]
impl TrayController {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(windows))]
pub struct SingleInstanceGuard;

#[cfg(not(windows))]
use crate::errors::Result;
#[cfg(not(windows))]
use std::path::PathBuf;
#[cfg(not(windows))]
use tokio::sync::mpsc::UnboundedSender;

#[cfg(not(windows))]
pub fn try_acquire_single_instance() -> Result<Option<SingleInstanceGuard>> {
    Ok(Some(SingleInstanceGuard))
}

#[cfg(not(windows))]
pub fn spawn(
    _config_path: PathBuf,
    _event_tx: UnboundedSender<TrayEvent>,
    _session_stats: std::sync::Arc<SessionStats>,
) -> Result<TrayController> {
    Ok(TrayController::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_running_duration_handles_sub_minute() {
        assert_eq!(format_running_duration(Duration::from_secs(0)), "<1m");
        assert_eq!(format_running_duration(Duration::from_secs(59)), "<1m");
    }

    #[test]
    fn format_running_duration_handles_minutes_and_hours() {
        assert_eq!(format_running_duration(Duration::from_secs(60)), "1m");
        assert_eq!(format_running_duration(Duration::from_secs(59 * 60 + 59)), "59m");
        assert_eq!(format_running_duration(Duration::from_secs(60 * 60)), "1h 0m");
        assert_eq!(
            format_running_duration(Duration::from_secs((90 * 60) as u64)),
            "1h 30m"
        );
        assert_eq!(
            format_running_duration(Duration::from_secs((72 * 60 * 60) as u64)),
            "72h 0m"
        );
    }

    #[test]
    fn format_running_duration_handles_days_over_72_hours() {
        assert_eq!(
            format_running_duration(Duration::from_secs((72 * 60 * 60 + 60) as u64)),
            "72h 1m"
        );
        assert_eq!(
            format_running_duration(Duration::from_secs((73 * 60 * 60 + 60) as u64)),
            "3d 1h 1m"
        );
        assert_eq!(
            format_running_duration(Duration::from_secs((93 * 60 * 60 + 49 * 60) as u64)),
            "3d 21h 49m"
        );
    }

    #[test]
    fn session_stats_counters_increment() {
        let stats = SessionStats::new();
        assert_eq!(stats.images_shown(), 0);
        assert_eq!(stats.manual_skips(), 0);

        stats.inc_images_shown();
        stats.inc_images_shown();
        stats.inc_manual_skips();

        assert_eq!(stats.images_shown(), 2);
        assert_eq!(stats.manual_skips(), 1);
    }
}
