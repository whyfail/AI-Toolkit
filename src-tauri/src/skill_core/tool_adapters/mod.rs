use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// 支持 skills 同步的工具 ID（14 种）
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
    pub fn as_key(&self) -> &'static str {
        match self {
            // 支持的 14 种工具（与 README 和 AppType 保持一致）
            ToolId::QwenCode => "qwen-code",
            ToolId::ClaudeCode => "claude",
            ToolId::Codex => "codex",
            ToolId::GeminiCli => "gemini",
            ToolId::OpenCode => "opencode",
            ToolId::Qoder => "qoder",
            ToolId::QoderCli => "qodercli", // Qoder CLI 使用 qodercli 作为 ID
            ToolId::Trae => "trae",
            ToolId::TraeCn => "trae-cn",
            ToolId::TraeWork => "trae-work",
            ToolId::TraeSoloCn => "trae-solo-cn", // TRAE Work CN 沿用 trae-solo-cn 作为兼容 ID
            ToolId::CodeBuddy => "codebuddy",
            ToolId::Hermes => "hermes",
            ToolId::MimoCode => "mimo-code",
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
}

/// 支持的工具列表（与 README 保持一致，共 14 种）
/// MCP 服务器管理支持的工具: Qwen Code, Claude Code, Codex, Gemini CLI, OpenCode,
/// Qoder, Qoder CLI, TRAE IDE, TRAE IDE CN, TRAE Work, TRAE Work CN, CodeBuddy, Hermes Agent, Mimo Code
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
            display_name: "TRAE IDE",
            relative_skills_dir: ".trae/skills",
            relative_detect_dir: ".trae",
        },
        ToolAdapter {
            id: ToolId::TraeCn,
            display_name: "TRAE IDE CN",
            relative_skills_dir: ".trae-cn/skills",
            relative_detect_dir: ".trae-cn",
        },
        ToolAdapter {
            id: ToolId::TraeWork,
            display_name: "TRAE Work",
            relative_skills_dir: ".trae/skills",
            relative_detect_dir: ".trae",
        },
        ToolAdapter {
            id: ToolId::TraeSoloCn,
            display_name: "TRAE Work CN",
            // NOTE: TRAE Work CN 和 Trae CN 使用相同的 skills 目录
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
    default_tool_adapters()
        .into_iter()
        .find(|adapter| adapter.id.as_key() == key)
}

pub fn resolve_default_path(adapter: &ToolAdapter) -> Result<PathBuf> {
    let home = dirs::home_dir().context("failed to resolve home directory")?;
    #[cfg(windows)]
    {
        let appdata = std::env::var("APPDATA").ok().map(PathBuf::from);
        let local_appdata = std::env::var("LOCALAPPDATA").ok().map(PathBuf::from);
        let path = match adapter.id {
            ToolId::OpenCode => home.join(".config").join("opencode").join("skills"),
            ToolId::Qoder | ToolId::QoderCli => local_appdata
                .unwrap_or_else(|| home.join("AppData").join("Local"))
                .join("Qoder")
                .join("skills"),
            ToolId::Trae => appdata
                .unwrap_or_else(|| home.join("AppData").join("Roaming"))
                .join("Trae")
                .join("skills"),
            ToolId::TraeCn | ToolId::TraeSoloCn => appdata
                .unwrap_or_else(|| home.join("AppData").join("Roaming"))
                .join("Trae CN")
                .join("skills"),
            _ => home.join(adapter.relative_skills_dir),
        };
        return Ok(path);
    }

    #[cfg(not(windows))]
    Ok(home.join(adapter.relative_skills_dir))
}

pub fn resolve_detect_path(adapter: &ToolAdapter) -> Result<PathBuf> {
    let home = dirs::home_dir().context("failed to resolve home directory")?;
    #[cfg(windows)]
    {
        let appdata = std::env::var("APPDATA").ok().map(PathBuf::from);
        let local_appdata = std::env::var("LOCALAPPDATA").ok().map(PathBuf::from);
        let path = match adapter.id {
            ToolId::OpenCode => home.join(".config").join("opencode"),
            ToolId::Qoder | ToolId::QoderCli => local_appdata
                .unwrap_or_else(|| home.join("AppData").join("Local"))
                .join("Qoder"),
            ToolId::Trae => appdata
                .unwrap_or_else(|| home.join("AppData").join("Roaming"))
                .join("Trae"),
            ToolId::TraeCn | ToolId::TraeSoloCn => appdata
                .unwrap_or_else(|| home.join("AppData").join("Roaming"))
                .join("Trae CN"),
            _ => home.join(adapter.relative_detect_dir),
        };
        return Ok(path);
    }

    #[cfg(not(windows))]
    Ok(home.join(adapter.relative_detect_dir))
}

