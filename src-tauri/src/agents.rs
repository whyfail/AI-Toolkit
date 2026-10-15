use crate::services::local_detection::is_agent_installed;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::mcp::AppType;

/// 检测到的 Agent 工具信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedAgent {
    pub app_type: AppType,
    pub name: String,
    pub config_path: String,
    pub exists: bool,
    pub mcp_count: usize,
}

/// 解析路径，支持 ~ 和 Windows 环境变量 (如 %USERPROFILE%)
pub fn resolve_path(path_str: &str) -> PathBuf {
    let mut path = path_str.to_string();

    // Handle Windows Environment Variables like %USERPROFILE% or %APPDATA%
    if cfg!(windows) {
        let mut result = String::new();
        let mut chars = path.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '%' {
                let mut var_name = String::new();
                while let Some(&next_c) = chars.peek() {
                    if next_c == '%' {
                        chars.next(); // consume closing %
                        break;
                    }
                    var_name.push(next_c);
                    chars.next();
                }
                if let Ok(val) = std::env::var(&var_name) {
                    result.push_str(&val);
                } else {
                    result.push('%');
                    result.push_str(&var_name);
                    result.push('%');
                }
            } else {
                result.push(c);
            }
        }
        path = result;
    }

    // Handle ~ (Home Directory)
    if path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            let home_str = home.to_string_lossy().to_string();
            path = home_str + &path[1..];
        }
    }

    PathBuf::from(path)
}

/// Stable key for deduplicating paths that may be spelled differently.
pub fn normalized_path_key(path: &Path) -> String {
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let key = resolved.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        key.to_ascii_lowercase()
    } else {
        key
    }
}

pub(crate) fn xdg_config_root() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| resolve_path("~/.config"))
}

fn extension_config_paths(ids: &[&str], filename: &str) -> Vec<PathBuf> {
    let base = if cfg!(target_os = "macos") {
        resolve_path("~/Library/Application Support")
    } else if cfg!(windows) {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| resolve_path("~/AppData/Roaming"))
    } else {
        xdg_config_root()
    };
    let mut editors = [
        "Code",
        "Code - Insiders",
        "VSCodium",
        "Cursor",
        "Windsurf",
        "Devin",
        "Trae",
        "Trae CN",
        "Kiro",
    ];
    editors.sort_by_key(|editor| {
        !crate::services::local_detection::extension_installed_in_editor(editor, ids)
    });
    editors
        .iter()
        .flat_map(|editor| {
            ids.iter().map({
                let base = base.clone();
                move |id| {
                    base.join(editor)
                        .join("User/globalStorage")
                        .join(id)
                        .join("settings")
                        .join(filename)
                }
            })
        })
        .collect()
}

