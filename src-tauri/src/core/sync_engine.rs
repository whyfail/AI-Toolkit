use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

#[cfg(windows)]
use crate::utils::SuppressConsole;
use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub enum SyncMode {
    Auto,
    Symlink,
    Junction,
    Copy,
}

#[derive(Clone, Debug)]
pub struct SyncOutcome {
    pub mode_used: SyncMode,
    pub target_path: PathBuf,
    pub replaced: bool,
}

type TargetLock = Arc<Mutex<()>>;

static TARGET_LOCKS: OnceLock<Mutex<HashMap<PathBuf, TargetLock>>> = OnceLock::new();

fn normalized_target_key(target: &Path) -> PathBuf {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let normalized_parent = parent
        .canonicalize()
        .unwrap_or_else(|_| parent.to_path_buf());
    match target.file_name() {
        Some(name) => normalized_parent.join(name),
        None => normalized_parent,
    }
}

fn sync_target_lock(target: &Path) -> Result<TargetLock> {
    let locks = TARGET_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = locks
        .lock()
        .map_err(|_| anyhow::anyhow!("sync target lock registry is poisoned"))?;
    Ok(map
        .entry(normalized_target_key(target))
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone())
}

fn paths_resolve_to_same_entry(source: &Path, target: &Path) -> bool {
    match (source.canonicalize(), target.canonicalize()) {
        (Ok(source), Ok(target)) => source == target,
        _ => false,
    }
}

fn reject_destructive_self_overwrite(source: &Path, target: &Path) -> Result<()> {
    if paths_resolve_to_same_entry(source, target) && !is_same_link(target, source) {
        anyhow::bail!(
            "refusing to overwrite source directory with itself: {:?}",
            source
        );
    }
    Ok(())
}

pub fn sync_dir_hybrid(source: &Path, target: &Path) -> Result<SyncOutcome> {
    let target_lock = sync_target_lock(target)?;
    let _target_guard = target_lock
        .lock()
        .map_err(|_| anyhow::anyhow!("sync target lock is poisoned"))?;
    sync_dir_hybrid_inner(source, target)
}

fn sync_dir_hybrid_inner(source: &Path, target: &Path) -> Result<SyncOutcome> {
    if target.exists() {
        if is_same_link(target, source) {
            return Ok(SyncOutcome {
                mode_used: SyncMode::Symlink,
                target_path: target.to_path_buf(),
                replaced: false,
            });
        }
        anyhow::bail!("target already exists: {:?}", target);
    }

    ensure_parent_dir(target)?;

    if try_link_dir(source, target).is_ok() {
        return Ok(SyncOutcome {
            mode_used: SyncMode::Symlink,
            target_path: target.to_path_buf(),
            replaced: false,
        });
    }

    #[cfg(windows)]
    if try_junction(source, target).is_ok() {
        return Ok(SyncOutcome {
            mode_used: SyncMode::Junction,
            target_path: target.to_path_buf(),
            replaced: false,
        });
    }

    copy_dir_recursive(source, target)?;
    Ok(SyncOutcome {
        mode_used: SyncMode::Copy,
        target_path: target.to_path_buf(),
        replaced: false,
    })
}

pub fn sync_dir_hybrid_with_overwrite(
    source: &Path,
    target: &Path,
    overwrite: bool,
) -> Result<SyncOutcome> {
    let target_lock = sync_target_lock(target)?;
    let _target_guard = target_lock
        .lock()
        .map_err(|_| anyhow::anyhow!("sync target lock is poisoned"))?;
    reject_destructive_self_overwrite(source, target)?;
    let mut did_replace = false;
    if std::fs::symlink_metadata(target).is_ok() {
        if is_same_link(target, source) {
            return Ok(SyncOutcome {
                mode_used: SyncMode::Symlink,
                target_path: target.to_path_buf(),
                replaced: false,
            });
        }

        if overwrite {
            remove_path_any(target)
                .with_context(|| format!("remove existing target {:?}", target))?;
            did_replace = true;
        } else {
            anyhow::bail!("target already exists: {:?}", target);
        }
    }

    sync_dir_hybrid_inner(source, target).map(|mut out| {
        out.replaced = did_replace;
        out
    })
}

