use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::central_repo::{ensure_central_repo, resolve_central_repo_path};
use super::content_hash::hash_dir;
use super::git_cache::cached_repo;
use super::github_download::{download_github_directory, parse_github_api_params};
use super::path_safety::{
    publish_skill, skill_staging, source_within, validate_skill_name, validate_subpath,
};
use super::sync_engine::copy_dir_recursive;

pub struct InstallResult {
    pub skill_id: String,
    pub name: String,
    pub description: Option<String>,
    pub central_path: PathBuf,
    pub content_hash: Option<String>,
    pub source_subpath: Option<String>,
    pub source_revision: Option<String>,
}

/// Scan base directories used for skill discovery.
const SKILL_SCAN_BASES: [&str; 5] = [
    "skills",
    "skills/.curated",
    "skills/.experimental",
    "skills/.system",
    ".claude/skills",
];

/// Check if a directory is a valid skill (has SKILL.md/skill.md or is a
/// Claude plugin skill under .claude/skills/ with at least one manifest file).
fn is_skill_dir(p: &Path) -> bool {
    if !p.is_dir() {
        return false;
    }
    if find_skill_md(p).is_some() {
        return true;
    }
    if is_claude_skill_dir(p) && has_claude_plugin_manifest(p) {
        return true;
    }
    false
}

/// Check if a directory is a Claude plugin skill (under .claude/skills/).
fn is_claude_skill_dir(p: &Path) -> bool {
    if let Some(parent) = p.parent() {
        let parent_str = parent.to_string_lossy();
        if parent_str.ends_with(".claude/skills") || parent_str.ends_with(".claude\\skills") {
            return true;
        }
    }
    false
}

/// Check if a directory has no entries (used to detect a failed install
/// where the copy step left an empty central path behind).
fn is_dir_empty(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|mut it| it.next().is_none())
        .unwrap_or(true)
}

/// Heuristic: Claude plugin skills almost always ship a manifest file like
/// `.claude-plugin/plugin.json` or `plugin.json`. Without any such marker the
/// directory is likely empty / not a real skill, so we don't surface it.
fn has_claude_plugin_manifest(p: &Path) -> bool {
    const CANDIDATES: &[&str] = &[
        ".claude-plugin/plugin.json",
        "plugin.json",
        "manifest.json",
        "index.js",
        "index.ts",
        "skill.yaml",
        "skill.yml",
    ];
    CANDIDATES.iter().any(|rel| p.join(rel).exists())
}

/// Find SKILL.md or skill.md in a directory (case-insensitive, prefer uppercase).
pub(crate) fn find_skill_md(dir: &Path) -> Option<PathBuf> {
    let root = dir.canonicalize().ok()?;
    ["SKILL.md", "skill.md"].into_iter().find_map(|name| {
        let path = root.join(name).canonicalize().ok()?;
        (path.starts_with(&root) && path.is_file()).then_some(path)
    })
}

/// Extract name and description for a skill directory.
fn extract_skill_info(skill_dir: &Path) -> (String, Option<String>) {
    if let Some(skill_md) = find_skill_md(skill_dir) {
        if let Some((name, desc)) = parse_skill_md(&skill_md) {
            return (name, desc);
        }
    }
    let name = skill_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    (name, None)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GitSkillCandidate {
    pub name: String,
    pub description: Option<String>,
    pub subpath: String,
}

/// Scan all skill candidates in a cloned repo directory, returning GitSkillCandidate list.
pub fn scan_git_skill_candidates(repo_dir: &Path) -> Vec<GitSkillCandidate> {
    let mut out = Vec::new();

    // Root-level skill
    if let Some(skill_md) = find_skill_md(repo_dir) {
        if let Some((name, desc)) = parse_skill_md(&skill_md) {
            out.push(GitSkillCandidate {
                name,
                description: desc,
                subpath: ".".to_string(),
            });
        }
    }

    // Root-level subdirectories (skip "skills" and hidden dirs)
    if let Ok(rd) = std::fs::read_dir(repo_dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                continue;
            }
            let dir_name = entry.file_name();
            let dir_name = dir_name.to_string_lossy();
            if dir_name == "skills" || dir_name.starts_with('.') {
                continue;
            }
            if let Some(skill_md) = find_skill_md(&p) {
                let (name, desc) =
                    parse_skill_md(&skill_md).unwrap_or((dir_name.to_string(), None));
                let rel = p
                    .strip_prefix(repo_dir)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .to_string();
                out.push(GitSkillCandidate {
                    name,
                    description: desc,
                    subpath: rel,
                });
            }
        }
    }

    // Scan known sub-locations: skills/*, .claude/skills/*, etc.
    for base in SKILL_SCAN_BASES {
        let Ok(base_dir) = source_within(repo_dir, base) else {
            continue;
        };
        if let Ok(rd) = std::fs::read_dir(&base_dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false)
                    || !is_skill_dir(&p)
                {
                    continue;
                }
                let (name, desc) = extract_skill_info(&p);
                let rel = p
                    .strip_prefix(repo_dir)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .to_string();
                out.push(GitSkillCandidate {
                    name,
                    description: desc,
                    subpath: rel,
                });
            }
        }
    }

    out.retain(|candidate| {
        validate_skill_name(&candidate.name).is_ok()
            && source_within(repo_dir, &candidate.subpath).is_ok()
    });
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out.dedup_by(|a, b| a.subpath == b.subpath);
    out
}

