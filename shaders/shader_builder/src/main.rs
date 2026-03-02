use anyhow::{Context, Result};
use clap::Parser;
use spirv_builder::{ModuleResult, SpirvBuilder, SpirvMetadata};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    shader_crate: PathBuf,
    #[arg(long)]
    out: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let shader_crate = args
        .shader_crate
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", args.shader_crate.display()))?;

    let build = SpirvBuilder::new(shader_crate, "spirv-unknown-vulkan1.1")
        .spirv_metadata(SpirvMetadata::None)
        .multimodule(false)
        .build()
        .context("failed to compile shader crate with rust-gpu")?;

    let module_path = match build.module {
        ModuleResult::SingleModule(path) => path,
        ModuleResult::MultiModule(map) => map
            .into_values()
            .next()
            .context("shader builder produced no modules")?,
    };

    if let Some(parent) = args.out.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::copy(&module_path, &args.out).with_context(|| {
        format!(
            "failed to copy compiled module {} -> {}",
            module_path.display(),
            args.out.display()
        )
    })?;

    println!("{}", args.out.display());
    Ok(())
}
