use serde::{Deserialize, Serialize};
use tauri::State;

use crate::agents::{get_agent_config_paths, get_agent_name};
use crate::app_state::AppState;
use crate::mcp::AppType;
#[cfg(target_os = "windows")]
use crate::services::tool_manager::which_binary;
use crate::services::McpService;

const DEFAULT_TERMINAL_SETTING_KEY: &str = "default_terminal";
#[cfg(target_os = "macos")]
const DEFAULT_TERMINAL_ID: &str = "terminal";
#[cfg(target_os = "windows")]
const DEFAULT_TERMINAL_ID: &str = "windows-terminal";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DEFAULT_TERMINAL_ID: &str = "system";

fn normalize_terminal_id(id: &str) -> Option<String> {
    match id {
        "terminal" | "iterm" | "warp" | "ghostty" | "windows-terminal" | "powershell"
        | "command-prompt" | "system" => Some(id.to_string()),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn terminal_candidates() -> Vec<TerminalOption> {
    vec![
        TerminalOption {
            id: "terminal".to_string(),
            label: "Terminal".to_string(),
            available: [
                "/System/Applications/Utilities/Terminal.app",
                "/Applications/Terminal.app",
            ]
            .iter()
            .any(|path| std::path::Path::new(path).exists()),
        },
        TerminalOption {
            id: "iterm".to_string(),
            label: "iTerm".to_string(),
            available: ["/Applications/iTerm.app", "/Applications/iTerm2.app"]
                .iter()
                .any(|path| std::path::Path::new(path).exists()),
        },
        TerminalOption {
            id: "warp".to_string(),
            label: "Warp".to_string(),
            available: ["/Applications/Warp.app", "/Applications/Warp Preview.app"]
                .iter()
                .any(|path| std::path::Path::new(path).exists()),
        },
        TerminalOption {
            id: "ghostty".to_string(),
            label: "Ghostty".to_string(),
            available: ["/Applications/Ghostty.app"]
                .iter()
                .any(|path| std::path::Path::new(path).exists()),
        },
    ]
}

#[cfg(target_os = "windows")]
fn terminal_candidates() -> Vec<TerminalOption> {
    vec![
        TerminalOption {
            id: "windows-terminal".to_string(),
            label: "Windows Terminal".to_string(),
            available: which_binary("wt").is_some(),
        },
        TerminalOption {
            id: "powershell".to_string(),
            label: "PowerShell".to_string(),
            available: which_binary("powershell").is_some(),
        },
        TerminalOption {
            id: "command-prompt".to_string(),
            label: "Command Prompt".to_string(),
            available: which_binary("cmd").is_some(),
        },
    ]
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn terminal_candidates() -> Vec<TerminalOption> {
    Vec::new()
}

/// 获取当前应用版本
#[tauri::command]
pub async fn get_version(app: tauri::AppHandle) -> Result<VersionInfo, String> {
    Ok(VersionInfo {
        version: app.package_info().version.to_string(),
    })
}

#[derive(Serialize)]
pub struct VersionInfo {
    pub version: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalOption {
    pub id: String,
    pub label: String,
    pub available: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchPreferences {
    pub default_terminal: String,
    pub available_terminals: Vec<TerminalOption>,
}

#[tauri::command]
pub async fn get_launch_preferences(
    state: State<'_, AppState>,
) -> Result<LaunchPreferences, String> {
    let available_terminals = terminal_candidates();
    let stored = state
        .db
        .get_setting(DEFAULT_TERMINAL_SETTING_KEY)
        .map_err(|e| e.to_string())?;

    Ok(LaunchPreferences {
        default_terminal: stored
            .as_deref()
            .and_then(normalize_terminal_id)
            .unwrap_or_else(|| DEFAULT_TERMINAL_ID.to_string()),
        available_terminals,
    })
}

#[tauri::command]
pub async fn set_default_terminal(
    state: State<'_, AppState>,
    terminal_id: String,
) -> Result<(), String> {
    let normalized = normalize_terminal_id(&terminal_id)
        .ok_or_else(|| format!("Unsupported terminal: {}", terminal_id))?;
    state
        .db
        .set_setting(DEFAULT_TERMINAL_SETTING_KEY, &normalized)
        .map_err(|e| e.to_string())
}

/// 获取应用配置路径
#[tauri::command]
pub async fn get_app_configs(_state: State<'_, AppState>) -> Result<Vec<AppConfigInfo>, String> {
    let configs = AppType::all()
        .into_iter()
        .map(|app| AppConfigInfo {
            id: app.name().to_string(),
            name: get_agent_name(&app),
            config_path: get_agent_config_paths(&app)
                .first()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default(),
            mcp_count: 0,
        })
        .collect();
    Ok(configs)
}

/// 从指定应用导入 MCP
#[tauri::command]
pub async fn import_mcp_from_app(
    state: State<'_, AppState>,
    app_id: String,
) -> Result<usize, String> {
    let app = app_id.parse::<AppType>().map_err(|e| e.to_string())?;
    McpService::import_from_app(&state, app).map_err(|e| e.to_string())
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfigInfo {
    pub id: String,
    pub name: String,
    pub config_path: String,
    pub mcp_count: usize,
}
