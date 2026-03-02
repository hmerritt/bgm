use crate::errors::Result;
use anyhow::{bail, Context};
use std::ptr;
use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, POINT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, FindWindowW, GetCursorPos, GetSystemMetrics, SendMessageTimeoutW,
    SetParent, SetWindowPos, ShowWindow, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
    SM_YVIRTUALSCREEN, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_NOZORDER, SW_HIDE, SW_SHOW,
};

#[derive(Debug, Clone, Copy)]
pub struct DesktopRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub fn attach_window_to_desktop(hwnd: HWND) -> Result<()> {
    unsafe {
        let progman = find_window("Progman", None)?;
        spawn_workerw(progman);
        let workerw = find_workerw().context("failed to find WorkerW host window")?;
        let parent = SetParent(hwnd, workerw);
        if parent.is_null() {
            bail!("SetParent failed for desktop render window");
        }
    }
    Ok(())
}

pub fn show_desktop_window(hwnd: HWND, visible: bool) {
    unsafe {
        ShowWindow(hwnd, if visible { SW_SHOW } else { SW_HIDE });
    }
}

pub fn place_window_over_virtual_desktop(hwnd: HWND) -> Result<DesktopRect> {
    let rect = virtual_desktop_rect();
    if rect.width <= 0 || rect.height <= 0 {
        bail!("virtual desktop has invalid bounds");
    }

    unsafe {
        let ok = SetWindowPos(
            hwnd,
            ptr::null_mut(),
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOACTIVATE,
        );
        if ok == 0 {
            bail!("SetWindowPos failed for desktop render window");
        }
    }

    Ok(rect)
}

pub fn virtual_desktop_rect() -> DesktopRect {
    unsafe {
        DesktopRect {
            x: GetSystemMetrics(SM_XVIRTUALSCREEN),
            y: GetSystemMetrics(SM_YVIRTUALSCREEN),
            width: GetSystemMetrics(SM_CXVIRTUALSCREEN),
            height: GetSystemMetrics(SM_CYVIRTUALSCREEN),
        }
    }
}

pub fn virtual_cursor_position() -> Option<(f32, f32)> {
    let mut point: POINT = POINT { x: 0, y: 0 };
    let ok = unsafe { GetCursorPos(&mut point) };
    if ok == 0 {
        return None;
    }
    let rect = virtual_desktop_rect();
    Some(((point.x - rect.x) as f32, (point.y - rect.y) as f32))
}

unsafe fn spawn_workerw(progman: HWND) {
    let mut _result: usize = 0;
    let _ = SendMessageTimeoutW(progman, 0x052C, 0, 0, 0, 1000, &mut _result as *mut usize);
}

unsafe fn find_window(class_name: &str, window_name: Option<&str>) -> Result<HWND> {
    let class = wide_null(class_name);
    let name_buf = window_name.map(wide_null);
    let name_ptr = name_buf.as_ref().map(|n| n.as_ptr()).unwrap_or(ptr::null());
    let hwnd = FindWindowW(class.as_ptr(), name_ptr);
    if hwnd.is_null() {
        bail!("FindWindowW could not find class {class_name}");
    }
    Ok(hwnd)
}

unsafe fn find_workerw() -> Option<HWND> {
    let mut workerw: HWND = ptr::null_mut();
    let _ = EnumWindows(
        Some(enum_windows_cb),
        (&mut workerw as *mut HWND).cast::<core::ffi::c_void>() as LPARAM,
    );
    if workerw.is_null() {
        None
    } else {
        Some(workerw)
    }
}

unsafe extern "system" fn enum_windows_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let shell_class = wide_null("SHELLDLL_DefView");
    let shell = FindWindowExW(hwnd, ptr::null_mut(), shell_class.as_ptr(), ptr::null());
    if shell.is_null() {
        return 1;
    }

    let worker_class = wide_null("WorkerW");
    let worker = FindWindowExW(ptr::null_mut(), hwnd, worker_class.as_ptr(), ptr::null());
    if !worker.is_null() {
        let out = lparam as *mut HWND;
        if !out.is_null() {
            *out = worker;
        }
        return 0;
    }

    1
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
