# bgm Implementation Plan

## Scope
- Windows-first background manager daemon in Rust
- Long-running process that rotates wallpapers from mixed sources
- HCL config file (`bgm.hcl`)

## Deliverables
- Rust binary crate with modular architecture
- Source handlers for:
  - single image file
  - directory
  - RSS feed image downloader
- Timer-based scheduler:
  - `timer` for wallpaper rotation
  - `remoteUpdateTimer` for RSS refresh
- Image processing pipeline:
  - cover scaling (crop if needed)
  - screen-sized output cache
- Windows wallpaper backend via Win32 API
- Persistent runtime state and cache cleanup
- Unit tests for config parsing and rotation behavior

## Implementation Notes
- Runtime: `tokio`
- Config parsing: `hcl-rs`
- Image processing: `image`
- RSS and downloads: `feed-rs` + `reqwest`
- Logging: `tracing`
- Rotation mode: shuffle no-repeat cycle

## Validation
- Run `cargo test`
- Run daemon with `bgm.hcl` and verify wallpaper updates over time
- Verify RSS refresh adds new candidates without restart

