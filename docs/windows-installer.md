# Windows Installer (Squirrel)

`aura` uses `Squirrel.Windows` for per-user installation and update packaging.

## Install Scope

- Installs into user space under `%LOCALAPPDATA%`.
- No administrator rights are required.

## Startup Behavior

- Installer registers app startup using Squirrel shortcut management.
- Startup is always enabled for the current user.
- Startup and Start Menu shortcuts are created on install/update and removed on uninstall.

## Runtime Lifecycle Flags

Squirrel may launch the app with one of these internal flags:

- `--squirrel-install`
- `--squirrel-updated`
- `--squirrel-uninstall`
- `--squirrel-obsolete`
- `--squirrel-firstrun`

`aura` handles these flags at startup before normal runtime initialization.

## Build Artifacts

The release pipeline publishes both existing assets and Squirrel assets:

- `aura-<version>-windows.exe`
- `aura-<version>-windows.zip`
- `aura-<version>-windows-installer.exe`
- `aura-<version>-windows-installer.zip`
- `RELEASES`
- `*.nupkg` (full package and delta package when generated)

These Squirrel artifacts are required for in-app self-update support.

## Packaging Determinism

- `scripts/windows/package-squirrel.ps1` installs a pinned `Squirrel.Windows` version for deterministic local/CI behavior.
- CI sets this via `SQUIRREL_WINDOWS_VERSION` in `.github/workflows/release.yml`.
- Packaging hard-fails if generated installer binaries contain known dummy `Update.exe` marker text.

## Runtime Self-Updates

- `aura` checks for updates on startup and periodically in the background (default `6h`).
- Tray menu includes `Check for Updates` for manual checks.
- When an update is available, `aura` downloads and installs silently via `Update.exe --update`.
- In image mode, restart is deferred until the next wallpaper-switch cycle.
- In shader mode, restart is immediate after install completes.

## Feed Requirements

- The updater feed URL must host Squirrel `RELEASES` and matching `.nupkg` files at the same base URL.
- Default feed URL is GitHub release latest-download assets:
  - `https://github.com/hmerritt/aura/releases/latest/download`