/// 获取 ToolId 对应的 CLI binary 名称（仅支持 skills 模块的 14 种工具）
fn get_tool_binary_name(id: &ToolId) -> &'static str {
    match id {
        ToolId::ClaudeCode => "claude",
        ToolId::Codex => "codex",
        ToolId::GeminiCli => "gemini",
        ToolId::OpenCode => "opencode",
        ToolId::Qoder => "qoder",
        ToolId::QoderCli => "qodercli",
        ToolId::QwenCode => "qwen",
        ToolId::Trae => "trae",
        ToolId::TraeCn => "trae-cn",
        ToolId::TraeWork => "trae",
        ToolId::TraeSoloCn => "trae-solo-cn",
        ToolId::CodeBuddy => "codebuddy",
        ToolId::Hermes => "hermes",
        ToolId::MimoCode => "mimo",
    }
}

/// 使用 which_binary 检测工具是否已安装
pub fn is_tool_installed_by_binary(id: &ToolId) -> bool {
    use crate::services::tool_manager::which_binary;
    let binary_name = get_tool_binary_name(id);
    which_binary(binary_name).is_some()
}

/// 使用 binary 检测工具是否已安装（接受 ToolAdapter）
pub fn is_tool_installed(adapter: &ToolAdapter) -> bool {
    // Primary: binary 检测（最可靠，CLI 工具卸载后 binary 会被移除）
    if is_tool_installed_by_binary(&adapter.id) {
        return true;
    }

    // Mac GUI 应用检测（通过检查 /Applications/*.app）
    #[cfg(target_os = "macos")]
    {
        let app = match adapter.id {
            ToolId::OpenCode => Some(crate::mcp::AppType::OpenCode),
            ToolId::Trae => Some(crate::mcp::AppType::Trae),
            ToolId::TraeCn => Some(crate::mcp::AppType::TraeCn),
            ToolId::TraeWork => Some(crate::mcp::AppType::TraeWork),
            ToolId::TraeSoloCn => Some(crate::mcp::AppType::TraeSoloCn),
            ToolId::Qoder => Some(crate::mcp::AppType::Qoder),
            _ => None,
        };
        if let Some(app) = app {
            if crate::services::tool_manager::is_app_installed_mac(&app) {
                return true;
            }
        }
    }

    // Windows GUI 应用检测
    #[cfg(target_os = "windows")]
    {
        let app = match adapter.id {
            ToolId::OpenCode => Some(crate::mcp::AppType::OpenCode),
            ToolId::Trae => Some(crate::mcp::AppType::Trae),
            ToolId::TraeCn => Some(crate::mcp::AppType::TraeCn),
            ToolId::TraeWork => Some(crate::mcp::AppType::TraeWork),
            ToolId::TraeSoloCn => Some(crate::mcp::AppType::TraeSoloCn),
            ToolId::Qoder => Some(crate::mcp::AppType::Qoder),
            _ => None,
        };
        if let Some(app) = app {
            if crate::services::tool_manager::is_app_installed_windows(&app) {
                return true;
            }
        }
    }

    // 数据目录检测已移除
    // 原因: CLI 工具卸载后数据目录仍会残留，导致误判为已安装
    // 对于纯 GUI 应用（Trae/TraeCn/Qoder），通过 /Applications/*.app 检测已足够

    false
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
    let mut tool_statuses = Vec::new();

    for tool in default_tool_adapters() {
        // 使用 binary 检测工具是否已安装
        let installed = is_tool_installed(&tool);
        let skills_dir = resolve_default_path(&tool)?;
        let skills = scan_tool_dir(&tool, &skills_dir)?;

        tool_statuses.push(ToolStatus {
            tool,
            installed,
            skills,
        });
    }

    Ok(tool_statuses)
}
