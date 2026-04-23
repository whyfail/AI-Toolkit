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

/// 获取指定 Agent 工具的配置文件路径列表
pub fn get_agent_config_paths(app: &AppType) -> Vec<PathBuf> {
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
        AppType::Trae => {
            if cfg!(windows) {
                vec!["%APPDATA%\\Trae\\User\\mcp.json"]
            } else {
                vec!["~/Library/Application Support/Trae/User/mcp.json"]
            }
        }
        AppType::TraeCn => {
            if cfg!(windows) {
                vec!["%APPDATA%\\Trae CN\\User\\mcp.json"]
            } else {
                vec!["~/Library/Application Support/Trae CN/User/mcp.json"]
            }
        }
        AppType::TraeSoloCn => {
            if cfg!(windows) {
                vec!["%APPDATA%\\TRAE SOLO CN\\User\\mcp.json"]
            } else {
                vec!["~/Library/Application Support/TRAE SOLO CN/User/mcp.json"]
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
    };
    paths.iter().map(|p| resolve_path(p)).collect()
}

/// 获取 Agent 工具的显示名称
pub fn get_agent_name(app: &AppType) -> String {
    match app {
        AppType::QwenCode => "Qwen Code".to_string(),
        AppType::Claude => "Claude Code".to_string(),
        AppType::Codex => "Codex".to_string(),
        AppType::Gemini => "Gemini CLI".to_string(),
        AppType::OpenCode => "OpenCode".to_string(),
        AppType::Trae => "Trae".to_string(),
        AppType::TraeCn => "Trae CN".to_string(),
        AppType::TraeSoloCn => "TRAE SOLO CN".to_string(),
        AppType::Qoder => "Qoder".to_string(),
        AppType::Qodercli => "Qoder CLI".to_string(),
        AppType::CodeBuddy => "CodeBuddy".to_string(),
    }
}

/// 获取 Agent 工具的安装检测目录（用于判断工具是否已安装）
pub fn get_agent_detect_dir(app: &AppType) -> Option<PathBuf> {
    let path_str = match app {
        AppType::QwenCode => "~/.qwen",
        AppType::Claude => {
            if cfg!(windows) {
                "%USERPROFILE%\\.claude"
            } else {
                "~/.claude"
            }
        }
        AppType::Codex => {
            if cfg!(windows) {
                "%USERPROFILE%\\.codex"
            } else {
                "~/.codex"
            }
        }
        AppType::Gemini => {
            if cfg!(windows) {
                "%USERPROFILE%\\.gemini"
            } else {
                "~/.gemini"
            }
        }
        AppType::OpenCode => {
            if cfg!(windows) {
                "%USERPROFILE%\\.config\\opencode"
            } else {
                "~/.config/opencode"
            }
        }
        AppType::Trae => {
            if cfg!(windows) {
                "%APPDATA%\\Trae"
            } else {
                "~/Library/Application Support/Trae"
            }
        }
        AppType::TraeCn => {
            if cfg!(windows) {
                "%APPDATA%\\Trae CN"
            } else {
                "~/Library/Application Support/Trae CN"
            }
        }
        AppType::TraeSoloCn => {
            if cfg!(windows) {
                "%APPDATA%\\TRAE SOLO CN"
            } else {
                "~/Library/Application Support/TRAE SOLO CN"
            }
        }
        AppType::Qoder => {
            if cfg!(windows) {
                "%APPDATA%\\Qoder"
            } else {
                "~/Library/Application Support/Qoder"
            }
        }
        AppType::Qodercli => {
            if cfg!(windows) {
                "%USERPROFILE%\\.qodercli"
            } else {
                "~/.qodercli"
            }
        }
        AppType::CodeBuddy => {
            if cfg!(windows) {
                "%USERPROFILE%\\.codebuddy"
            } else {
                "~/.codebuddy"
            }
        }
    };
    Some(resolve_path(path_str))
}

/// 获取 Agent 工具的 CLI binary 名称
fn get_agent_binary_name(app: &AppType) -> &'static str {
    match app {
        AppType::Trae => "trae",
        AppType::TraeCn => "trae",
        AppType::TraeSoloCn => "trae",
        AppType::QwenCode => "qwen",
        AppType::Claude => "claude",
        AppType::Codex => "codex",
        AppType::Gemini => "gemini",
        AppType::OpenCode => "opencode",
        AppType::Qoder => "qoder",
        AppType::Qodercli => "qodercli",
        AppType::CodeBuddy => "codebuddy",
    }
}

