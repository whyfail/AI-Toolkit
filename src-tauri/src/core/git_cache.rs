use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::git_fetcher::{clone_or_pull, validate_cached_origin, validate_git_source};
use super::private_fs::{ensure_private_dir, secure_file};

static CACHE_LOCK: Mutex<()> = Mutex::new(());

pub struct CachedRepo {
    pub path: PathBuf,
    pub revision: String,
    // Keep the checkout stable until the caller finishes scanning/copying it.
    _guard: MutexGuard<'static, ()>,
}

#[derive(Serialize, Deserialize)]
struct CacheMeta {
    fetched_at: u64,
    revision: String,
}

pub fn cached_repo(url: &str, branch: Option<&str>) -> Result<CachedRepo> {
    let root = dirs::cache_dir()
        .context("无法解析用户缓存目录")?
        .join("ai-toolkit");
    ensure_private_dir(&root)?;
    cached_repo_in(&root.join("git-v2"), url, branch)
}

fn cached_repo_in(root: &Path, url: &str, branch: Option<&str>) -> Result<CachedRepo> {
    validate_git_source(url, branch)?;
    let guard = CACHE_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("Git 缓存锁不可用"))?;
    ensure_private_dir(root)?;
    let key = hex::encode(Sha256::digest(
        format!("{url}\n{}", branch.unwrap_or("")).as_bytes(),
    ));
    let path = root.join(&key);
    let meta_path = root.join(format!("{key}.json"));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let ttl = std::env::var("MCP_MANAGER_GIT_CACHE_TTL_SECS")
        .ok()
        .map(|v| v.parse::<u64>())
        .transpose()
        .context("Git 缓存 TTL 无效")?
        .unwrap_or(3600);
    let exists = std::fs::symlink_metadata(&path).is_ok();
    if exists {
        // Validate before reading metadata or executing anything from this checkout.
        validate_cached_origin(url, &path)?;
        if meta_path.exists() {
            secure_file(&meta_path)?;
            if let Ok(meta) = serde_json::from_slice::<CacheMeta>(&std::fs::read(&meta_path)?) {
                if ttl > 0 && meta.fetched_at <= now && now - meta.fetched_at < ttl {
                    return Ok(CachedRepo {
                        path,
                        revision: meta.revision,
                        _guard: guard,
                    });
                }
            }
        }
    }
    // A failed fetch never destroys a known-good checkout. Publish only a complete clone.
    let stage = tempfile::Builder::new().prefix("fetch-").tempdir_in(root)?;
    let checkout = stage.path().join("repo");
    let revision = clone_or_pull(url, &checkout, branch)?;
    ensure_private_dir(&checkout)?;
    validate_cached_origin(url, &checkout)?;
    if exists {
        std::fs::rename(&path, stage.path().join("previous"))?;
    }
    if let Err(error) = std::fs::rename(&checkout, &path) {
        if exists {
            if let Err(restore) = std::fs::rename(stage.path().join("previous"), &path) {
                let recovery = stage.keep();
                anyhow::bail!(
                    "缓存更新失败: {error}; 恢复失败: {restore}; 原缓存保存在 {:?}",
                    recovery
                );
            }
        }
        return Err(error.into());
    }
    let mut meta = tempfile::NamedTempFile::new_in(root)?;
    serde_json::to_writer(
        &mut meta,
        &CacheMeta {
            fetched_at: now,
            revision: revision.clone(),
        },
    )?;
    meta.persist(&meta_path).context("保存 Git 缓存状态失败")?;
    secure_file(&meta_path)?;
    Ok(CachedRepo {
        path,
        revision,
        _guard: guard,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn local_cache_preserves_slash_branches_ttl_and_previous_checkout_on_failure() {
        let sandbox = tempfile::tempdir().unwrap();
        let source = sandbox.path().join("source repo");
        let cache = sandbox.path().join("cache");
        std::fs::create_dir(&source).unwrap();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(&source)
                .args([
                    "-c",
                    "user.name=Security Test",
                    "-c",
                    "user.email=security@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                ])
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        git(&["init"]);
        git(&["checkout", "-b", "feature/security"]);
        std::fs::write(source.join("SKILL.md"), "first").unwrap();
        git(&["add", "SKILL.md"]);
        git(&["commit", "-m", "fixture"]);
        let url = source.to_str().unwrap();
        let first = cached_repo_in(&cache, url, Some("feature/security")).unwrap();
        let path = first.path.clone();
        let revision = first.revision.clone();
        drop(first);
        assert_eq!(
            std::fs::read_to_string(path.join("SKILL.md")).unwrap(),
            "first"
        );
        std::fs::write(source.join("SKILL.md"), "second").unwrap();
        git(&["commit", "-am", "updated fixture"]);
        let fresh = cached_repo_in(&cache, url, Some("feature/security")).unwrap();
        assert_eq!(fresh.revision, revision);
        drop(fresh);
        let meta = std::fs::read_dir(&cache)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p.extension().is_some_and(|e| e == "json"))
            .unwrap();
        let expire = || {
            std::fs::write(
                &meta,
                serde_json::to_vec(&CacheMeta {
                    fetched_at: 0,
                    revision: revision.clone(),
                })
                .unwrap(),
            )
            .unwrap()
        };
        expire();
        let updated = cached_repo_in(&cache, url, Some("feature/security")).unwrap();
        assert_ne!(updated.revision, revision);
        drop(updated);
        assert_eq!(
            std::fs::read_to_string(path.join("SKILL.md")).unwrap(),
            "second"
        );
        expire();
        std::fs::rename(source.join(".git"), source.join("saved-git")).unwrap();
        assert!(cached_repo_in(&cache, url, Some("feature/security")).is_err());
        assert_eq!(
            std::fs::read_to_string(path.join("SKILL.md")).unwrap(),
            "second"
        );
    }

    #[test]
    fn mismatched_cached_origin_is_rejected_before_fetch() {
        let sandbox = tempfile::tempdir().unwrap();
        let root = sandbox.path().join("cache");
        ensure_private_dir(&root).unwrap();
        let url = "https://example.invalid/requested.git";
        let key = hex::encode(Sha256::digest(format!("{url}\n").as_bytes()));
        let repo = root.join(key);
        assert!(Command::new("git")
            .arg("init")
            .arg(&repo)
            .output()
            .unwrap()
            .status
            .success());
        assert!(Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args([
                "remote",
                "add",
                "origin",
                "https://example.invalid/other.git"
            ])
            .output()
            .unwrap()
            .status
            .success());
        assert!(cached_repo_in(&root, url, None).is_err());
    }
}
