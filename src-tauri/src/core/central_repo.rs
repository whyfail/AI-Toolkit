use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use anyhow::{Context, Result};
use dirs::home_dir;

pub const SKILLS_INSTALL_LOCATION_SETTING_KEY: &str = "skills_install_location";
pub const AGENTS_SKILLS_LOCATION_ID: &str = "agents";
pub const AI_TOOLKIT_SKILLS_LOCATION_ID: &str = "ai_toolkit";
const DEFAULT_SKILLS_LOCATION_ID: &str = AGENTS_SKILLS_LOCATION_ID;
const AI_TOOLKIT_DIR_NAME: &str = ".ai-toolkit";
const AGENTS_DIR_NAME: &str = ".agents";
const SKILLS_SUBDIR: &str = "skills";
static SKILLS_INSTALL_LOCATION: OnceLock<RwLock<String>> = OnceLock::new();

fn skills_install_location_lock() -> &'static RwLock<String> {
    SKILLS_INSTALL_LOCATION.get_or_init(|| RwLock::new(DEFAULT_SKILLS_LOCATION_ID.to_string()))
}

pub fn normalize_skills_install_location(id: &str) -> Option<&'static str> {
    match id {
        AGENTS_SKILLS_LOCATION_ID => Some(AGENTS_SKILLS_LOCATION_ID),
        AI_TOOLKIT_SKILLS_LOCATION_ID => Some(AI_TOOLKIT_SKILLS_LOCATION_ID),
        _ => None,
    }
}

pub fn get_skills_install_location() -> String {
    skills_install_location_lock()
        .read()
        .map(|location| location.clone())
        .unwrap_or_else(|_| DEFAULT_SKILLS_LOCATION_ID.to_string())
}

pub fn set_skills_install_location(id: &str) -> Result<String> {
    let normalized = normalize_skills_install_location(id)
        .ok_or_else(|| anyhow::anyhow!("unsupported skills install location: {}", id))?;
    let mut location = skills_install_location_lock()
        .write()
        .map_err(|_| anyhow::anyhow!("failed to update skills install location"))?;
    *location = normalized.to_string();
    Ok(normalized.to_string())
}

pub fn resolve_central_repo_path_for_location(id: &str) -> Result<PathBuf> {
    let normalized = normalize_skills_install_location(id)
        .ok_or_else(|| anyhow::anyhow!("unsupported skills install location: {}", id))?;
    if let Some(home) = home_dir() {
        let root_dir = match normalized {
            AGENTS_SKILLS_LOCATION_ID => AGENTS_DIR_NAME,
            AI_TOOLKIT_SKILLS_LOCATION_ID => AI_TOOLKIT_DIR_NAME,
            _ => unreachable!(),
        };
        return Ok(home.join(root_dir).join(SKILLS_SUBDIR));
    }
    anyhow::bail!("failed to resolve home directory")
}

/// Resolve the central skills repository path.
pub fn resolve_central_repo_path() -> Result<PathBuf> {
    resolve_central_repo_path_for_location(&get_skills_install_location())
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
    fn test_resolve_ai_toolkit_repo_path() {
        let path = resolve_central_repo_path_for_location(AI_TOOLKIT_SKILLS_LOCATION_ID).unwrap();
        assert!(path.to_string_lossy().contains(".ai-toolkit"));
        assert!(path.to_string_lossy().contains("skills"));
    }

    #[test]
    fn test_resolve_agents_repo_path() {
        let path = resolve_central_repo_path_for_location(AGENTS_SKILLS_LOCATION_ID).unwrap();
        assert!(path.to_string_lossy().contains(".agents"));
        assert!(path.to_string_lossy().contains("skills"));
    }
}
