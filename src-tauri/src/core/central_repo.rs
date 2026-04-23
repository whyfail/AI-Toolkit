use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use dirs::home_dir;

const CENTRAL_DIR_NAME: &str = ".ai-toolkit";
const SKILLS_SUBDIR: &str = "skills";

/// Resolve the central skills repository path: ~/.ai-toolkit/skills/
pub fn resolve_central_repo_path() -> Result<PathBuf> {
    if let Some(home) = home_dir() {
        return Ok(home.join(CENTRAL_DIR_NAME).join(SKILLS_SUBDIR));
    }
    anyhow::bail!("failed to resolve home directory")
}

/// Ensure the central repository directory exists
pub fn ensure_central_repo(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).with_context(|| format!("create {:?}", path))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_central_repo_path() {
        let path = resolve_central_repo_path().unwrap();
        assert!(path.to_string_lossy().contains(".ai-toolkit"));
        assert!(path.to_string_lossy().contains("skills"));
    }
}
