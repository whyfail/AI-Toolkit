use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use walkdir::{DirEntry, WalkDir};

const IGNORE_NAMES: [&str; 4] = [".git", ".DS_Store", "Thumbs.db", ".gitignore"];

fn is_ignored(entry: &DirEntry) -> bool {
    let file_name = entry.file_name().to_string_lossy();
    IGNORE_NAMES.iter().any(|name| name == &file_name.as_ref())
}

/// Compute a hash of the directory contents (files + their relative paths).
/// Files are ordered alphabetically for deterministic output.
pub fn hash_dir(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut budget = super::resource_limits::ResourceBudget::new()?;

    // Collect all entries first and sort for deterministic ordering
    let mut entries = Vec::new();
    for entry in WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !is_ignored(entry))
    {
        let entry = entry?;
        budget.enter(entry.depth())?;
        entries.push(entry);
    }

    entries.sort_by(|a, b| a.path().cmp(b.path()));

    for entry in entries {
        if is_ignored(&entry) {
            continue;
        }

        let relative = entry
            .path()
            .strip_prefix(path)
            .with_context(|| format!("strip prefix {:?}", entry.path()))?;
        hasher.update(relative.to_string_lossy().as_bytes());

        if entry.file_type().is_file() {
            let mut file = std::fs::File::open(entry.path())
                .with_context(|| format!("read file {:?}", entry.path()))?;
            let mut chunk = [0u8; 64 * 1024];
            loop {
                let count = file.read(&mut chunk)?;
                if count == 0 {
                    break;
                }
                budget.bytes(count as u64)?;
                hasher.update(&chunk[..count]);
            }
        }
    }

    let digest = hasher.finalize();
    Ok(hex::encode(digest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_dir_empty() {
        let temp_dir = tempfile::tempdir().unwrap();
        let hash = hash_dir(temp_dir.path()).unwrap();
        // Empty directory should produce a consistent hash
        assert!(!hash.is_empty());
    }
}
