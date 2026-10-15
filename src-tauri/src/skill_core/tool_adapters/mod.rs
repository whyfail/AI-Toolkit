use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// 支持 skills 同步的工具 ID；旧 TRAE Work ID 仅用于兼容。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolId {
    ClaudeCode,
    Codex,
    GeminiCli,
    OpenCode,
    Qoder,
    QoderCli,
    QwenCode,
    Trae,
    TraeCn,
    TraeWork,
    TraeSoloCn,
    CodeBuddy,
    Hermes,
    MimoCode,
    KimiCode,
    MinimaxCode,
    WorkBuddy,
    WorkBuddyCn,
    Zcode,
    Cursor,
    Windsurf,
    Cline,
    Roo,
    Kilo,
    Continue,
    Goose,
    Kiro,
}

impl serde::Serialize for ToolId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_key())
    }
}

/// 返回工具的唯一标识符（kebab-case，与 AppType serde 名保持一致）
impl ToolId {
    pub fn canonical(&self) -> Self {
        match self {
            Self::TraeWork => Self::Trae,
            Self::TraeSoloCn => Self::TraeCn,
            Self::WorkBuddyCn => Self::WorkBuddy,
            _ => self.clone(),
        }
    }

    pub fn as_key(&self) -> &'static str {
        match self {
            // 与 AppType 规范 ID 保持一致。
            ToolId::QwenCode => "qwen-code",
            ToolId::ClaudeCode => "claude",
            ToolId::Codex => "codex",
            ToolId::GeminiCli => "gemini",
            ToolId::OpenCode => "opencode",
            ToolId::Qoder => "qoder",
            ToolId::QoderCli => "qodercli", // Qoder CLI 使用 qodercli 作为 ID
            ToolId::Trae | ToolId::TraeWork => "trae",
            ToolId::TraeCn | ToolId::TraeSoloCn => "trae-cn",
            ToolId::CodeBuddy => "codebuddy",
            ToolId::Hermes => "hermes",
            ToolId::MimoCode => "mimo-code",
            ToolId::KimiCode => "kimi-code",
            ToolId::MinimaxCode => "minimax-code",
            ToolId::WorkBuddy | ToolId::WorkBuddyCn => "workbuddy",
            ToolId::Zcode => "zcode",
            ToolId::Cursor => "cursor",
            ToolId::Windsurf => "windsurf",
            ToolId::Cline => "cline",
            ToolId::Roo => "roo",
            ToolId::Kilo => "kilo",
            ToolId::Continue => "continue",
            ToolId::Goose => "goose",
            ToolId::Kiro => "kiro",
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ToolAdapter {
    pub id: ToolId,
    pub display_name: &'static str,
    /// Global skill directory under user home (aligned with add-skill docs).
    pub relative_skills_dir: &'static str,
    /// Directory used to detect whether the tool is installed (aligned with add-skill docs).
    pub relative_detect_dir: &'static str,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct DetectedSkill {
    pub tool: ToolId,
    pub name: String,
    pub path: PathBuf,
    pub is_link: bool,
    pub link_target: Option<PathBuf>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ToolStatus {
    pub tool: ToolAdapter,
    pub installed: bool,
    pub skills: Vec<DetectedSkill>,
    /// Previous Windows locations are import sources, never current sync targets.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub legacy_skills: Vec<DetectedSkill>,
}

/// 与 MCP 共用产品 ID 和安装状态，客户端共享配置时只保留一个适配器。
pub fn default_tool_adapters() -> Vec<ToolAdapter> {
    vec![
        ToolAdapter {
            id: ToolId::QwenCode,
            display_name: "Qwen Code",
            relative_skills_dir: ".qwen/skills",
            relative_detect_dir: ".qwen",
        },
        ToolAdapter {
            id: ToolId::ClaudeCode,
            display_name: "Claude Code",
            relative_skills_dir: ".claude/skills",
            relative_detect_dir: ".claude",
        },
        ToolAdapter {
            id: ToolId::Codex,
            display_name: "Codex",
            relative_skills_dir: ".codex/skills",
            relative_detect_dir: ".codex",
        },
        ToolAdapter {
            id: ToolId::GeminiCli,
            display_name: "Gemini CLI",
            relative_skills_dir: ".gemini/skills",
            relative_detect_dir: ".gemini",
        },
        ToolAdapter {
            id: ToolId::OpenCode,
            display_name: "OpenCode",
            relative_skills_dir: ".config/opencode/skills",
            relative_detect_dir: ".config/opencode",
        },
        ToolAdapter {
            id: ToolId::Qoder,
            display_name: "Qoder",
            relative_skills_dir: ".qoder/skills",
            relative_detect_dir: ".qoder",
        },
        ToolAdapter {
            id: ToolId::QoderCli,
            display_name: "Qoder CLI",
            // NOTE: Qoder CLI 和 Qoder 使用相同的 skills 目录
            relative_skills_dir: ".qoder/skills",
            relative_detect_dir: ".qoder",
        },
        ToolAdapter {
            id: ToolId::Trae,
            display_name: "TRAE",
            relative_skills_dir: ".trae/skills",
            relative_detect_dir: ".trae",
        },
        ToolAdapter {
            id: ToolId::TraeCn,
            display_name: "TRAE CN",
            relative_skills_dir: ".trae-cn/skills",
            relative_detect_dir: ".trae-cn",
        },
        ToolAdapter {
            id: ToolId::CodeBuddy,
            display_name: "CodeBuddy CN CLI",
            relative_skills_dir: ".codebuddy/skills",
            relative_detect_dir: ".codebuddy",
        },
        ToolAdapter {
            id: ToolId::Hermes,
            display_name: "Hermes Agent",
            relative_skills_dir: ".hermes/skills",
            relative_detect_dir: ".hermes",
        },
        ToolAdapter {
            id: ToolId::MimoCode,
            display_name: "Mimo Code",
            relative_skills_dir: ".config/mimocode/skills",
            relative_detect_dir: ".config/mimocode",
        },
        ToolAdapter {
            id: ToolId::KimiCode,
            display_name: "Kimi Code",
            relative_skills_dir: ".kimi-code/skills",
            relative_detect_dir: ".kimi-code",
        },
        ToolAdapter {
            id: ToolId::MinimaxCode,
            display_name: "MiniMax Code",
            relative_skills_dir: ".minimax/skills",
            relative_detect_dir: ".minimax",
        },
        ToolAdapter {
            id: ToolId::WorkBuddy,
            display_name: "WorkBuddy",
            relative_skills_dir: ".workbuddy/skills",
            relative_detect_dir: ".workbuddy",
        },
        ToolAdapter {
            id: ToolId::Zcode,
            display_name: "ZCode",
            relative_skills_dir: ".zcode/skills",
            relative_detect_dir: ".zcode",
        },
        ToolAdapter {
            id: ToolId::Cursor,
            display_name: "Cursor",
            relative_skills_dir: ".cursor/skills",
            relative_detect_dir: ".cursor",
        },
        ToolAdapter {
            id: ToolId::Windsurf,
            display_name: "Windsurf",
            relative_skills_dir: ".codeium/windsurf/skills",
            relative_detect_dir: ".codeium/windsurf",
        },
        ToolAdapter {
            id: ToolId::Cline,
            display_name: "Cline",
            relative_skills_dir: ".cline/skills",
            relative_detect_dir: ".cline",
        },
        ToolAdapter {
            id: ToolId::Roo,
            display_name: "Roo Code",
            relative_skills_dir: ".roo/skills",
            relative_detect_dir: ".roo",
        },
        ToolAdapter {
            id: ToolId::Kilo,
            display_name: "Kilo Code",
            relative_skills_dir: ".kilo/skills",
            relative_detect_dir: ".config/kilo",
        },
        ToolAdapter {
            id: ToolId::Continue,
            display_name: "Continue",
            relative_skills_dir: ".continue/skills",
            relative_detect_dir: ".continue",
        },
        ToolAdapter {
            id: ToolId::Goose,
            display_name: "Goose",
            relative_skills_dir: ".config/goose/skills",
            relative_detect_dir: ".config/goose",
        },
        ToolAdapter {
            id: ToolId::Kiro,
            display_name: "Kiro",
            relative_skills_dir: ".kiro/skills",
            relative_detect_dir: ".kiro",
        },
    ]
}

/// Tools can share the same global skills directory (e.g. Amp and Kimi Code CLI).
/// Use this to coordinate UI warnings and avoid duplicate filesystem operations.
pub fn adapters_sharing_skills_dir(adapter: &ToolAdapter) -> Vec<ToolAdapter> {
    let adapter_path = resolve_default_path(adapter).ok();
    default_tool_adapters()
        .into_iter()
        .filter(|a| resolve_default_path(a).ok() == adapter_path)
        .collect()
}

pub fn adapter_by_key(key: &str) -> Option<ToolAdapter> {
    let key = match key {
        "trae-work" => "trae",
        "trae-solo-cn" => "trae-cn",
        "workbuddy-cn" => "workbuddy",
        "codex-cli" | "codex-desktop" => "codex",
        _ => key,
    };
    default_tool_adapters()
        .into_iter()
        .find(|adapter| adapter.id.as_key() == key)
}

pub fn resolve_default_path(adapter: &ToolAdapter) -> Result<PathBuf> {
    let home = dirs::home_dir().context("failed to resolve home directory")?;
    match adapter.id.canonical() {
        ToolId::Codex => {
            return Ok(std::env::var_os("CODEX_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".codex"))
                .join("skills"))
        }
        ToolId::Cline => {
            return Ok(std::env::var_os("CLINE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".cline"))
                .join("skills"))
        }
        ToolId::Goose => {
            let config = crate::agents::get_canonical_mcp_config_path(&crate::mcp::AppType::Goose);
            return Ok(config
                .parent()
                .context("Goose config has no parent directory")?
                .join("skills"));
        }
        ToolId::KimiCode | ToolId::MinimaxCode => {
            let app = adapter
                .id
                .as_key()
                .parse::<crate::mcp::AppType>()
                .map_err(anyhow::Error::msg)?;
            let config = crate::agents::get_canonical_mcp_config_path(&app);
            return Ok(config
                .parent()
                .context("Agent config has no parent directory")?
                .join("skills"));
        }
        ToolId::OpenCode => return Ok(crate::agents::xdg_config_root().join("opencode/skills")),
        ToolId::Trae => return Ok(home.join(".trae").join("skills")),
        ToolId::TraeCn => return Ok(home.join(".trae-cn").join("skills")),
        _ => {}
    }
    #[cfg(windows)]
    {
        let local_appdata = std::env::var("LOCALAPPDATA").ok().map(PathBuf::from);
        let path = match adapter.id {
            ToolId::OpenCode => home.join(".config").join("opencode").join("skills"),
            ToolId::Qoder | ToolId::QoderCli => local_appdata
                .unwrap_or_else(|| home.join("AppData").join("Local"))
                .join("Qoder")
                .join("skills"),
            _ => home.join(adapter.relative_skills_dir),
        };
        return Ok(path);
    }

    #[cfg(not(windows))]
    Ok(home.join(adapter.relative_skills_dir))
}

fn skill_read_path_candidates(
    adapter: &ToolAdapter,
    canonical: PathBuf,
    windows_appdata: Option<&Path>,
) -> Vec<PathBuf> {
    let mut paths = vec![canonical];
    if let Some(home) = dirs::home_dir() {
        if matches!(
            adapter.id.canonical(),
            ToolId::Codex
                | ToolId::Cline
                | ToolId::Kilo
                | ToolId::Goose
                | ToolId::KimiCode
                | ToolId::MinimaxCode
        ) {
            paths.push(home.join(".agents/skills"));
        }
        if adapter.id == ToolId::Goose {
            paths.push(crate::agents::xdg_config_root().join("goose/skills"));
            if cfg!(target_os = "macos") {
                paths.push(home.join("Library/Application Support/Block/goose/skills"));
            }
        }
        if adapter.id == ToolId::Kilo {
            paths.push(home.join(".kilocode/skills"));
        }
        if adapter.id == ToolId::Windsurf {
            paths.push(crate::agents::xdg_config_root().join("devin/skills"));
        }
    }
    if let Some(appdata) = windows_appdata {
        let legacy_directory = match adapter.id.canonical() {
            ToolId::Trae => Some("Trae"),
            ToolId::TraeCn => Some("Trae CN"),
            _ => None,
        };
        if let Some(directory) = legacy_directory {
            paths.push(appdata.join(directory).join("skills"));
        }
    }
    let mut seen = std::collections::HashSet::new();
    paths
        .into_iter()
        .filter(|path| seen.insert(crate::agents::normalized_path_key(path)))
        .collect()
}

/// Read-only discovery: writes and removals must keep using resolve_default_path.
pub fn resolve_read_paths(adapter: &ToolAdapter) -> Result<Vec<PathBuf>> {
    let canonical = resolve_default_path(adapter)?;
    #[cfg(windows)]
    let windows_appdata = Some(
        std::env::var("APPDATA").ok().map(PathBuf::from).unwrap_or(
            dirs::home_dir()
                .context("failed to resolve home directory")?
                .join("AppData")
                .join("Roaming"),
        ),
    );
    #[cfg(not(windows))]
    let windows_appdata: Option<PathBuf> = None;
    Ok(skill_read_path_candidates(
        adapter,
        canonical,
        windows_appdata.as_deref(),
    ))
}

pub fn resolve_detect_path(adapter: &ToolAdapter) -> Result<PathBuf> {
    let home = dirs::home_dir().context("failed to resolve home directory")?;
    match adapter.id.canonical() {
        ToolId::Trae => return Ok(home.join(".trae")),
        ToolId::TraeCn => return Ok(home.join(".trae-cn")),
        _ => {}
    }
    #[cfg(windows)]
    {
        let local_appdata = std::env::var("LOCALAPPDATA").ok().map(PathBuf::from);
        let path = match adapter.id {
            ToolId::OpenCode => home.join(".config").join("opencode"),
            ToolId::Qoder | ToolId::QoderCli => local_appdata
                .unwrap_or_else(|| home.join("AppData").join("Local"))
                .join("Qoder"),
            _ => home.join(adapter.relative_detect_dir),
        };
        return Ok(path);
    }

    #[cfg(not(windows))]
    Ok(home.join(adapter.relative_detect_dir))
}

/// MCP 与 Skills 使用同一个本地安装检测器。
pub fn is_tool_installed(adapter: &ToolAdapter) -> bool {
    adapter
        .id
        .as_key()
        .parse::<crate::mcp::AppType>()
        .map(|app| crate::services::local_detection::is_agent_installed(&app))
        .unwrap_or(false)
}

pub fn scan_tool_dir(tool: &ToolAdapter, dir: &Path) -> Result<Vec<DetectedSkill>> {
    let mut results = Vec::new();
    if !dir.exists() {
        return Ok(results);
    }

    let ignore_hint = "Application Support/com.tauri.dev/skills";

    for entry in std::fs::read_dir(dir).with_context(|| format!("read dir {:?}", dir))? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        let is_dir = file_type.is_dir() || (file_type.is_symlink() && path.is_dir());
        if !is_dir {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if tool.id == ToolId::Codex && name == ".system" {
            continue;
        }
        let (is_link, link_target) = detect_link(&path);
        if path.to_string_lossy().contains(ignore_hint)
            || link_target
                .as_ref()
                .map(|p| p.to_string_lossy().contains(ignore_hint))
                .unwrap_or(false)
        {
            continue;
        }
        results.push(DetectedSkill {
            tool: tool.id.clone(),
            name,
            path,
            is_link,
            link_target,
        });
    }

    Ok(results)
}

fn detect_link(path: &Path) -> (bool, Option<PathBuf>) {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let target = std::fs::read_link(path).ok();
            (true, target)
        }
        _ => {
            let target = std::fs::read_link(path).ok();
            if target.is_some() {
                (true, target)
            } else {
                (false, None)
            }
        }
    }
}

pub fn get_all_tool_status() -> Result<Vec<ToolStatus>> {
    get_tool_status_for_agents(&crate::agents::detect_all_agents())
}

pub fn get_tool_status_for_agents(
    agents: &[crate::agents::DetectedAgent],
) -> Result<Vec<ToolStatus>> {
    let installed: std::collections::HashSet<_> = agents
        .iter()
        .filter(|agent| agent.exists)
        .map(|agent| agent.app_type.name())
        .collect();
    default_tool_adapters()
        .into_iter()
        .filter(|tool| installed.contains(tool.id.as_key()))
        .map(|tool| {
            let paths = resolve_read_paths(&tool)?;
            tool_status_from_read_paths(tool, true, &paths)
        })
        .collect()
}

fn tool_status_from_read_paths(
    tool: ToolAdapter,
    installed: bool,
    paths: &[PathBuf],
) -> Result<ToolStatus> {
    let mut skills = Vec::new();
    let mut legacy_skills = Vec::new();
    let mut seen_dirs = std::collections::HashSet::new();
    let mut seen_skills = std::collections::HashSet::new();
    for (index, path) in paths.iter().enumerate() {
        if !seen_dirs.insert(crate::agents::normalized_path_key(path)) {
            continue;
        }
        let discovered = scan_tool_dir(&tool, path)?
            .into_iter()
            .filter(|skill| seen_skills.insert(crate::agents::normalized_path_key(&skill.path)));
        if index == 0 || !matches!(tool.id.canonical(), ToolId::Trae | ToolId::TraeCn) {
            skills.extend(discovered);
        } else {
            legacy_skills.extend(discovered);
        }
    }
    Ok(ToolStatus {
        tool,
        installed,
        skills,
        legacy_skills,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_and_skills_registries_share_products_and_empty_installations_do_not_scan_directories() {
        let mcp: std::collections::HashSet<_> = crate::mcp::AppType::all()
            .iter()
            .map(|app| app.name().to_string())
            .collect();
        let skills: std::collections::HashSet<_> = default_tool_adapters()
            .iter()
            .map(|adapter| adapter.id.as_key().to_string())
            .collect();
        assert_eq!(mcp, skills);
        assert!(get_tool_status_for_agents(&[]).unwrap().is_empty());
        assert_eq!(
            adapter_by_key("workbuddy-cn").unwrap().id,
            ToolId::WorkBuddy
        );
        assert_eq!(adapter_by_key("codex-desktop").unwrap().id, ToolId::Codex);
        let goose = adapter_by_key("goose").unwrap();
        let config = crate::agents::get_canonical_mcp_config_path(&crate::mcp::AppType::Goose);
        assert_eq!(
            resolve_default_path(&goose).unwrap(),
            config.parent().unwrap().join("skills")
        );
    }

    #[test]
    fn windows_legacy_skill_read_paths_are_region_specific_and_home_first() {
        let international = adapter_by_key("trae-work").unwrap();
        let china = adapter_by_key("trae-solo-cn").unwrap();
        let home = PathBuf::from("C:/Users/Example");
        let appdata = home.join("AppData/Roaming");
        assert_eq!(
            skill_read_path_candidates(&international, home.join(".trae/skills"), Some(&appdata)),
            vec![home.join(".trae/skills"), appdata.join("Trae/skills")]
        );
        assert_eq!(
            skill_read_path_candidates(&china, home.join(".trae-cn/skills"), Some(&appdata)),
            vec![home.join(".trae-cn/skills"), appdata.join("Trae CN/skills")]
        );
        assert_eq!(
            skill_read_path_candidates(&international, home.join(".trae/skills"), None),
            vec![home.join(".trae/skills")]
        );
        let other = adapter_by_key("codex").unwrap();
        assert_eq!(
            skill_read_path_candidates(&other, home.join(".codex/skills"), Some(&appdata)),
            vec![
                home.join(".codex/skills"),
                dirs::home_dir().unwrap().join(".agents/skills")
            ]
        );
    }

    #[test]
    fn legacy_skill_discovery_keeps_current_targets_and_installation_state_separate() {
        let directory = tempfile::tempdir().unwrap();
        let tool = adapter_by_key("trae-cn").unwrap();
        let canonical = directory.path().join("home/.trae-cn/skills");
        let appdata = directory.path().join("AppData/Roaming");
        let legacy = appdata.join("Trae CN/skills");
        std::fs::create_dir_all(canonical.join("current-only")).unwrap();
        std::fs::create_dir_all(legacy.join("legacy-only")).unwrap();
        std::fs::write(canonical.join("current-only/SKILL.md"), "# Current").unwrap();
        std::fs::write(legacy.join("legacy-only/SKILL.md"), "# Legacy").unwrap();
        let paths = skill_read_path_candidates(&tool, canonical.clone(), Some(&appdata));
        let status = tool_status_from_read_paths(tool.clone(), false, &paths).unwrap();
        assert!(!status.installed);
        assert_eq!(status.skills.len(), 1);
        assert_eq!(status.skills[0].name, "current-only");
        assert_eq!(status.legacy_skills.len(), 1);
        assert_eq!(status.legacy_skills[0].name, "legacy-only");
        assert_eq!(status.legacy_skills[0].path, legacy.join("legacy-only"));
        assert!(serde_json::to_value(&status)
            .unwrap()
            .get("legacy_skills")
            .is_some());
        let current_only = tool_status_from_read_paths(tool, false, &[canonical]).unwrap();
        assert!(serde_json::to_value(&current_only)
            .unwrap()
            .get("legacy_skills")
            .is_none());
        assert!(legacy.join("legacy-only/SKILL.md").exists());
    }

    #[cfg(unix)]
    #[test]
    fn legacy_skill_paths_equivalent_to_current_are_not_reported_twice() {
        let directory = tempfile::tempdir().unwrap();
        let tool = adapter_by_key("trae").unwrap();
        let canonical = directory.path().join("home/.trae/skills");
        let appdata = directory.path().join("AppData/Roaming");
        std::fs::create_dir_all(canonical.join("shared")).unwrap();
        std::fs::create_dir_all(appdata.join("Trae")).unwrap();
        std::os::unix::fs::symlink(&canonical, appdata.join("Trae/skills")).unwrap();
        let paths = skill_read_path_candidates(&tool, canonical.clone(), Some(&appdata));
        assert_eq!(paths, vec![canonical.clone()]);
        let status =
            tool_status_from_read_paths(tool, false, &[canonical, appdata.join("Trae/skills")])
                .unwrap();
        assert_eq!(status.skills.len(), 1);
        assert!(status.legacy_skills.is_empty());
    }

    #[test]
    fn legacy_trae_skill_ids_resolve_to_one_global_directory_per_region() {
        let international = adapter_by_key("trae").unwrap();
        let international_legacy = adapter_by_key("trae-work").unwrap();
        let china = adapter_by_key("trae-cn").unwrap();
        let china_legacy = adapter_by_key("trae-solo-cn").unwrap();
        assert_eq!(international_legacy.id, ToolId::Trae);
        assert_eq!(china_legacy.id, ToolId::TraeCn);
        let home = dirs::home_dir().unwrap();
        assert_eq!(
            resolve_default_path(&international).unwrap(),
            home.join(".trae/skills")
        );
        assert_eq!(
            resolve_default_path(&china).unwrap(),
            home.join(".trae-cn/skills")
        );
        assert_eq!(
            resolve_default_path(&international_legacy).unwrap(),
            resolve_default_path(&international).unwrap()
        );
        assert_eq!(
            resolve_default_path(&china_legacy).unwrap(),
            resolve_default_path(&china).unwrap()
        );
        assert_eq!(default_tool_adapters().len(), 24);
        assert_eq!(adapters_sharing_skills_dir(&international).len(), 1);
        assert_eq!(adapters_sharing_skills_dir(&china).len(), 1);
    }

    #[test]
    fn kimi_and_minimax_keep_private_sync_targets_and_discover_shared_skills() {
        for key in ["kimi-code", "minimax-code"] {
            let tool = adapter_by_key(key).unwrap();
            let app = key.parse::<crate::mcp::AppType>().unwrap();
            let config = crate::agents::get_canonical_mcp_config_path(&app);
            let target = resolve_default_path(&tool).unwrap();
            assert_eq!(target, config.parent().unwrap().join("skills"));
            assert_ne!(target, dirs::home_dir().unwrap().join(".agents/skills"));
            let candidates = skill_read_path_candidates(&tool, target, None);
            assert!(candidates.contains(&dirs::home_dir().unwrap().join(".agents/skills")));

            let directory = tempfile::tempdir().unwrap();
            let skills = directory.path().join("skills");
            std::fs::create_dir_all(skills.join("example")).unwrap();
            std::fs::write(skills.join("example/SKILL.md"), "# Example").unwrap();
            let status = tool_status_from_read_paths(tool, false, &[skills]).unwrap();
            assert!(!status.installed);
            assert_eq!(status.skills.len(), 1);
            assert_eq!(status.skills[0].name, "example");
        }
    }

    #[test]
    fn workbuddy_editions_share_skills_directory_and_not_installation_fallback() {
        let international = adapter_by_key("workbuddy").expect("international adapter");
        let china = adapter_by_key("workbuddy-cn").expect("China adapter");

        assert_eq!(international.relative_skills_dir, ".workbuddy/skills");
        assert_eq!(china.relative_skills_dir, ".workbuddy/skills");
        assert_eq!(
            resolve_default_path(&international).expect("international path"),
            resolve_default_path(&china).expect("China path")
        );
    }

    #[test]
    fn workbuddy_shared_directory_group_contains_both_editions() {
        let international = adapter_by_key("workbuddy").expect("international adapter");
        let shared = adapters_sharing_skills_dir(&international);
        let ids: Vec<&str> = shared.iter().map(|adapter| adapter.id.as_key()).collect();
        assert!(ids.contains(&"workbuddy"));
        assert_eq!(ids, vec!["workbuddy"]);
    }
}