/// 获取指定 Agent 工具的配置文件路径列表
pub fn get_agent_config_paths(app: &AppType) -> Vec<PathBuf> {
    match app.canonical() {
        AppType::Cline => {
            let root = std::env::var_os("CLINE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| resolve_path("~/.cline"));
            let data = std::env::var_os("CLINE_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| root.join("data"));
            let mut paths = vec![data.join("settings/cline_mcp_settings.json")];
            paths.extend(extension_config_paths(
                &["saoudrizwan.claude-dev"],
                "cline_mcp_settings.json",
            ));
            return paths;
        }
        AppType::Roo => {
            return extension_config_paths(
                &["rooveterinaryinc.roo-cline", "roo-cline.roo-cline"],
                "mcp_settings.json",
            )
        }
        AppType::Kilo => {
            let root = xdg_config_root().join("kilo");
            let mut paths = [
                "kilo.json",
                "kilo.jsonc",
                "config.json",
                "opencode.json",
                "opencode.jsonc",
            ]
            .iter()
            .map(|name| root.join(name))
            .collect::<Vec<_>>();
            paths.extend(extension_config_paths(
                &["kilocode.kilo-code", "kilo-code.kilo-code"],
                "mcp_settings.json",
            ));
            return paths;
        }
        AppType::Goose => {
            if let Some(root) = std::env::var_os("GOOSE_PATH_ROOT")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
            {
                return vec![root.join("config/config.yaml")];
            }
            let mut paths = vec![xdg_config_root().join("goose/config.yaml")];
            if cfg!(target_os = "macos") {
                paths.insert(
                    0,
                    resolve_path("~/Library/Application Support/Block/goose/config.yaml"),
                );
            } else if cfg!(windows) {
                if let Some(base) = std::env::var_os("APPDATA") {
                    paths.insert(
                        0,
                        PathBuf::from(base).join("Block/goose/config/config.yaml"),
                    );
                }
            }
            return paths;
        }
        AppType::Codex => {
            let root = std::env::var_os("CODEX_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| resolve_path("~/.codex"));
            return vec![root.join("config.toml")];
        }
        AppType::KimiCode => {
            let root = std::env::var_os("KIMI_CODE_HOME")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| resolve_path("~/.kimi-code"));
            return vec![root.join("mcp.json")];
        }
        AppType::MinimaxCode => {
            let root = ["MINIMAX_DATA_DIR", "MAVIS_DATA_DIR"]
                .iter()
                .filter_map(std::env::var_os)
                .find(|value| !value.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| resolve_path("~/.minimax"));
            return vec![root.join("mcp.json")];
        }
        _ => {}
    }
    let paths: Vec<&str> = match app {
        AppType::QwenCode => vec!["~/.qwen/settings.json"],
        AppType::Claude => {
            if cfg!(windows) {
                vec!["%USERPROFILE%\\.claude.json"]
            } else {
                vec!["~/.claude.json"]
            }
        }
        AppType::Codex => {
            if cfg!(windows) {
                vec!["%USERPROFILE%\\.codex\\config.toml"]
            } else {
                vec!["~/.codex/config.toml"]
            }
        }
        AppType::Gemini => {
            if cfg!(windows) {
                vec!["%USERPROFILE%\\.gemini\\settings.json"]
            } else {
                vec!["~/.gemini/settings.json"]
            }
        }
        AppType::OpenCode => {
            if cfg!(windows) {
                vec!["%USERPROFILE%\\.config\\opencode\\opencode.json"]
            } else {
                vec!["~/.config/opencode/opencode.json"]
            }
        }
        AppType::Trae | AppType::TraeWork => {
            if cfg!(windows) {
                vec![
                    "%APPDATA%\\Trae\\User\\mcp.json",
                    "%APPDATA%\\TRAE Work\\User\\mcp.json",
                ]
            } else {
                vec![
                    "~/Library/Application Support/Trae/User/mcp.json",
                    "~/Library/Application Support/TRAE Work/User/mcp.json",
                ]
            }
        }
        AppType::TraeCn | AppType::TraeSoloCn => {
            if cfg!(windows) {
                vec![
                    "%APPDATA%\\Trae CN\\User\\mcp.json",
                    "%APPDATA%\\TRAE Work CN\\User\\mcp.json",
                    "%APPDATA%\\TRAE SOLO CN\\User\\mcp.json",
                ]
            } else {
                vec![
                    "~/Library/Application Support/Trae CN/User/mcp.json",
                    "~/Library/Application Support/TRAE Work CN/User/mcp.json",
                    "~/Library/Application Support/TRAE SOLO CN/User/mcp.json",
                ]
            }
        }
        AppType::Qoder => {
            if cfg!(windows) {
                vec!["%APPDATA%\\Qoder\\SharedClientCache\\mcp.json"]
            } else {
                vec!["~/Library/Application Support/Qoder/SharedClientCache/mcp.json"]
            }
        }
        AppType::Qodercli => {
            if cfg!(windows) {
                vec!["%USERPROFILE%\\.qodercli\\settings.json"]
            } else {
                vec!["~/.qodercli/settings.json"]
            }
        }
        AppType::CodeBuddy => {
            if cfg!(windows) {
                vec!["%USERPROFILE%\\.codebuddy\\mcp.json"]
            } else {
                vec!["~/.codebuddy/mcp.json"]
            }
        }
        AppType::Hermes => {
            if cfg!(windows) {
                vec!["%USERPROFILE%\\.hermes\\config.yaml"]
            } else {
                vec!["~/.hermes/config.yaml"]
            }
        }
        AppType::MimoCode => {
            if cfg!(windows) {
                vec![
                    "%USERPROFILE%\\.config\\mimocode\\mimocode.json",
                    "%USERPROFILE%\\.config\\mimocode\\config.json",
                    "%USERPROFILE%\\.config\\mimocode\\mimocode.jsonc",
                ]
            } else {
                vec![
                    "~/.config/mimocode/mimocode.json",
                    "~/.config/mimocode/config.json",
                    "~/.config/mimocode/mimocode.jsonc",
                ]
            }
        }
        AppType::WorkBuddy | AppType::WorkBuddyCn => {
            if cfg!(windows) {
                vec!["%USERPROFILE%\\.workbuddy\\.mcp.json"]
            } else {
                vec!["~/.workbuddy/.mcp.json"]
            }
        }
        AppType::Cursor => vec!["~/.cursor/mcp.json"],
        AppType::Windsurf => vec![
            "~/.codeium/windsurf/mcp_config.json",
            "~/.config/devin/mcp_config.json",
        ],
        AppType::Continue => vec!["~/.continue/config.yaml"],
        AppType::Kiro => vec!["~/.kiro/settings/mcp.json"],
        AppType::Cline
        | AppType::Roo
        | AppType::Kilo
        | AppType::Goose
        | AppType::KimiCode
        | AppType::MinimaxCode => unreachable!(),
        AppType::Zcode => {
            if cfg!(windows) {
                vec!["%USERPROFILE%\\.zcode\\cli\\config.json"]
            } else {
                vec!["~/.zcode/cli/config.json"]
            }
        }
    };
    paths.iter().map(|p| resolve_path(p)).collect()
}

/// TRAE keeps its migration target; other Agents reuse the existing local configuration.
pub fn get_canonical_mcp_config_path(app: &AppType) -> PathBuf {
    let paths = get_agent_config_paths(&app.canonical());
    if !app.is_trae() {
        if let Some(path) = paths.iter().find(|path| path.is_file()) {
            return path.clone();
        }
    }
    paths
        .into_iter()
        .next()
        .expect("every supported application has an MCP configuration path")
}

/// 获取 Agent 工具的显示名称
pub fn get_agent_name(app: &AppType) -> String {
    match app {
        AppType::QwenCode => "Qwen Code".to_string(),
        AppType::Claude => "Claude Code".to_string(),
        AppType::Codex => "Codex".to_string(),
        AppType::Gemini => "Gemini CLI".to_string(),
        AppType::OpenCode => "OpenCode".to_string(),
        AppType::Trae | AppType::TraeWork => "TRAE".to_string(),
        AppType::TraeCn | AppType::TraeSoloCn => "TRAE CN".to_string(),
        AppType::Qoder => "Qoder".to_string(),
        AppType::Qodercli => "Qoder CLI".to_string(),
        AppType::CodeBuddy => "CodeBuddy CN CLI".to_string(),
        AppType::Hermes => "Hermes Agent".to_string(),
        AppType::MimoCode => "Mimo Code".to_string(),
        AppType::KimiCode => "Kimi Code".to_string(),
        AppType::MinimaxCode => "MiniMax Code".to_string(),
        AppType::WorkBuddy => "WorkBuddy".to_string(),
        AppType::WorkBuddyCn => "WorkBuddy".to_string(),
        AppType::Zcode => "ZCode".to_string(),
        AppType::Cursor => "Cursor".to_string(),
        AppType::Windsurf => "Windsurf".to_string(),
        AppType::Cline => "Cline".to_string(),
        AppType::Roo => "Roo Code".to_string(),
        AppType::Kilo => "Kilo Code".to_string(),
        AppType::Continue => "Continue".to_string(),
        AppType::Goose => "Goose".to_string(),
        AppType::Kiro => "Kiro".to_string(),
    }
}

/// 检测系统中所有已安装的 Agent 工具
pub fn detect_all_agents() -> Vec<DetectedAgent> {
    AppType::all()
        .iter()
        .map(|app| {
            // 使用更可靠的 binary 检测方式
            let exists = is_agent_installed(app);

            // 获取配置文件路径
            let paths = get_agent_config_paths(app);
            let mut found_path: Option<&PathBuf> = None;

            for p in &paths {
                if p.exists() {
                    found_path = Some(p);
                    break;
                }
            }

            let mcp_count = if let Some(config_path) = found_path {
                crate::import::import_from_path(app.clone(), config_path)
                    .map(|result| result.servers.len())
                    .unwrap_or(0)
            } else {
                0
            };

            DetectedAgent {
                app_type: app.clone(),
                name: get_agent_name(app),
                config_path: found_path
                    .map(|p| p.to_string_lossy().to_string())
                    .or_else(|| paths.first().map(|p| p.to_string_lossy().to_string()))
                    .unwrap_or_default(),
                exists,
                mcp_count,
            }
        })
        .filter(|agent| agent.exists)
        .collect()
}

/// 获取上次检测的工具列表（从状态文件读取）
pub fn get_last_detected_agents() -> Vec<String> {
    let state_path = resolve_path("~/.ai-toolkit/detected.json");
    if let Ok(content) = fs::read_to_string(&state_path) {
        if let Ok(agents) = serde_json::from_str::<Vec<String>>(&content) {
            return normalize_detected_agent_ids(&agents);
        }
    }
    vec![]
}

/// 保存检测到的工具列表
pub fn save_detected_agents(agents: &[String]) {
    let state_dir = resolve_path("~/.ai-toolkit");
    let _ = fs::create_dir_all(&state_dir);
    let state_path = state_dir.join("detected.json");
    if let Ok(json) = serde_json::to_string_pretty(&normalize_detected_agent_ids(agents)) {
        let _ = fs::write(&state_path, json);
    }
}

fn normalize_detected_agent_ids(agents: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    agents
        .iter()
        .map(|id| {
            id.parse::<AppType>()
                .map(|app| app.canonical().name().to_string())
                .unwrap_or_else(|_| id.clone())
        })
        .filter(|id| seen.insert(id.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trae_mcp_candidates_keep_regions_separate_and_current_path_first() {
        let international = get_agent_config_paths(&AppType::Trae);
        let china = get_agent_config_paths(&AppType::TraeCn);
        assert_eq!(international.len(), 2);
        assert_eq!(china.len(), 3);
        assert!(international[0].to_string_lossy().contains("Trae"));
        assert!(international[1].to_string_lossy().contains("TRAE Work"));
        assert!(china[0].to_string_lossy().contains("Trae CN"));
        assert!(china[1].to_string_lossy().contains("TRAE Work CN"));
        assert!(china[2].to_string_lossy().contains("TRAE SOLO CN"));
        assert!(!international.iter().any(|path| china.contains(path)));
        assert_eq!(get_agent_config_paths(&AppType::TraeWork), international);
        assert_eq!(get_agent_config_paths(&AppType::TraeSoloCn), china);
        assert_eq!(
            get_canonical_mcp_config_path(&AppType::TraeWork),
            international[0]
        );
        assert_eq!(
            get_canonical_mcp_config_path(&AppType::TraeSoloCn),
            china[0]
        );
    }

    #[test]
    fn previous_detection_legacy_ids_do_not_trigger_duplicate_product_notifications() {
        let ids = ["trae-work", "trae", "trae-solo-cn", "trae-cn", "codex"].map(str::to_string);
        assert_eq!(
            normalize_detected_agent_ids(&ids),
            vec!["trae", "trae-cn", "codex"]
        );
    }
}