/// Install a skill from a Git URL (GitHub, GitLab, etc.)
/// When subpath is None and the repo contains multiple skills, returns a MULTI_SKILLS error.
pub fn install_git_skill(
    repo_url: &str,
    name: Option<String>,
    subpath: Option<&str>,
) -> Result<InstallResult> {
    install_git_skill_inner(repo_url, name, subpath, true)
}

/// Install a selected skill using the same validation and publication boundary.
pub fn install_git_skill_from_selection(
    repo_url: &str,
    subpath: &str,
    name: Option<String>,
) -> Result<InstallResult> {
    install_git_skill_inner(repo_url, name, Some(subpath), false)
}

fn install_git_skill_inner(
    repo_url: &str,
    name: Option<String>,
    subpath: Option<&str>,
    allow_api: bool,
) -> Result<InstallResult> {
    let parsed = parse_github_url(repo_url);
    let user_provided_name = name.is_some();
    let effective_subpath = subpath.map(str::to_owned).or(parsed.subpath);
    if let Some(sp) = &effective_subpath {
        validate_subpath(sp)?;
    }
    let mut skill_name = name.unwrap_or_else(|| {
        effective_subpath
            .as_deref()
            .filter(|sp| *sp != ".")
            .and_then(|sp| sp.rsplit('/').next())
            .map(str::to_owned)
            .unwrap_or_else(|| derive_name_from_repo_url(&parsed.clone_url))
    });
    validate_skill_name(&skill_name)?;
    let central_dir = resolve_central_repo_path()?;
    ensure_central_repo(&central_dir)?;
    let staging = skill_staging(&central_dir)?;
    let prepared = staging.path().join("skill");

    let api_params = allow_api
        .then(|| {
            parse_github_api_params(
                &parsed.clone_url,
                parsed.branch.as_deref(),
                effective_subpath.as_deref(),
            )
        })
        .flatten();
    let api_revision = if let Some((owner, repo, branch, sp)) = api_params {
        match download_github_directory(&owner, &repo, &branch, &sp, &prepared, None) {
            Ok(()) => Some(format!("api-download-{branch}")),
            Err(error) => {
                super::path_safety::remove_entry(&prepared)?;
                let message = format!("{error:#}");
                if message.contains("404") || message.contains("Not Found") {
                    anyhow::bail!("该 Skill 在 GitHub 上未找到，请检查仓库链接和路径");
                }
                if message.contains("RATE_LIMITED") || message.contains("403") {
                    anyhow::bail!("GitHub API 访问受限，请稍后重试或配置 GitHub Token");
                }
                log::warn!("GitHub API download failed, falling back to git: {error:#}");
                None
            }
        }
    } else {
        None
    };
    let revision = if let Some(revision) = api_revision {
        revision
    } else {
        let cached = cached_repo(&parsed.clone_url, parsed.branch.as_deref())?;
        let repo_dir = &cached.path;
        if effective_subpath.is_none() {
            let candidates = scan_git_skill_candidates(&repo_dir);
            if candidates.len() >= 2 {
                anyhow::bail!(
                    "MULTI_SKILLS|该仓库包含 {} 个 Skills，请选择具体 Skill 文件夹再导入。",
                    candidates.len()
                );
            }
        }
        let source = source_within(&repo_dir, effective_subpath.as_deref().unwrap_or("."))?;
        copy_dir_recursive(&source, &prepared).context("复制技能到临时目录失败")?;
        cached.revision.clone()
    };
    if is_dir_empty(&prepared) {
        anyhow::bail!("安装内容为空，请在网络稳定后重试");
    }
    let (description, md_name) = find_skill_md(&prepared)
        .and_then(|p| parse_skill_md(&p))
        .map(|(n, d)| (d, Some(n)))
        .unwrap_or((None, None));
    if let Some(md_name) = md_name {
        validate_skill_name(&md_name)?;
        if !user_provided_name {
            // Retain the existing behavior when the document's name is already taken.
            if md_name == skill_name
                || !super::path_safety::skill_entry(&central_dir, &md_name)?.exists()
            {
                skill_name = md_name;
            }
        }
    }
    let content_hash = Some(hash_dir(&prepared)?);
    let central_path = publish_skill(&prepared, &central_dir, &skill_name, true)?;
    Ok(InstallResult {
        skill_id: format!("git-{}", Uuid::new_v4()),
        name: skill_name,
        description,
        central_path,
        content_hash,
        source_subpath: effective_subpath,
        source_revision: Some(revision),
    })
}

