use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use ignore::WalkBuilder;
use sha2::{Digest, Sha256};
use std::path::Path;
use tar::{Archive, Builder};

const EXCLUDED_DIRS: &[&str] = &[".git", ".baro", "target", "node_modules", ".next"];

/// Create a tar.gz archive from a directory, respecting .gitignore.
/// Returns (bytes, sha256_hex).
pub fn create_archive(dir: &Path) -> Result<(Vec<u8>, String)> {
    let buf = Vec::new();
    let encoder = GzEncoder::new(buf, Compression::default());
    let mut builder = Builder::new(encoder);

    let walker = WalkBuilder::new(dir)
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            // Exclude known dirs
            if entry.file_type().map_or(false, |ft| ft.is_dir()) {
                return !EXCLUDED_DIRS.contains(&name.as_ref());
            }
            // Exclude .env* files
            if name.starts_with(".env") {
                return false;
            }
            true
        })
        .build();

    for entry in walker {
        let entry = entry?;
        let path = entry.path();

        if path == dir {
            continue;
        }

        let relative = path
            .strip_prefix(dir)
            .context("Failed to compute relative path")?;

        if path.is_file() {
            builder
                .append_path_with_name(path, relative)
                .with_context(|| format!("Failed to add file: {}", relative.display()))?;
        } else if path.is_dir() {
            builder
                .append_dir(relative, path)
                .with_context(|| format!("Failed to add dir: {}", relative.display()))?;
        }
    }

    let encoder = builder.into_inner()?;
    let bytes = encoder.finish()?;

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let hash = format!("{:x}", hasher.finalize());

    Ok((bytes, hash))
}

/// Extract a tar.gz archive into a destination directory.
pub fn extract_archive(bytes: &[u8], dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    let decoder = GzDecoder::new(bytes);
    let mut archive = Archive::new(decoder);
    archive.unpack(dest)?;
    Ok(())
}
