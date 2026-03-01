fn main() {
    println!("cargo:rerun-if-changed=assets/tray.png");
    println!("cargo:rerun-if-changed=build.rs");

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