pub fn parse_github_url(input: &str) -> ParsedGitSource {
    let trimmed = input.trim().trim_end_matches('/');

    let normalized = if trimmed.starts_with("https://github.com/") {
        trimmed.to_string()
    } else if trimmed.starts_with("github.com/") {
        format!("https://{}", trimmed)
    } else if looks_like_github_shorthand(trimmed) {
        format!("https://github.com/{}", trimmed)
    } else {
        trimmed.to_string()
    };

    let trimmed = normalized.trim_end_matches('/');
    let gh_prefix = "https://github.com/";
    if !trimmed.starts_with(gh_prefix) {
        return ParsedGitSource {
            clone_url: trimmed.to_string(),
            branch: None,
            subpath: None,
        };
    }

    let rest = &trimmed[gh_prefix.len()..];
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() < 2 {
        return ParsedGitSource {
            clone_url: trimmed.to_string(),
            branch: None,
            subpath: None,
        };
    }

    let owner = parts[0];
    let mut repo = parts[1].to_string();
    if let Some(stripped) = repo.strip_suffix(".git") {
        repo = stripped.to_string();
    }
    let clone_url = format!("https://github.com/{}/{}.git", owner, repo);

    if parts.len() >= 4 && (parts[2] == "tree" || parts[2] == "blob") {
        let branch = Some(parts[3].to_string());
        let subpath = if parts.len() > 4 {
            Some(parts[4..].join("/"))
        } else {
            None
        };
        return ParsedGitSource {
            clone_url,
            branch,
            subpath,
        };
    }

    ParsedGitSource {
        clone_url,
        branch: None,
        subpath: None,
    }
}

fn looks_like_github_shorthand(input: &str) -> bool {
    if input.is_empty() {
        return false;
    }
    if input.starts_with('/') || input.starts_with('~') || input.starts_with('.') {
        return false;
    }
    if input.contains("://") || input.contains('@') || input.contains(':') {
        return false;
    }

    let parts: Vec<&str> = input.split('/').collect();
    if parts.len() < 2 {
        return false;
    }

    let owner = parts[0];
    let repo = parts[1];
    if owner.is_empty()
        || repo.is_empty()
        || owner == "."
        || owner == ".."
        || repo == "."
        || repo == ".."
    {
        return false;
    }

    let is_safe_segment = |s: &str| {
        s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    };
    if !is_safe_segment(owner) || !is_safe_segment(repo.trim_end_matches(".git")) {
        return false;
    }

    if parts.len() > 2 {
        matches!(parts[2], "tree" | "blob")
    } else {
        true
    }
}

#[derive(Clone, Debug)]
pub struct ParsedGitSource {
    pub clone_url: String,
    pub branch: Option<String>,
    pub subpath: Option<String>,
}

fn derive_name_from_repo_url(repo_url: &str) -> String {
    let mut name = repo_url
        .split('/')
        .next_back()
        .unwrap_or("skill")
        .to_string();
    if let Some(stripped) = name.strip_suffix(".git") {
        name = stripped.to_string();
    }
    if name.is_empty() {
        "skill".to_string()
    } else {
        name
    }
}

fn parse_skill_md(path: &Path) -> Option<(String, Option<String>)> {
    parse_skill_md_with_reason(path).ok()
}

