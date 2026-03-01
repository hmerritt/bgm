use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    println!("cargo:rerun-if-changed=assets/tray.png");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-env-changed=BGM_VERSION_PRERELEASE");
    println!("cargo:rerun-if-env-changed=BGM_VERSION_METADATA");
    println!("cargo:rerun-if-env-changed=BGM_BUILD_DATE");

    emit_version_metadata();

    let target = std::env::var("TARGET").unwrap_or_default();
    if !target.contains("windows") {
        return;
    }

    let out_dir = std::path::PathBuf::from(
        std::env::var("OUT_DIR").expect("OUT_DIR is required for resource generation"),
    );

    let source_png = std::path::Path::new("assets").join("tray.png");
    if !source_png.exists() {
        panic!("missing source tray image: {}", source_png.display());
    }

    let generated_ico = out_dir.join("tray.ico");
    generate_multi_size_ico(&source_png, &generated_ico);

    let generated_rc = out_dir.join("bgm-auto.rc");
    let ico_path_for_rc = generated_ico.to_string_lossy().replace('\\', "/");
    std::fs::write(&generated_rc, format!("101 ICON \"{}\"\n", ico_path_for_rc))
        .expect("failed to write generated rc file");

    let generated_rc_str = generated_rc
        .to_str()
        .expect("generated rc path must be valid UTF-8");
    let _ = embed_resource::compile(generated_rc_str, embed_resource::NONE);
}

fn generate_multi_size_ico(source_png: &std::path::Path, output_ico: &std::path::Path) {
    let source = image::open(source_png)
        .unwrap_or_else(|e| panic!("failed to load {}: {}", source_png.display(), e))
        .to_rgba8();

    let sizes = [16u32, 20, 24, 32, 48, 64, 256];

    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);
    for size in sizes {
        let resized =
            image::imageops::resize(&source, size, size, image::imageops::FilterType::Lanczos3);
        let icon_image = ico::IconImage::from_rgba_data(size, size, resized.into_raw());
        let entry = ico::IconDirEntry::encode(&icon_image)
            .unwrap_or_else(|e| panic!("failed to encode {}x{} icon entry: {}", size, size, e));
        icon_dir.add_entry(entry);
    }

    let mut file = std::fs::File::create(output_ico)
        .unwrap_or_else(|e| panic!("failed to create {}: {}", output_ico.display(), e));
    icon_dir
        .write(&mut file)
        .unwrap_or_else(|e| panic!("failed to write {}: {}", output_ico.display(), e));
}

fn emit_version_metadata() {
    let git_commit = run_git(&["rev-parse", "--short", "HEAD"]).unwrap_or_default();
    let git_branch = run_git(&["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();

    let build_date = std::env::var("BGM_BUILD_DATE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(current_unix_timestamp);

    let version_prerelease = std::env::var("BGM_VERSION_PRERELEASE").unwrap_or_default();
    let version_metadata = std::env::var("BGM_VERSION_METADATA").unwrap_or_default();

    println!("cargo:rustc-env=BGM_GIT_COMMIT={git_commit}");
    println!("cargo:rustc-env=BGM_GIT_BRANCH={git_branch}");
    println!("cargo:rustc-env=BGM_BUILD_DATE={build_date}");
    println!("cargo:rustc-env=BGM_VERSION_PRERELEASE={version_prerelease}");
    println!("cargo:rustc-env=BGM_VERSION_METADATA={version_metadata}");
}

fn run_git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8(output.stdout).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn current_unix_timestamp() -> String {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs().to_string(),
        Err(_) => String::new(),
    }
}
