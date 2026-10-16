use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

use anyhow::{bail, Context, Result};

static INSTALL_LOCK: Mutex<()> = Mutex::new(());

/// Names cross IPC and repository boundaries. Treat both OS separators as separators.
pub fn validate_skill_name(name: &str) -> Result<()> {
    if name.is_empty()
        || matches!(name, "." | "..")
        || name.chars().any(|c| c.is_control() || "/\\:".contains(c))
        || !matches!(
            Path::new(name).components().next(),
            Some(Component::Normal(_))
        )
    {
        bail!("技能名称必须是单个目录名称: {:?}", name);
    }
    #[cfg(windows)]
    {
        let stem = name.split('.').next().unwrap_or("").to_uppercase();
        if name.ends_with([' ', '.'])
            || name.contains(['<', '>', '"', '|', '?', '*'])
            || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || ["COM", "LPT"].iter().any(|prefix| {
                stem.strip_prefix(prefix).is_some_and(|n| {
                    matches!(
                        n,
                        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                    )
                })
            })
        {
            bail!("Windows 不支持此技能目录名称: {:?}", name);
        }
    }
    Ok(())
}

/// Resolve only the parent: the final entry may be a link that must be replaced/unlinked.
pub fn skill_entry(root: &Path, name: &str) -> Result<PathBuf> {
    validate_skill_name(name)?;
    Ok(root
        .canonicalize()
        .with_context(|| format!("解析技能目录 {:?}", root))?
        .join(name))
}

/// Legacy names may be non-portable. Rename only an existing native child entry.
pub fn existing_skill_entry(root: &Path, name: &str) -> Result<PathBuf> {
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(part)) if part == std::ffi::OsStr::new(name))
        || components.next().is_some()
    {
        bail!("旧技能名称超出受管目录，不能自动移动: {:?}", name);
    }
    let root = root.canonicalize()?;
    let entry = root.join(name);
    let metadata = std::fs::symlink_metadata(&entry)?;
    let is_link = metadata.file_type().is_symlink();
    #[cfg(windows)]
    let is_link = is_link || junction::exists(&entry)?;
    if !is_link && (!metadata.is_dir() || entry.canonicalize()?.parent() != Some(root.as_path())) {
        bail!("旧技能不是受管目录内的子目录");
    }
    Ok(entry)
}

pub fn validate_subpath(subpath: &str) -> Result<()> {
    if subpath.contains(['\\', ':'])
        || subpath.chars().any(char::is_control)
        || Path::new(subpath)
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        bail!("来源子路径必须位于所选仓库或目录内: {:?}", subpath);
    }
    Ok(())
}

pub fn source_within(root: &Path, subpath: &str) -> Result<PathBuf> {
    validate_subpath(subpath)?;
    let root = root
        .canonicalize()
        .with_context(|| format!("解析来源目录 {:?}", root))?;
    let source = root
        .join(subpath)
        .canonicalize()
        .context("来源子路径不存在")?;
    if !source.starts_with(&root) || !source.is_dir() {
        bail!("来源目录超出允许范围或不是目录");
    }
    Ok(source)
}

