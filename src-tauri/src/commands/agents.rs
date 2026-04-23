use serde::{Deserialize, Serialize};
use tauri::State;
use std::str::FromStr;

use crate::agents::{detect_all_agents, DetectedAgent, get_agent_config_paths, get_agent_name};
use crate::app_state::AppState;
use crate::database::McpApps;
use crate::import::import_from_path;
use crate::mcp::AppType;
use crate::services::sync;
use crate::utils::SuppressConsole;
use std::process::Command;

/// 检测 Node.js 环境并返回需要添加到 PATH 的路径
/// 返回 Ok(bin_dir_path) 或 Err(error_message)
#[cfg(not(windows))]
fn detect_node_environment() -> Result<String, String> {
    let home = std::env::var("HOME").unwrap_or_default();

    // 先检测 node 是否已经可用 (直接用 which)
    if let Ok(output) = Command::new("which").suppress_console().arg("node").output() {
        if output.status.success() {
            let node_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !node_path.is_empty() {
                if let Some(parent) = std::path::Path::new(&node_path).parent() {
                    return Ok(parent.to_string_lossy().to_string());
                }
            }
        }
    }

    // 检查 nvm
    let nvm_prefix = format!("{}/.nvm/versions/node", home);
    if std::path::Path::new(&nvm_prefix).exists() {
        if let Ok(entries) = std::fs::read_dir(&nvm_prefix) {
            if let Some(newest) = entries.filter_map(|e| e.ok())
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.starts_with('v') {
                        e.path().join("bin/node").exists().then_some(name)
                    } else {
                        None
                    }
                })
                .max() {
                return Ok(format!("{}/.nvm/versions/node/{}/bin", home, newest));
            }
        }
    }

    // 检查 fnm
    let fnm_dir = format!("{}/.fnm", home);
    if std::path::Path::new(&fnm_dir).exists() {
        if let Ok(output) = Command::new("fnm").suppress_console().arg("current").output() {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !version.is_empty() {
                    let fnm_path = format!("{}/.fnm/versions/{}/installation/bin", home, version);
                    if std::path::Path::new(&fnm_path).exists() {
                        return Ok(fnm_path);
                    }
                }
            }
        }
        let fnm_default = format!("{}/.fnm/versions/node-default/bin", home);
        if std::path::Path::new(&fnm_default).exists() {
            return Ok(fnm_default);
        }
    }

    // 检查 volta
    let volta_path = format!("{}/.volta/bin", home);
    if std::path::Path::new(&volta_path).exists() {
        return Ok(volta_path);
    }

    // 检查 nvmd
    let nvmd_path = format!("{}/.nvmd/bin", home);
    if std::path::Path::new(&nvmd_path).exists() {
        return Ok(nvmd_path);
    }

    // 检查 homebrew node
    if std::path::Path::new("/opt/homebrew/bin/node").exists() {
        return Ok("/opt/homebrew/bin".to_string());
    }
    if std::path::Path::new("/usr/local/bin/node").exists() {
        return Ok("/usr/local/bin".to_string());
    }

    Err("未检测到 Node.js 安装，请先安装: https://nodejs.org".to_string())
}