/// Public helper: extract just the description from a SKILL.md, if any.
/// Used by callers (e.g. `install_local_selection`) that need to surface the
/// description alongside an install result without caring about the name.
pub(crate) fn read_skill_description(dir: &Path) -> Option<String> {
    find_skill_md(dir)
        .and_then(|p| parse_skill_md(&p))
        .and_then(|(_, d)| d)
}

pub(crate) fn copy_local_skill_dir_verified(source: &Path, target: &Path) -> Result<()> {
    if !source.is_dir() {
        anyhow::bail!("本地技能来源不是目录: {:?}", source);
    }

    let source_doc = find_skill_md(source)
        .ok_or_else(|| anyhow::anyhow!("本地技能缺少 SKILL.md 或 skill.md: {:?}", source))?;
    let source_doc_size = std::fs::metadata(&source_doc)
        .with_context(|| format!("读取本地技能文件信息失败: {:?}", source_doc))?
        .len();
    if source_doc_size == 0 {
        anyhow::bail!("本地技能的 SKILL.md 为空: {:?}", source_doc);
    }

    let source_hash =
        hash_dir(source).with_context(|| format!("计算本地技能内容哈希失败: {:?}", source))?;
    let copy_result = copy_dir_recursive(source, target)
        .with_context(|| format!("复制本地技能失败: {:?} -> {:?}", source, target));

    let verification = copy_result.and_then(|_| {
        let target_doc = find_skill_md(target).ok_or_else(|| {
            anyhow::anyhow!("复制后的技能缺少 SKILL.md 或 skill.md: {:?}", target)
        })?;
        if std::fs::metadata(&target_doc)
            .with_context(|| format!("读取复制后的技能文件信息失败: {:?}", target_doc))?
            .len()
            == 0
        {
            anyhow::bail!("复制后的 SKILL.md 为空: {:?}", target_doc);
        }

        let target_hash = hash_dir(target)
            .with_context(|| format!("计算复制后的技能内容哈希失败: {:?}", target))?;
        if source_hash != target_hash {
            anyhow::bail!("本地技能复制校验失败: 源目录与中央目录内容不一致");
        }
        Ok(())
    });

    if verification.is_err() && std::fs::symlink_metadata(target).is_ok() {
        let _ = std::fs::remove_dir_all(target);
    }
    verification
}

#[cfg(test)]
mod local_copy_tests {
    use super::*;

    #[test]
    fn invalid_install_inputs_are_rejected_before_resolving_or_writing_user_directories() {
        for name in ["../victim", "/tmp/victim", "a\\b"] {
            let error = install_git_skill("missing-repository", Some(name.into()), None)
                .err()
                .unwrap();
            assert!(error.to_string().contains("目录名称"));
        }
        let error = install_git_skill_from_selection(
            "missing-repository",
            "../outside",
            Some("valid".into()),
        )
        .err()
        .unwrap();
        assert!(error.to_string().contains("子路径"));
    }

    #[cfg(unix)]
    #[test]
    fn discovery_does_not_follow_outside_links_or_accept_frontmatter_path_names() {
        let repository = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(
            repository.path().join("SKILL.md"),
            "---\nname: ../victim\n---\nbody",
        )
        .unwrap();
        std::fs::write(
            outside.path().join("SKILL.md"),
            "---\nname: outside\n---\nbody",
        )
        .unwrap();
        std::os::unix::fs::symlink(outside.path(), repository.path().join("linked")).unwrap();
        std::os::unix::fs::symlink(outside.path(), repository.path().join("skills")).unwrap();
        assert!(scan_git_skill_candidates(repository.path()).is_empty());
    }

    #[test]
    fn verified_local_copy_preserves_contents() {
        let source = tempfile::tempdir().unwrap();
        let destination_parent = tempfile::tempdir().unwrap();
        let destination = destination_parent.path().join("copied-skill");
        std::fs::create_dir(source.path().join("scripts")).unwrap();
        std::fs::write(
            source.path().join("SKILL.md"),
            "---\nname: copied-skill\ndescription: test\n---\n\n# Body\n",
        )
        .unwrap();
        std::fs::write(source.path().join("scripts/run.sh"), "echo test\n").unwrap();

        copy_local_skill_dir_verified(source.path(), &destination).unwrap();

        assert_eq!(
            hash_dir(source.path()).unwrap(),
            hash_dir(&destination).unwrap()
        );
    }