pub fn sync_dir_copy_with_overwrite(
    source: &Path,
    target: &Path,
    overwrite: bool,
) -> Result<SyncOutcome> {
    let target_lock = sync_target_lock(target)?;
    let _target_guard = target_lock
        .lock()
        .map_err(|_| anyhow::anyhow!("sync target lock is poisoned"))?;
    reject_destructive_self_overwrite(source, target)?;
    let mut did_replace = false;
    if std::fs::symlink_metadata(target).is_ok() {
        if overwrite {
            remove_path_any(target)
                .with_context(|| format!("remove existing target {:?}", target))?;
            did_replace = true;
        } else {
            anyhow::bail!("target already exists: {:?}", target);
        }
    }

    ensure_parent_dir(target)?;
    copy_dir_recursive(source, target)?;

    Ok(SyncOutcome {
        mode_used: SyncMode::Copy,
        target_path: target.to_path_buf(),
        replaced: did_replace,
    })
}

pub fn sync_dir_for_tool_with_overwrite(
    tool_key: &str,
    source: &Path,
    target: &Path,
    overwrite: bool,
) -> Result<SyncOutcome> {
    // Cursor 目前不支持软链/junction：强制使用 copy
    if tool_key.eq_ignore_ascii_case("cursor") {
        return sync_dir_copy_with_overwrite(source, target, overwrite);
    }
    sync_dir_hybrid_with_overwrite(source, target, overwrite)
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create dir {:?}", parent))?;
    }
    Ok(())
}

fn remove_path_any(path: &Path) -> Result<()> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("stat {:?}", path)),
    };
    let ft = meta.file_type();

    if ft.is_symlink() {
        std::fs::remove_file(path).with_context(|| format!("remove symlink {:?}", path))?;
        return Ok(());
    }
    if ft.is_dir() {
        // On Windows, remove_dir_all recursively deletes directory contents,
        // which is wrong for junctions (directory symbolic links) - we only
        // want to delete the junction itself. Use rmdir which handles this correctly.
        #[cfg(windows)]
        {
            let exit = std::process::Command::new("cmd")
                .suppress_console()
                .args(["/c", "rmdir", path.to_string_lossy().as_ref()])
                .output();
            if let Ok(output) = exit {
                if output.status.success() {
                    return Ok(());
                }
                // If rmdir fails (e.g., not a junction, or regular directory),
                // fall through to remove_dir_all
            }
        }
        std::fs::remove_dir_all(path).with_context(|| format!("remove dir {:?}", path))?;
        return Ok(());
    }
    std::fs::remove_file(path).with_context(|| format!("remove file {:?}", path))?;
    Ok(())
}

