use crate::errors::Result;
use crate::installer::SquirrelEvent;
use crate::installer::StartupRegistrationStatus;
use crate::version::BINARY_FILENAME;
use anyhow::Context;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn handle(event: SquirrelEvent) -> Result<bool> {
    match event {
        SquirrelEvent::Install | SquirrelEvent::Updated => {
            create_startup_and_start_menu_shortcuts()?;
            Ok(true)
        }
        SquirrelEvent::Uninstall => {
            remove_startup_and_start_menu_shortcuts()?;
            Ok(true)
        }
        SquirrelEvent::Obsolete => Ok(true),
        SquirrelEvent::Firstrun => Ok(false),
    }
}

pub fn ensure_startup_registered() -> Result<StartupRegistrationStatus> {
    let update_exe = locate_update_exe_if_installed()?;
    let startup_shortcut_path = startup_shortcut_path()?;
    let startup_shortcut_exists = startup_shortcut_path.exists();
    ensure_startup_registered_inner(
        update_exe.as_deref(),
        startup_shortcut_exists,
        create_startup_and_start_menu_shortcuts_with_update_exe,
    )
}

fn create_startup_and_start_menu_shortcuts() -> Result<()> {
    let update_exe = locate_update_exe()?;
    create_startup_and_start_menu_shortcuts_with_update_exe(&update_exe)
}

fn create_startup_and_start_menu_shortcuts_with_update_exe(update_exe: &Path) -> Result<()> {
    let exe_name = binary_filename_with_extension();

    let variants = [
        vec![
            "--createShortcut".to_string(),
            exe_name.clone(),
            "--shortcut-locations=Startup,StartMenu".to_string(),
        ],
        vec![
            format!("--createShortcut={exe_name}"),
            "--shortcut-locations=Startup,StartMenu".to_string(),
        ],
        vec![
            "--createShortcut".to_string(),
            exe_name.clone(),
            "-l=Startup,StartMenu".to_string(),
        ],
        vec![
            format!("--createShortcut={exe_name}"),
            "-l=Startup,StartMenu".to_string(),
        ],
    ];

    run_update_variants(update_exe, &variants, "create startup/start menu shortcuts")
}

fn remove_startup_and_start_menu_shortcuts() -> Result<()> {
    let update_exe = locate_update_exe()?;
    remove_startup_and_start_menu_shortcuts_with_update_exe(&update_exe)
}

fn remove_startup_and_start_menu_shortcuts_with_update_exe(update_exe: &Path) -> Result<()> {
    let exe_name = binary_filename_with_extension();

    let variants = [
        vec![
            "--removeShortcut".to_string(),
            exe_name.clone(),
            "--shortcut-locations=Startup,StartMenu".to_string(),
        ],
        vec![
            format!("--removeShortcut={exe_name}"),
            "--shortcut-locations=Startup,StartMenu".to_string(),
        ],
        vec![
            "--removeShortcut".to_string(),
            exe_name.clone(),
            "-l=Startup,StartMenu".to_string(),
        ],
        vec![
            format!("--removeShortcut={exe_name}"),
            "-l=Startup,StartMenu".to_string(),
        ],
    ];

    run_update_variants(update_exe, &variants, "remove startup/start menu shortcuts")
}

pub(crate) fn locate_update_exe() -> Result<PathBuf> {
    let current_exe = std::env::current_exe().context("failed to resolve current executable")?;
    let update_exe = update_exe_path_from_current_exe(&current_exe)?;
    if !update_exe.exists() {
        anyhow::bail!(
            "squirrel Update.exe was not found at {}; lifecycle event cannot continue",
            update_exe.display()
        );
    }
    Ok(update_exe)
}

fn locate_update_exe_if_installed() -> Result<Option<PathBuf>> {
    let current_exe = std::env::current_exe().context("failed to resolve current executable")?;
    let update_exe = update_exe_path_from_current_exe(&current_exe)?;
    if update_exe.exists() {
        return Ok(Some(update_exe));
    }
    Ok(None)
}

fn update_exe_path_from_current_exe(current_exe: &Path) -> Result<PathBuf> {
    let app_dir = current_exe.parent().with_context(|| {
        format!(
            "failed to resolve app directory for {}",
            current_exe.display()
        )
    })?;
    let root_dir = app_dir.parent().with_context(|| {
        format!(
            "failed to resolve squirrel root directory from {}",
            app_dir.display()
        )
    })?;
    Ok(root_dir.join("Update.exe"))
}