    #[test]
    fn verified_local_copy_rejects_empty_skill_doc_without_creating_target() {
        let source = tempfile::tempdir().unwrap();
        let destination_parent = tempfile::tempdir().unwrap();
        let destination = destination_parent.path().join("copied-skill");
        std::fs::write(source.path().join("SKILL.md"), "").unwrap();

        let error = copy_local_skill_dir_verified(source.path(), &destination).unwrap_err();

        assert!(error.to_string().contains("SKILL.md 为空"));
        assert!(!destination.exists());
    }
}

fn parse_skill_md_with_reason(path: &Path) -> Result<(String, Option<String>), &'static str> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|_| "read_failed")?;
    let mut text = String::new();
    file.take(4 * 1024 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(|_| "read_failed")?;
    if text.len() > 4 * 1024 * 1024 {
        return Err("document_too_large");
    }

    // Parse YAML frontmatter. Supports:
    //   - Standard: first line "---", fields, closing "---"
    //   - Relaxed: first line is a field, ends at "---" or non-field line
    //   - Multi-line block scalars: description: | (literal) / description: > (folded)
    //     indented continuation lines are joined into the previous field
    let mut name: Option<String> = None;
    let mut desc: Option<String> = None;
    let mut found_end = false;
    let raw_lines: Vec<&str> = text.lines().collect();

    let start_idx = if raw_lines.first().map(|v| v.trim()) == Some("---") {
        1
    } else {
        0
    };

    let mut i = start_idx;
    while i < raw_lines.len() {
        let l = raw_lines[i].trim();
        if l == "---" {
            found_end = true;
            break;
        }

        let (key, raw_value) = if let Some(v) = l.strip_prefix("name:") {
            ("name", v.trim().trim_matches('"').to_string())
        } else if let Some(v) = l.strip_prefix("description:") {
            ("description", v.trim().trim_matches('"').to_string())
        } else if start_idx == 0 && i == 0 {
            // Relaxed mode: first line may be a field — handled above
            i += 1;
            continue;
        } else if l.is_empty() || l.starts_with('#') {
            // Blank line or comment — skip
            i += 1;
            continue;
        } else {
            // Non-field line after start: end of header in relaxed mode
            break;
        };

        // Multi-line block scalar indicator: `|` (literal) or `>` (folded)
        if raw_value == "|" || raw_value == ">" {
            let mut block_lines: Vec<String> = Vec::new();
            i += 1;
            while i < raw_lines.len() {
                let next = raw_lines[i];
                if next.trim().is_empty() {
                    // A blank line ends the block scalar in standard YAML
                    // unless followed by indented continuation.
                    if i + 1 < raw_lines.len() && raw_lines[i + 1].starts_with([' ', '\t']) {
                        block_lines.push(String::new());
                        i += 1;
                        continue;
                    }
                    break;
                }
                if !next.starts_with([' ', '\t']) {
                    break;
                }
                // Strip the common indent (at least 2 spaces for block scalars)
                let trimmed = next.trim_start();
                block_lines.push(trimmed.to_string());
                i += 1;
            }
            let joined = if raw_value == "|" {
                block_lines.join("\n")
            } else {
                // Folded: lines joined by spaces, blank lines preserved as paragraph breaks
                let mut out = String::new();
                let mut first = true;
                let mut blank_run = 0;
                for line in &block_lines {
                    if line.is_empty() {
                        blank_run += 1;
                        continue;
                    }
                    if blank_run > 0 {
                        if !first {
                            out.push('\n');
                        }
                        out.push('\n');
                        blank_run = 0;
                    }
                    if !first && !out.ends_with('\n') {
                        out.push(' ');
                    }
                    out.push_str(line);
                    first = false;
                }
                out
            };
            let joined = joined.trim().to_string();
            if !joined.is_empty() {
                if key == "name" {
                    name = Some(joined);
                } else {
                    desc = Some(joined);
                }
            }
            continue;
        }

        if !raw_value.is_empty() {
            if key == "name" {
                name = Some(raw_value);
            } else {
                desc = Some(raw_value);
            }
        }
        i += 1;
    }

    // Relaxed mode: we accept the header once we've parsed at least one field.
    if !found_end && name.is_some() {
        found_end = true;
    }

    if !found_end {
        return Err("invalid_frontmatter");
    }
    let name = name.ok_or("missing_name")?;
    Ok((name, desc))
}
