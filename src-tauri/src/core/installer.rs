use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::central_repo::{ensure_central_repo, resolve_central_repo_path};
use super::content_hash::hash_dir;
use super::git_fetcher::clone_or_pull;
use super::github_download::{download_github_directory, parse_github_api_params};
use super::sync_engine::copy_dir_recursive;

pub struct InstallResult {
    pub skill_id: String,
    pub name: String,
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

/// Check if a directory is a valid skill (has SKILL.md/skill.md or is under .claude/skills/).
fn is_skill_dir(p: &Path) -> bool {
    p.is_dir() && (find_skill_md(p).is_some() || is_claude_skill_dir(p))
}

/// Check if a directory is a Claude plugin skill (under .claude/skills/ without SKILL.md).
fn is_claude_skill_dir(p: &Path) -> bool {
    if let Some(parent) = p.parent() {
        let parent_str = parent.to_string_lossy();
        if parent_str.ends_with(".claude/skills") || parent_str.ends_with(".claude\\skills") {
            return p.is_dir();
        }
    }
    false
}

/// Find SKILL.md or skill.md in a directory (case-insensitive, prefer uppercase).
fn find_skill_md(dir: &Path) -> Option<PathBuf> {
    let upper = dir.join("SKILL.md");
    if upper.exists() {
        return Some(upper);
    }
    let lower = dir.join("skill.md");
    if lower.exists() {
        return Some(lower);
    }
    None
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
            if !p.is_dir() {
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
        let base_dir = repo_dir.join(base);
        if !base_dir.exists() {
            continue;
        }
        if let Ok(rd) = std::fs::read_dir(&base_dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if !is_skill_dir(&p) {
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
    let parsed = parse_github_url(repo_url);
    let user_provided_name = name.is_some();

    // If URL has a /tree/ or /blob/ subpath, that IS the subpath
    let effective_subpath = subpath.map(|s| s.to_string()).or_else(|| parsed.subpath.clone());

    let skill_name = name.unwrap_or_else(|| {
        if let Some(sp) = &effective_subpath {
            if sp.as_str() == "." {
                derive_name_from_repo_url(&parsed.clone_url)
            } else {
                sp.rsplit('/')
                    .next()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| derive_name_from_repo_url(&parsed.clone_url))
            }
        } else {
            derive_name_from_repo_url(&parsed.clone_url)
        }
    });

    let central_dir = resolve_central_repo_path()?;
    ensure_central_repo(&central_dir)?;
    let mut central_path = central_dir.join(&skill_name);

    // 先删除已存在的目录（处理 APFS 删除延迟问题），不再先检查
    if central_path.exists() {
        std::fs::remove_dir_all(&central_path)
            .context(format!("failed to remove existing skill: {:?}", central_path))?;
        // 等待 APFS 完成删除操作
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // Fast path: for GitHub URLs with a subpath, download via API instead of cloning.
    let revision;
    if let Some((owner, repo, branch, sp)) = parse_github_api_params(
        &parsed.clone_url,
        parsed.branch.as_deref(),
        effective_subpath.as_deref(),
    ) {
        match download_github_directory(&owner, &repo, &branch, &sp, &central_path, None) {
            Ok(()) => {
                revision = format!("api-download-{}", branch);
            }
            Err(err) => {
                let _ = std::fs::remove_dir_all(&central_path);
                let err_msg = format!("{:#}", err);

                if err_msg.contains("404") || err_msg.contains("Not Found") {
                    anyhow::bail!(
                        "该 Skill 在 GitHub 上未找到（可能已被删除或路径已变更）。\n请检查链接是否正确：{}/tree/{}/{}",
                        parsed.clone_url.trim_end_matches(".git"),
                        branch,
                        sp
                    );
                }
                if err_msg.contains("RATE_LIMITED") {
                    anyhow::bail!(
                        "GitHub API 频率限制已触发。可在设置中配置 GitHub Token 以提升限额。"
                    );
                }
                if err_msg.contains("403") || err_msg.contains("Forbidden") {
                    anyhow::bail!("GitHub API 访问被拒绝（可能触发了频率限制）。请稍后再试。");
                }

                log::warn!(
                    "[installer] GitHub API download failed, falling back to git clone: {:#}",
                    err
                );
                let (repo_dir, rev) = clone_to_cache_with_ttl(&parsed.clone_url, parsed.branch.as_deref())?;
                let copy_src = if effective_subpath.as_deref() == Some(".") || effective_subpath.is_none() {
                    repo_dir.clone()
                } else {
                    repo_dir.join(effective_subpath.as_ref().unwrap())
                };
                if !copy_src.exists() {
                    anyhow::bail!("subpath not found in repo: {:?}", copy_src);
                }
                copy_dir_recursive(&copy_src, &central_path)
                    .with_context(|| format!("copy {:?} -> {:?}", copy_src, central_path))?;
                revision = rev;
            }
        }
    } else {
        // No /tree/ subpath in URL: clone and detect
        let (repo_dir, rev) = clone_to_cache_with_ttl(&parsed.clone_url, parsed.branch.as_deref())?;

        // If no explicit subpath, check for multi-skill
        if effective_subpath.is_none() {
            let candidates = scan_git_skill_candidates(&repo_dir);
            if candidates.len() >= 2 {
                anyhow::bail!(
                    "MULTI_SKILLS|该仓库包含 {} 个 Skills。请复制具体 Skill 文件夹链接（例如 GitHub 的 /tree/<branch>/<skill-folder>），再导入。",
                    candidates.len()
                );
            }
        }

        let copy_src = if let Some(sp) = &effective_subpath {
            if sp == "." {
                repo_dir.clone()
            } else {
                repo_dir.join(sp)
            }
        } else {
            repo_dir.clone()
        };

        if !copy_src.exists() {
            anyhow::bail!("path not found in repo: {:?}", copy_src);
        }
        copy_dir_recursive(&copy_src, &central_path)
            .with_context(|| format!("copy {:?} -> {:?}", copy_src, central_path))?;
        revision = rev;
    }

    // After download, prefer the name from SKILL.md over the derived name
    let (_description, md_name) = find_skill_md(&central_path)
        .and_then(|p| parse_skill_md(&p))
        .map(|(n, d)| (d, Some(n)))
        .unwrap_or((None, None));

    if !user_provided_name {
        if let Some(ref better_name) = md_name {
            if *better_name != skill_name {
                let new_central = central_dir.join(better_name);
                if !new_central.exists() {
                    std::fs::rename(&central_path, &new_central).with_context(|| {
                        format!("rename {:?} -> {:?}", central_path, new_central)
                    })?;
                    central_path = new_central;
                }
            }
        }
    }

    let content_hash = compute_content_hash(&central_path);
    let skill_id = format!("git-{}", Uuid::new_v4());

    Ok(InstallResult {
        skill_id,
        name: skill_name,
        central_path,
        content_hash,
        source_subpath: effective_subpath,
        source_revision: Some(revision),
    })
}

/// Install a specific skill from a Git URL (after user picks from list_git_skills).
pub fn install_git_skill_from_selection(
    repo_url: &str,
    subpath: &str,
    name: Option<String>,
) -> Result<InstallResult> {
    let parsed = parse_github_url(repo_url);
    let user_provided_name = name.is_some();

    let display_name = name.unwrap_or_else(|| {
        if subpath == "." {
            derive_name_from_repo_url(&parsed.clone_url)
        } else {
            subpath
                .rsplit('/')
                .next()
                .map(|s| s.to_string())
                .unwrap_or_else(|| derive_name_from_repo_url(&parsed.clone_url))
        }
    });

    let central_dir = resolve_central_repo_path()?;
    ensure_central_repo(&central_dir)?;
    let mut central_path = central_dir.join(&display_name);

    // 先删除已存在的目录（处理 APFS 删除延迟问题），不再先检查
    if central_path.exists() {
        std::fs::remove_dir_all(&central_path)
            .context(format!("failed to remove existing skill: {:?}", central_path))?;
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    let (repo_dir, revision) = clone_to_cache_with_ttl(&parsed.clone_url, parsed.branch.as_deref())?;

    let copy_src = if subpath == "." {
        repo_dir.clone()
    } else {
        repo_dir.join(subpath)
    };

    if !copy_src.exists() {
        anyhow::bail!("path not found in repo: {:?}", copy_src);
    }

    copy_dir_recursive(&copy_src, &central_path)
        .with_context(|| format!("copy {:?} -> {:?}", copy_src, central_path))?;

    // Prefer name from SKILL.md
    let (_description, md_name) = find_skill_md(&central_path)
        .and_then(|p| parse_skill_md(&p))
        .map(|(n, d)| (d, Some(n)))
        .unwrap_or((None, None));

    if !user_provided_name {
        if let Some(ref better_name) = md_name {
            if *better_name != display_name {
                let new_central = central_dir.join(better_name);
                if !new_central.exists() {
                    std::fs::rename(&central_path, &new_central).with_context(|| {
                        format!("rename {:?} -> {:?}", central_path, new_central)
                    })?;
                    central_path = new_central;
                }
            }
        }
    }

    let content_hash = compute_content_hash(&central_path);
    let skill_id = format!("git-{}", Uuid::new_v4());
    let source_subpath = if subpath == "." {
        None
    } else {
        Some(subpath.to_string())
    };

    Ok(InstallResult {
        skill_id,
        name: display_name,
        central_path,
        content_hash,
        source_subpath,
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

fn now_ms() -> i64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    now.as_millis() as i64
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RepoCacheMeta {
    last_fetched_ms: i64,
    head: Option<String>,
}

static GIT_CACHE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn get_git_cache_ttl_secs() -> u64 {
    std::env::var("MCP_MANAGER_GIT_CACHE_TTL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(3600)
}

fn clone_to_cache_with_ttl(clone_url: &str, branch: Option<&str>) -> Result<(PathBuf, String)> {
    let cache_root = std::env::temp_dir().join("ai-toolkit-git-cache");
    std::fs::create_dir_all(&cache_root)
        .with_context(|| format!("failed to create cache dir {:?}", cache_root))?;

    let repo_dir = cache_root.join(repo_cache_key(clone_url, branch));
    let meta_path = repo_dir.join(".ai-toolkit-cache.json");

    let lock = GIT_CACHE_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().unwrap_or_else(|err| err.into_inner());

    if repo_dir.join(".git").exists() {
        if let Ok(meta) = std::fs::read_to_string(&meta_path) {
            if let Ok(meta) = serde_json::from_str::<RepoCacheMeta>(&meta) {
                if let Some(head) = meta.head {
                    let ttl_ms = get_git_cache_ttl_secs().saturating_mul(1000) as i64;
                    if ttl_ms > 0 && now_ms().saturating_sub(meta.last_fetched_ms) < ttl_ms {
                        log::info!(
                            "[installer] git cache hit (fresh) url={} branch={:?} repo_dir={:?}",
                            clone_url,
                            branch,
                            repo_dir
                        );
                        return Ok((repo_dir, head));
                    }
                }
            }
        }
    }

    log::info!(
        "[installer] git cache miss/stale; fetching url={} branch={:?} repo_dir={:?}",
        clone_url,
        branch,
        repo_dir
    );

    let rev = clone_or_pull(clone_url, &repo_dir, branch).map_err(|err| {
        if repo_dir.exists() {
            let _ = std::fs::remove_dir_all(&repo_dir);
        }
        anyhow::anyhow!("{:#}", err)
    })?;

    let _ = std::fs::write(
        &meta_path,
        serde_json::to_string(&RepoCacheMeta {
            last_fetched_ms: now_ms(),
            head: Some(rev.clone()),
        })
        .unwrap_or_else(|_| "{}".to_string()),
    );

    Ok((repo_dir, rev))
}

fn repo_cache_key(clone_url: &str, branch: Option<&str>) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(clone_url.as_bytes());
    hasher.update(b"\n");
    if let Some(b) = branch {
        hasher.update(b.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn compute_content_hash(path: &Path) -> Option<String> {
    hash_dir(path).ok()
}

fn parse_skill_md(path: &Path) -> Option<(String, Option<String>)> {
    parse_skill_md_with_reason(path).ok()
}

fn parse_skill_md_with_reason(path: &Path) -> Result<(String, Option<String>), &'static str> {
    let text = std::fs::read_to_string(path).map_err(|_| "read_failed")?;
    let mut lines = text.lines();

    // Support two formats:
    // 1. Standard: first line is "---", fields follow, closing "---"
    // 2. Relaxed: first line is a field (e.g. "name: ..."), fields continue until "---" or end-of-header
    let mut name: Option<String> = None;
    let mut desc: Option<String> = None;
    let mut found_end = false;

    if lines.next().map(|v| v.trim()) == Some("---") {
        // Standard frontmatter
        for line in lines.by_ref() {
            let l = line.trim();
            if l == "---" {
                found_end = true;
                break;
            }
            if let Some(v) = l.strip_prefix("name:") {
                name = Some(v.trim().trim_matches('"').to_string());
            } else if let Some(v) = l.strip_prefix("description:") {
                desc = Some(v.trim().trim_matches('"').to_string());
            }
        }
    } else {
        // Relaxed: first line is a field, read until "---" or a non-field line
        for (i, line) in text.lines().enumerate() {
            let l = line.trim();
            if i > 0 && l == "---" {
                found_end = true;
                break;
            }
            if let Some(v) = l.strip_prefix("name:") {
                name = Some(v.trim().trim_matches('"').to_string());
            } else if let Some(v) = l.strip_prefix("description:") {
                desc = Some(v.trim().trim_matches('"').to_string());
            } else if i > 0 && !l.is_empty() && !l.contains(':') {
                // Non-field, non-empty line after fields -> end of header
                break;
            }
        }
        // In relaxed mode, finding fields is enough even without closing ---
        if name.is_some() {
            found_end = true;
        }
    }

    if !found_end {
        return Err("invalid_frontmatter");
    }
    let name = name.ok_or("missing_name")?;
    Ok((name, desc))
}
