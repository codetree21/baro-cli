use anyhow::{Context, Result};
use std::path::Path;

use crate::types::Manifest;

const MANIFEST_DIR: &str = ".baro";
const MANIFEST_FILE: &str = "manifest.json";

pub fn read(dir: &Path) -> Result<Manifest> {
    let path = dir.join(MANIFEST_DIR).join(MANIFEST_FILE);
    let content = std::fs::read_to_string(&path)
        .context("Not a baro product (no .baro/manifest.json found)")?;
    let manifest: Manifest = serde_json::from_str(&content)?;
    Ok(manifest)
}

pub fn write(dir: &Path, manifest: &Manifest) -> Result<()> {
    let baro_dir = dir.join(MANIFEST_DIR);
    std::fs::create_dir_all(&baro_dir)?;
    let path = baro_dir.join(MANIFEST_FILE);
    std::fs::write(&path, serde_json::to_string_pretty(manifest)?)?;
    Ok(())
}
