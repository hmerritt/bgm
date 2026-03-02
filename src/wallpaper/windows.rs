use crate::errors::Result;
use crate::wallpaper::WallpaperBackend;
use anyhow::{bail, Context};
use std::iter;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SPI_SETDESKWALLPAPER,
};

#[derive(Debug, Default)]
pub struct WindowsWallpaperBackend;

impl WindowsWallpaperBackend {
    pub fn new() -> Self {
        Self
    }
}

impl WallpaperBackend for WindowsWallpaperBackend {
    fn set_wallpaper(&self, path: &Path) -> Result<()> {
        enforce_fill_style().context("failed to enforce wallpaper Fill style")?;

        let absolute = path
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", path.display()))?;

        let wide: Vec<u16> = absolute
            .as_os_str()
            .encode_wide()
            .chain(iter::once(0))
            .collect();

        let ok = unsafe {
            SystemParametersInfoW(
                SPI_SETDESKWALLPAPER,
                0,
                wide.as_ptr() as *mut _,
                SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
            )
        };
        if ok == 0 {
            bail!("SystemParametersInfoW(SPI_SETDESKWALLPAPER) failed");
        }
        Ok(())
    }
}

fn enforce_fill_style() -> Result<()> {
    let mut key: HKEY = ptr::null_mut();
    let subkey = wide_null("Control Panel\\Desktop");
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut key,
        )
    };
    if status != ERROR_SUCCESS {
        bail!("RegOpenKeyExW failed with status {status}");
    }

    let close_result = (|| -> Result<()> {
        write_reg_sz(key, "WallpaperStyle", "10")?;
        write_reg_sz(key, "TileWallpaper", "0")?;
        Ok(())
    })();

    let close_status = unsafe { RegCloseKey(key) };
    if close_status != ERROR_SUCCESS {
        bail!("RegCloseKey failed with status {close_status}");
    }

    close_result
}

fn write_reg_sz(key: HKEY, value_name: &str, value: &str) -> Result<()> {
    let value_name_w = wide_null(value_name);
    let value_w = wide_null(value);
    let data_len = (value_w.len() * size_of::<u16>()) as u32;

    let status = unsafe {
        RegSetValueExW(
            key,
            value_name_w.as_ptr(),
            0,
            REG_SZ,
            value_w.as_ptr() as *const u8,
            data_len,
        )
    };

    if status != ERROR_SUCCESS {
        bail!("RegSetValueExW({value_name}) failed with status {status}");
    }
    Ok(())
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(iter::once(0)).collect()
}