/// 检测 Node.js 环境并返回需要添加到 PATH 的路径 (Windows 版本)
#[cfg(windows)]
fn detect_node_environment() -> Result<String, String> {
    // 1. 先尝试 where node 找到 node.exe
    if let Ok(output) = Command::new("where").suppress_console().arg("node").output() {
        if output.status.success() {
            let node_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            // where 可能返回多行，取第一行
            if let Some(first_line) = node_path.lines().next() {
                let node_path = first_line.trim();
                if !node_path.is_empty() {
                    if let Some(parent) = std::path::Path::new(&node_path).parent() {
                        return Ok(parent.to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    let home = std::env::var("USERPROFILE").unwrap_or_default();

    // 2. 检查 fnm (Windows 常用)
    if let Ok(output) = Command::new("fnm").suppress_console().arg("current").output() {
        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !version.is_empty() {
                let fnm_path = format!("{}\\AppData\\Roaming\\fnm\\versions\\{}\\installation", home, version);
                if std::path::Path::new(&fnm_path).exists() {
                    return Ok(fnm_path);
                }
                let fnm_path2 = format!("{}\\.fnm\\versions\\{}\\installation", home, version);
                if std::path::Path::new(&fnm_path2).exists() {
                    return Ok(fnm_path2);
                }
            }
        }
    }

    // 3. 检查 nvm-windows
    let nvm_home = std::env::var("NVM_HOME").unwrap_or_default();
    if !nvm_home.is_empty() {
        let nvm_symlink = format!("{}\\v{}", nvm_home, std::env::var("NVM_SYMLINK").unwrap_or_default());
        if std::path::Path::new(&nvm_symlink).exists() {
            return Ok(nvm_symlink);
        }
    }

    // 4. 检查 volta
    let volta_path = format!("{}\\AppData\\Local\\Volta\\bin", home);
    if std::path::Path::new(&volta_path).exists() {
        return Ok(volta_path);
    }

    // 5. 检查 nvmd
    let nvmd_path = format!("{}\\.nvmd\\bin", home);
    if std::path::Path::new(&nvmd_path).exists() {
        return Ok(nvmd_path);
    }

    // 6. 检查默认 npm 全局目录
    if let Ok(output) = Command::new("cmd").suppress_console().args(["/C", "npm config get prefix"]).output() {
        if output.status.success() {
            let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !prefix.is_empty() && std::path::Path::new(&prefix).exists() {
                return Ok(prefix);
            }
        }
    }

    Err("未检测到 Node.js 安装，请先安装: https://nodejs.org".to_string())
}

/// 检测到的 Agent 信息（前端用）
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub config_path: String,
    pub exists: bool,
    pub mcp_count: usize,
}

impl From<DetectedAgent> for AgentInfo {
    fn from(agent: DetectedAgent) -> Self {
        Self {
            id: agent.app_type.name().to_string(),
            name: agent.name,
            config_path: agent.config_path,
            exists: agent.exists,
            mcp_count: agent.mcp_count,
        }
    }
}

/// 检测所有已安装的 Agent 工具
#[tauri::command]
pub async fn detect_agents() -> Vec<AgentInfo> {
    detect_all_agents()
        .into_iter()
        .map(AgentInfo::from)
        .collect()
}

/// 同步指定 Agent 的 MCP 配置
#[tauri::command]
pub async fn sync_agent_mcp(
    state: State<'_, AppState>,
    agent_id: String,
    enabled_apps: Vec<String>,
) -> Result<usize, String> {
    let app_type = AppType::from_str(&agent_id).map_err(|e| e.to_string())?;

    // Get OS-specific paths and try to import from the first existing one
    let paths = get_agent_config_paths(&app_type);
    let mut imported = None;
    
    for path in &paths {
        if let Some(result) = import_from_path(app_type.clone(), path) {
            imported = Some(result);
            break;
        }
    }
    
    let imported = imported.ok_or_else(|| format!("Failed to import from {}", agent_id))?;

    let mut count = 0;
    let enabled_apps_set: Vec<AppType> = enabled_apps
        .iter()
        .filter_map(|id| AppType::from_str(id).ok())
        .collect();

    for (_id, mut server) in imported.servers {
        // 设置启用的应用
        let mut apps = McpApps::default();
        for app in &enabled_apps_set {
            apps.set_enabled_for(app, true);
        }
        server.apps = apps;

        // 保存到数据库（如果已存在则更新）
        let _ = state.db.save_mcp_server(&server);
        count += 1;
    }

    // 同步到各工具的配置文件
    let servers = state.db.get_all_mcp_servers().map_err(|e| e.to_string())?;
    sync::sync_all_live_configs(&servers).map_err(|e| e.to_string())?;

    Ok(count)
}

/// 打开配置文件（使用系统默认编辑器）
#[tauri::command]
pub async fn open_config_file(agent_id: String) -> Result<(), String> {
    let app_type = AppType::from_str(&agent_id).map_err(|e| e.to_string())?;
    let paths = get_agent_config_paths(&app_type);

    let full_path = paths.first().ok_or_else(|| format!("No config path found for {}", agent_id))?;

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .suppress_console()
            .arg(&full_path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .suppress_console()
            .args(["/c", "start", &full_path.to_string_lossy()])
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .suppress_console()
            .arg(&full_path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }

    Ok(())
}

/// 启动 Agent 工具（打开默认终端并运行命令）

fn get_agent_launch_command(app: &AppType) -> Option<String> {
    match app {
        AppType::QwenCode => Some("qwen".to_string()),
        AppType::Claude => Some("claude".to_string()),
        AppType::Codex => Some("codex".to_string()),
        AppType::Gemini => Some("gemini".to_string()),
        AppType::OpenCode => Some("opencode".to_string()),
        AppType::Trae => None,
        AppType::TraeCn => None,
        AppType::TraeSoloCn => None,
        AppType::Qoder => None,
        AppType::Qodercli => Some("qodercli".to_string()),
        AppType::CodeBuddy => Some("codebuddy".to_string()),
    }
}

/// 启动 Agent 工具（打开默认终端并运行命令）
#[tauri::command]
pub async fn launch_agent(agent_id: String) -> Result<(), String> {
    let app_type = AppType::from_str(&agent_id).map_err(|e| e.to_string())?;

    let Some(command) = get_agent_launch_command(&app_type) else {
        return Err(format!("{} 没有 CLI 命令，无法启动", get_agent_name(&app_type)));
    };

    // 检测 Node.js 环境
    let node_bin_dir = detect_node_environment().map_err(|e| {
        format!("{}: 请先安装 Node.js", e)
    })?;

    #[cfg(target_os = "macos")]
    {
        let script_path = format!("/tmp/ai_toolkit_run_{}.sh", std::process::id());
        let full_cmd = format!(
            "cd ~/Desktop && export PATH=\"{}:$PATH:/usr/local/bin:/opt/homebrew/bin\" && {}; exec $SHELL",
            node_bin_dir, command
        );
        std::fs::write(&script_path, &full_cmd)
            .map_err(|e| format!("写入脚本失败: {}", e))?;

        let script = format!(
            "tell application \"Terminal\"\n\
             do script \"source {0}\"\n\
             end tell",
            script_path
        );
        let output = Command::new("osascript")
            .suppress_console()
            .args(["-e", &script])
            .output()
            .map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("启动 {} 失败: {}", agent_id, stderr));
        }
    }

    #[cfg(target_os = "windows")]
    {
        let desktop_path = dirs::desktop_dir()
            .or_else(|| dirs::home_dir())
            .ok_or("无法获取桌面路径")?
            .to_string_lossy()
            .to_string();

        let full_cmd = format!(
            "cd '{}'; $env:PATH = '{};' + $env:PATH; {}",
            desktop_path,
            node_bin_dir,
            command
        );
        Command::new("wt")
            .args(["new-tab", "powershell", "-NoExit", "-c", &full_cmd])
            .spawn()
            .map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        return Err("启动功能仅支持 macOS 和 Windows".to_string());
    }

    Ok(())
}