fn is_same_link(link_path: &Path, target: &Path) -> bool {
    // Try read_link first (works for symbolic links)
    if let Ok(existing) = std::fs::read_link(link_path) {
        // Resolve relative paths against the link's parent directory
        let existing_resolved = if existing.is_absolute() {
            existing.clone()
        } else {
            link_path.parent().unwrap_or(Path::new(".")).join(&existing)
        };

        // Canonicalize both paths for reliable comparison
        if let (Ok(existing_norm), Ok(target_norm)) =
            (existing_resolved.canonicalize(), target.canonicalize())
        {
            #[cfg(windows)]
            {
                // Windows paths are case-insensitive
                existing_norm.to_string_lossy().to_lowercase()
                    == target_norm.to_string_lossy().to_lowercase()
            }
            #[cfg(not(windows))]
            {
                existing_norm == target_norm
            }
        } else {
            // Fallback to direct comparison if canonicalization fails
            existing == target
        }
    } else {
        // read_link failed — on Windows this may be a junction
        #[cfg(windows)]
        {
            let output = std::process::Command::new("fsutil")
                .suppress_console()
                .args([
                    "reparsepoint",
                    "query",
                    link_path.to_string_lossy().as_ref(),
                ])
                .output();
            if let Ok(output) = output {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    for line in stdout.lines() {
                        if line.contains("Print Name:") {
                            let target_str = line.splitn(2, ':').nth(1).unwrap_or("").trim();
                            let target_str =
                                target_str.trim_start_matches('"').trim_end_matches('"');
                            let reparsed = PathBuf::from(target_str);
                            if let (Ok(reparse_norm), Ok(target_norm)) =
                                (reparsed.canonicalize(), target.canonicalize())
                            {
                                if reparse_norm.to_string_lossy().to_lowercase()
                                    == target_norm.to_string_lossy().to_lowercase()
                                {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
            false
        }
        #[cfg(not(windows))]
        {
            false
        }
    }
}

fn try_link_dir(source: &Path, target: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, target)
            .with_context(|| format!("symlink {:?} -> {:?}", target, source))?;
        Ok(())
    }

    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(source, target)
            .with_context(|| format!("symlink {:?} -> {:?}", target, source))?;
        Ok(())
    }

    #[cfg(not(any(unix, windows)))]
    anyhow::bail!("symlink not supported on this platform");
}

#[cfg(windows)]
fn try_junction(source: &Path, target: &Path) -> Result<()> {
    junction::create(source, target)
        .with_context(|| format!("junction {:?} -> {:?}", target, source))?;
    Ok(())
}

fn should_skip_copy(entry: &walkdir::DirEntry) -> bool {
    entry.file_name() == ".git"
}

pub fn copy_dir_recursive(source: &Path, target: &Path) -> Result<()> {
    if paths_resolve_to_same_entry(source, target) {
        anyhow::bail!(
            "refusing to copy directory onto itself: {:?} -> {:?}",
            source,
            target
        );
    }

    for entry in walkdir::WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !should_skip_copy(entry))
    {
        let entry = entry?;
        if should_skip_copy(&entry) {
            continue;
        }
        let relative = entry.path().strip_prefix(source)?;
        let target_path = target.join(relative);

        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target_path)
                .with_context(|| format!("create dir {:?}", target_path))?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            if paths_resolve_to_same_entry(entry.path(), &target_path) {
                anyhow::bail!(
                    "refusing to copy file onto itself: {:?} -> {:?}",
                    entry.path(),
                    target_path
                );
            }
            std::fs::copy(entry.path(), &target_path)
                .with_context(|| format!("copy file {:?} -> {:?}", entry.path(), target_path))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_dir_recursive_rejects_same_directory_without_truncating_files() {
        let source = tempfile::tempdir().unwrap();
        let skill = source.path().join("SKILL.md");
        std::fs::write(&skill, "skill contents").unwrap();

        let error = copy_dir_recursive(source.path(), source.path()).unwrap_err();

        assert!(error.to_string().contains("onto itself"));
        assert_eq!(std::fs::read_to_string(skill).unwrap(), "skill contents");
    }

    #[test]
    fn overwrite_rejects_same_directory_without_deleting_it() {
        let source = tempfile::tempdir().unwrap();
        let skill = source.path().join("SKILL.md");
        std::fs::write(&skill, "skill contents").unwrap();

        let error = sync_dir_hybrid_with_overwrite(source.path(), source.path(), true).unwrap_err();

        assert!(error.to_string().contains("with itself"));
        assert_eq!(std::fs::read_to_string(skill).unwrap(), "skill contents");
    }

    #[cfg(unix)]
    #[test]
    fn concurrent_syncs_to_shared_target_preserve_source_contents() {
        let source = tempfile::tempdir().unwrap();
        let target_parent = tempfile::tempdir().unwrap();
        let skill = source.path().join("SKILL.md");
        let target = target_parent.path().join("shared-skill");
        std::fs::write(&skill, "skill contents").unwrap();

        let handles: Vec<_> = (0..2)
            .map(|_| {
                let source = source.path().to_path_buf();
                let target = target.clone();
                std::thread::spawn(move || {
                    sync_dir_hybrid_with_overwrite(&source, &target, true).unwrap()
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(std::fs::read_to_string(&skill).unwrap(), "skill contents");
        assert_eq!(
            target.canonicalize().unwrap(),
            source.path().canonicalize().unwrap()
        );
    }
}