/// 使用 which_binary 检测 Agent 是否已安装（更可靠）
fn is_agent_installed(app: &AppType) -> bool {
    use crate::services::tool_manager::which_binary;
    let binary_name = get_agent_binary_name(app);

    // 先尝试 binary 检测
    if which_binary(binary_name).is_some() {
        return true;
    }

    // 检查配置路径是否存在
    let config_paths = get_agent_config_paths(app);
    if config_paths.iter().any(|p| p.exists()) {
        return true;
    }

    // Mac GUI 应用检测（通过检查 /Applications/*.app）
    #[cfg(target_os = "macos")]
    {
        let app_name = match app {
            AppType::Trae => "Trae.app",
            AppType::TraeCn => "Trae CN.app",
            AppType::TraeSoloCn => "TRAE SOLO CN.app",
            AppType::Qoder => "Qoder.app",
            _ => return false,
        };
        if std::path::Path::new(&format!("/Applications/{}", app_name)).exists() {
            return true;
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
    }

    false
}

/// 统计配置文件中的 MCP 服务器数量
fn count_mcp_in_config(path: &Path) -> usize {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return 0,
    };

    if path.extension().and_then(|s| s.to_str()) == Some("toml") {
        // Codex TOML 格式
        if let Ok(toml) = toml::from_str::<toml::Value>(&content) {
            if let Some(servers) = toml.get("mcp_servers") {
                return servers.as_table().map(|t| t.len()).unwrap_or(0);
            }
        }
    } else {
        // JSON 格式
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            // 多种可能的键名
            for key in ["mcpServers", "mcp"] {
                if let Some(obj) = json.get(key) {
                    if let Some(servers) = obj.as_object() {
                        return servers.len();
                    }
                    if let Some(arr) = obj.as_array() {
                        return arr.len();
                    }
                }
            }
            if let Some(models) = json.get("models") {
                if let Some(providers) = models.get("providers") {
                    if let Some(obj) = providers.as_object() {
                        return obj.len();
                    }
                }
            }
        }
    }

    0
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
                count_mcp_in_config(config_path)
            } else {
                0
            };

            DetectedAgent {
                app_type: app.clone(),
                name: get_agent_name(app),
                config_path: found_path
                    .map(|p| p.to_string_lossy().to_string())
                    .or_else(|| get_agent_detect_dir(app).map(|p| p.to_string_lossy().to_string()))
                    .unwrap_or_default(),
                exists,
                mcp_count,
            }
        })
        .collect()
}

/// 获取上次检测的工具列表（从状态文件读取）
pub fn get_last_detected_agents() -> Vec<String> {
    let state_path = resolve_path("~/.ai-toolkit/detected.json");
    if let Ok(content) = fs::read_to_string(&state_path) {
        if let Ok(agents) = serde_json::from_str::<Vec<String>>(&content) {
            return agents;
        }
    }
    vec![]
}

/// 保存检测到的工具列表
pub fn save_detected_agents(agents: &[String]) {
    let state_dir = resolve_path("~/.ai-toolkit");
    let _ = fs::create_dir_all(&state_dir);
    let state_path = state_dir.join("detected.json");
    if let Ok(json) = serde_json::to_string_pretty(agents) {
        let _ = fs::write(&state_path, json);
    }
}