fn startup_shortcut_path() -> Result<PathBuf> {
    let appdata = std::env::var_os("APPDATA")
        .context("failed to resolve APPDATA for startup shortcut path")?;
    Ok(startup_shortcut_path_from_appdata(Path::new(&appdata)))
}

fn startup_shortcut_path_from_appdata(appdata: &Path) -> PathBuf {
    appdata
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("Startup")
        .join(format!("{}.lnk", BINARY_FILENAME))
}

fn ensure_startup_registered_inner<F>(
    update_exe: Option<&Path>,
    startup_shortcut_exists: bool,
    create_shortcuts: F,
) -> Result<StartupRegistrationStatus>
where
    F: FnOnce(&Path) -> Result<()>,
{
    let Some(update_exe) = update_exe else {
        return Ok(StartupRegistrationStatus::SkippedNotInstalled);
    };

    if startup_shortcut_exists {
        return Ok(StartupRegistrationStatus::AlreadyRegistered);
    }

    create_shortcuts(update_exe)?;
    Ok(StartupRegistrationStatus::RegisteredNow)
}

fn run_update_variants(update_exe: &Path, variants: &[Vec<String>], action: &str) -> Result<()> {
    let mut failures = Vec::new();

    for args in variants {
        let output = Command::new(update_exe)
            .args(args)
            .output()
            .with_context(|| format!("failed to execute {}", update_exe.display()))?;

        if output.status.success() {
            return Ok(());
        }

        failures.push(format!(
            "args=[{}] status={} stderr={}",
            args.join(" "),
            output
                .status
                .code()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "<signal>".to_string()),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    anyhow::bail!(
        "failed to {action} via {}: {}",
        update_exe.display(),
        failures.join(" | ")
    );
}

fn binary_filename_with_extension() -> String {
    format!("{BINARY_FILENAME}.exe")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn startup_shortcut_path_uses_expected_windows_location() {
        let appdata = Path::new(r"C:\Users\alice\AppData\Roaming");
        let shortcut = startup_shortcut_path_from_appdata(appdata);
        assert!(shortcut.ends_with(Path::new(r"Startup\aura.lnk")));
    }

    #[test]
    fn update_exe_path_resolves_from_current_exe_layout() {
        let current_exe = Path::new(r"C:\Users\alice\AppData\Local\aura\app-1.2.3\aura.exe");
        let update_exe = update_exe_path_from_current_exe(current_exe).unwrap();
        assert_eq!(
            update_exe,
            PathBuf::from(r"C:\Users\alice\AppData\Local\aura\Update.exe")
        );
    }

    #[test]
    fn startup_registration_skips_when_not_installed() {
        let status = ensure_startup_registered_inner(None, false, |_| {
            anyhow::bail!("create should not be called")
        })
        .unwrap();
        assert_eq!(status, StartupRegistrationStatus::SkippedNotInstalled);
    }

    #[test]
    fn startup_registration_is_noop_when_shortcut_exists() {
        let status = ensure_startup_registered_inner(Some(Path::new("Update.exe")), true, |_| {
            anyhow::bail!("create should not be called")
        })
        .unwrap();
        assert_eq!(status, StartupRegistrationStatus::AlreadyRegistered);
    }

    #[test]
    fn startup_registration_restores_when_shortcut_is_missing() {
        let mut create_called = false;
        let status = ensure_startup_registered_inner(Some(Path::new("Update.exe")), false, |_| {
            create_called = true;
            Ok(())
        })
        .unwrap();
        assert_eq!(status, StartupRegistrationStatus::RegisteredNow);
        assert!(create_called);
    }

    #[test]
    fn update_exe_path_supports_temp_layout() {
        let tmp = tempdir().unwrap();
        let app_dir = tmp.path().join("app-2.0.0");
        std::fs::create_dir_all(&app_dir).unwrap();
        let current_exe = app_dir.join("aura.exe");
        std::fs::write(&current_exe, b"").unwrap();

        let update_exe = update_exe_path_from_current_exe(&current_exe).unwrap();
        assert_eq!(update_exe, tmp.path().join("Update.exe"));
    }
}