pub fn remove_entry(path: &Path) -> Result<()> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    #[cfg(windows)]
    if junction::exists(path)? {
        junction::delete(path).context("移除目录链接失败")?;
        return Ok(());
    }
    if metadata.file_type().is_symlink() {
        #[cfg(windows)]
        {
            use std::os::windows::fs::FileTypeExt;
            if metadata.file_type().is_symlink_dir() {
                std::fs::remove_dir(path)?;
                return Ok(());
            }
        }
        std::fs::remove_file(path)?;
    } else if metadata.is_dir() {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

/// Prepare alongside the repository so publishing uses same-filesystem renames.
pub fn skill_staging(root: &Path) -> Result<tempfile::TempDir> {
    let root = root.canonicalize()?;
    tempfile::Builder::new()
        .prefix(".ai-toolkit-install-")
        .tempdir_in(root.parent().context("技能目录没有父目录")?)
        .context("创建技能临时目录失败")
}

pub fn publish_skill(staged: &Path, root: &Path, name: &str, overwrite: bool) -> Result<PathBuf> {
    let _guard = INSTALL_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("安装锁不可用"))?;
    let target = skill_entry(root, name)?;
    let backup = skill_staging(root)?;
    let old = backup.path().join("previous");
    let exists = std::fs::symlink_metadata(&target).is_ok();
    if exists {
        if !overwrite {
            bail!("Skill already exists in central repo: {:?}", target);
        }
        std::fs::rename(&target, &old).context("保留原技能失败")?;
    }
    if let Err(error) = std::fs::rename(staged, &target) {
        if exists {
            if let Err(restore) = std::fs::rename(&old, &target) {
                let recovery = backup.keep();
                bail!(
                    "安装失败: {error}; 恢复原技能失败: {restore}; 原技能保存在 {:?}",
                    recovery
                );
            }
        }
        return Err(error).context("发布技能失败，原技能已保留");
    }
    if let Err(error) = remove_entry(&old) {
        let recovery = backup.keep();
        log::warn!(
            "技能已安装，旧内容清理失败: {error:#}; 残余内容保存在 {:?}",
            recovery
        );
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn legacy_nonportable_names_can_be_renamed_without_resolving_outside_paths() {
        let root = tempfile::tempdir().unwrap();
        let name = "old\\name";
        std::fs::create_dir(root.path().join(name)).unwrap();
        std::fs::write(root.path().join(name).join("SKILL.md"), "preserved").unwrap();
        assert!(validate_skill_name(name).is_err());
        let old = existing_skill_entry(root.path(), name).unwrap();
        let new = skill_entry(root.path(), "safe-name").unwrap();
        std::fs::rename(old, &new).unwrap();
        assert_eq!(
            std::fs::read_to_string(new.join("SKILL.md")).unwrap(),
            "preserved"
        );
        for outside in ["..", "../victim", "/tmp/victim", "safe-name/."] {
            assert!(existing_skill_entry(root.path(), outside).is_err());
        }
    }

    #[test]
    fn rejects_names_and_subpaths_that_escape_the_root() {
        for name in [
            "",
            ".",
            "..",
            "../victim",
            "/tmp/victim",
            "C:\\victim",
            "a/b",
            "a\\b",
            "a\0b",
            "a\nb",
        ] {
            assert!(validate_skill_name(name).is_err(), "{name:?}");
        }
        for name in ["中文技能", "my skill", "skill-1_test", "é"] {
            validate_skill_name(name).unwrap();
        }
        for subpath in [
            "../victim",
            "/tmp",
            "a/../../victim",
            "a\\..\\victim",
            "C:/victim",
        ] {
            assert!(validate_subpath(subpath).is_err());
        }
        validate_subpath(".").unwrap();
        validate_subpath("skills/.curated/example").unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn source_links_cannot_escape_and_replacing_links_preserves_their_target() {
        let sandbox = tempfile::tempdir().unwrap();
        let root = sandbox.path().join("skills");
        let outside = sandbox.path().join("outside");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("sentinel"), "untouched").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("linked")).unwrap();
        assert!(source_within(&root, "linked").is_err());
        let stage = skill_staging(&root).unwrap();
        let prepared = stage.path().join("new");
        std::fs::create_dir(&prepared).unwrap();
        std::fs::write(prepared.join("SKILL.md"), "new skill").unwrap();
        publish_skill(&prepared, &root, "linked", true).unwrap();
        assert_eq!(
            std::fs::read_to_string(outside.join("sentinel")).unwrap(),
            "untouched"
        );
        assert!(root.join("linked/SKILL.md").is_file());
    }

    #[test]
    fn failed_publish_restores_the_existing_skill() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("example")).unwrap();
        std::fs::write(root.path().join("example/SKILL.md"), "old skill").unwrap();
        assert!(publish_skill(&root.path().join("missing"), root.path(), "example", true).is_err());
        assert_eq!(
            std::fs::read_to_string(root.path().join("example/SKILL.md")).unwrap(),
            "old skill"
        );
    }
}
