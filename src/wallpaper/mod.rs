use crate::errors::Result;
use std::path::Path;

#[cfg(not(windows))]
use anyhow::bail;

#[cfg(windows)]
mod windows;

#[derive(Debug, Clone, Copy)]
pub struct ScreenSpec {
    pub width: u32,
    pub height: u32,
}

pub trait WallpaperBackend: Send + Sync {
    fn set_wallpaper(&self, path: &Path) -> Result<()>;
    fn screen_spec(&self) -> Result<ScreenSpec>;
}

#[cfg(windows)]
pub fn default_backend() -> Box<dyn WallpaperBackend> {
    Box::new(windows::WindowsWallpaperBackend::new())
}

#[cfg(not(windows))]
pub fn default_backend() -> Box<dyn WallpaperBackend> {
    Box::new(UnsupportedWallpaperBackend)
}

#[cfg(not(windows))]
struct UnsupportedWallpaperBackend;

#[cfg(not(windows))]
impl WallpaperBackend for UnsupportedWallpaperBackend {
    fn set_wallpaper(&self, _path: &Path) -> Result<()> {
        bail!("this build only supports wallpaper updates on Windows")
    }

    fn screen_spec(&self) -> Result<ScreenSpec> {
        bail!("this build only supports wallpaper updates on Windows")
    }
}
