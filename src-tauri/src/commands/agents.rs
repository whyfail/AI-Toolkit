use serde::{Deserialize, Serialize};
use tauri::State;
use std::str::FromStr;

use crate::agents::{detect_all_agents, DetectedAgent, get_agent_config_paths, get_agent_name};
use crate::app_state::AppState;
use crate::database::McpApps;
use crate::import::import_from_path;
use crate::mcp::AppType;
use crate::services::sync;
use std::process::Command;

/// 检测 Node.js 环境并返回需要添加到 PATH 的路径
/// 返回 Ok(bin_dir_path) 或 Err(error_message)
fn detect_node_environment() -> Result<String, String> {
    let home = std::env::var("HOME").unwrap_or_default();

    // 先检测 node 是否已经可用 (直接用 which)
    if let Ok(output) = Command::new("which").arg("node").output() {
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
        if let Ok(output) = Command::new("fnm").arg("current").output() {
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
            .arg(&full_path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/c", "start", &full_path.to_string_lossy()])
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&full_path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }

    Ok(())
}

/// 终端类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalInfo {
    pub id: String,
    pub name: String,
    pub path: String,
}

/// 检测系统已安装的终端
#[tauri::command]
pub fn get_terminals() -> Vec<TerminalInfo> {
    let mut terminals = Vec::new();

    #[cfg(target_os = "macos")]
    {
        // Terminal.app
        terminals.push(TerminalInfo {
            id: "terminal".to_string(),
            name: "Terminal".to_string(),
            path: "/System/Applications/Utilities/Terminal.app".to_string(),
        });

        // iTerm2
        if std::path::Path::new("/Applications/iTerm.app").exists() {
            terminals.push(TerminalInfo {
                id: "iterm".to_string(),
                name: "iTerm".to_string(),
                path: "/Applications/iTerm.app".to_string(),
            });
        }

        // Warp
        if std::path::Path::new("/Applications/Warp.app").exists() {
            terminals.push(TerminalInfo {
                id: "warp".to_string(),
                name: "Warp".to_string(),
                path: "/Applications/Warp.app".to_string(),
            });
        }

        // Hyper
        if std::path::Path::new("/Applications/Hyper.app").exists() {
            terminals.push(TerminalInfo {
                id: "hyper".to_string(),
                name: "Hyper".to_string(),
                path: "/Applications/Hyper.app".to_string(),
            });
        }

        // Kitty
        if std::path::Path::new("/Applications/kitty.app").exists() {
            terminals.push(TerminalInfo {
                id: "kitty".to_string(),
                name: "Kitty".to_string(),
                path: "/Applications/kitty.app".to_string(),
            });
        }

        // Alacritty
        if std::path::Path::new("/Applications/Alacritty.app").exists() {
            terminals.push(TerminalInfo {
                id: "alacritty".to_string(),
                name: "Alacritty".to_string(),
                path: "/Applications/Alacritty.app".to_string(),
            });
        }

        // Fig
        if std::path::Path::new("/Applications/Fig.app").exists() {
            terminals.push(TerminalInfo {
                id: "fig".to_string(),
                name: "Fig".to_string(),
                path: "/Applications/Fig.app".to_string(),
            });
        }

        // Kaku
        if std::path::Path::new("/Applications/Kaku.app").exists() {
            terminals.push(TerminalInfo {
                id: "kaku".to_string(),
                name: "Kaku".to_string(),
                path: "/Applications/Kaku.app".to_string(),
            });
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows Terminal
        if Command::new("where")
            .arg("wt")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            terminals.push(TerminalInfo {
                id: "windows-terminal".to_string(),
                name: "Windows Terminal".to_string(),
                path: "wt.exe".to_string(),
            });
        }

        // PowerShell 7+
        if Command::new("where")
            .arg("pwsh")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            terminals.push(TerminalInfo {
                id: "pwsh".to_string(),
                name: "PowerShell 7".to_string(),
                path: "pwsh.exe".to_string(),
            });
        }

        // Windows PowerShell (5.1)
        if Command::new("where")
            .arg("powershell")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            terminals.push(TerminalInfo {
                id: "powershell".to_string(),
                name: "Windows PowerShell".to_string(),
                path: "powershell.exe".to_string(),
            });
        }

        // CMD
        if Command::new("where")
            .arg("cmd")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            terminals.push(TerminalInfo {
                id: "cmd".to_string(),
                name: "CMD".to_string(),
                path: "cmd.exe".to_string(),
            });
        }

        // Git Bash
        let git_bash_path = r"C:\Program Files\Git\bin\bash.exe";
        if std::path::Path::new(git_bash_path).exists() {
            terminals.push(TerminalInfo {
                id: "git-bash".to_string(),
                name: "Git Bash".to_string(),
                path: git_bash_path.to_string(),
            });
        }
    }

    terminals
}

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

/// 启动 Agent 工具（打开终端并运行命令）
#[tauri::command]
pub async fn launch_agent(agent_id: String, terminal_id: Option<String>) -> Result<(), String> {
    let app_type = AppType::from_str(&agent_id).map_err(|e| e.to_string())?;

    let Some(command) = get_agent_launch_command(&app_type) else {
        return Err(format!("{} 没有 CLI 命令，无法启动", get_agent_name(&app_type)));
    };

    // 检测 Node.js 环境
    let node_bin_dir = detect_node_environment().map_err(|e| {
        format!("{}: 请先安装 Node.js", e)
    })?;

    let term_id = terminal_id.unwrap_or_else(|| "terminal".to_string());

    #[cfg(target_os = "macos")]
    {
        match term_id.as_str() {
            "terminal" => {
                let full_cmd = format!(
                    "cd ~/Desktop && export PATH=\"{0}:$PATH:/usr/local/bin:/opt/homebrew/bin\" && {1}",
                    node_bin_dir.replace("\"", "\\\""),
                    command
                );
                // 写入临时脚本文件
                let script_path = format!("/tmp/ai_toolkit_run_{}.sh", std::process::id());
                std::fs::write(&script_path, &full_cmd).map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
                let output = Command::new("osascript")
                    .args(["-e", &format!("tell application \"Terminal\" to do script \"chmod +x {0} && {0}\"", script_path)])
                    .output()
                    .map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    return Err(format!("启动 {} 失败: {} {}", agent_id, stdout, stderr));
                }
            }
            "iterm" => {
                let full_cmd = format!(
                    "cd ~/Desktop && export PATH=\"{0}:$PATH:/usr/local/bin:/opt/homebrew/bin\" && {1}",
                    node_bin_dir,
                    command
                );
                let script = format!(
                    "tell application \"iTerm\"\n\
                     activate\n\
                     create window with default profile\n\
                     tell current session of current window\n\
                     write text \"{}\"\n\
                     end tell\n\
                     end tell",
                    full_cmd
                );
                let output = Command::new("osascript")
                    .args(["-e", &script])
                    .output()
                    .map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    return Err(format!("启动 {} 失败: {} {}", agent_id, stdout, stderr));
                }
            }
            "warp" => {
                let full_cmd = format!(
                    "cd ~/Desktop && export PATH=\"{0}:$PATH:/usr/local/bin:/opt/homebrew/bin\" && {1}",
                    node_bin_dir,
                    command
                );
                let script = format!(
                    "tell application \"Warp\" to activate\n\
                     delay 0.5\n\
                     tell application \"System Events\"\n\
                     keystroke \"{}\" & return\n\
                     end tell",
                    full_cmd
                );
                let output = Command::new("osascript")
                    .args(["-e", &script])
                    .output()
                    .map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    return Err(format!("启动 {} 失败: {} {}", agent_id, stdout, stderr));
                }
            }
            "hyper" => {
                let full_cmd = format!(
                    "cd ~/Desktop && export PATH=\"{0}:$PATH:/usr/local/bin:/opt/homebrew/bin\" && {1}",
                    node_bin_dir.replace("\"", "\\\""),
                    command
                );
                let script_path = format!("/tmp/ai_toolkit_run_{}.sh", std::process::id());
                std::fs::write(&script_path, &full_cmd).map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
                let output = Command::new("osascript")
                    .args(["-e", &format!(
                        "tell application \"Hyper\"\n\
                         activate\n\
                         delay 0.5\n\
                         tell application \"System Events\"\n\
                         keystroke \"bash -c \\\"source {}\\\"\" & return\n\
                         end tell",
                        script_path
                    )])
                    .output()
                    .map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    return Err(format!("启动 {} 失败: {} {}", agent_id, stdout, stderr));
                }
            }
            "kitty" => {
                let full_cmd = format!(
                    "cd ~/Desktop && export PATH=\"{0}:$PATH:/usr/local/bin:/opt/homebrew/bin\" && {1}",
                    node_bin_dir.replace("\"", "\\\""),
                    command
                );
                let script_path = format!("/tmp/ai_toolkit_run_{}.sh", std::process::id());
                std::fs::write(&script_path, &full_cmd).map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
                let output = Command::new("osascript")
                    .args(["-e", &format!(
                        "tell application \"Kitty\"\n\
                         activate\n\
                         delay 0.5\n\
                         tell application \"System Events\"\n\
                         keystroke \"bash -c \\\"source {}\\\"\" & return\n\
                         end tell",
                        script_path
                    )])
                    .output()
                    .map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    return Err(format!("启动 {} 失败: {} {}", agent_id, stdout, stderr));
                }
            }
            "alacritty" => {
                let full_cmd = format!(
                    "cd ~/Desktop && export PATH=\"{0}:$PATH:/usr/local/bin:/opt/homebrew/bin\" && {1}",
                    node_bin_dir.replace("\"", "\\\""),
                    command
                );
                let script_path = format!("/tmp/ai_toolkit_run_{}.sh", std::process::id());
                std::fs::write(&script_path, &full_cmd).map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
                let output = Command::new("osascript")
                    .args(["-e", &format!(
                        "tell application \"Alacritty\"\n\
                         activate\n\
                         delay 0.5\n\
                         tell application \"System Events\"\n\
                         keystroke \"bash -c \\\"source {}\\\"\" & return\n\
                         end tell",
                        script_path
                    )])
                    .output()
                    .map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    return Err(format!("启动 {} 失败: {} {}", agent_id, stdout, stderr));
                }
            }
            "fig" => {
                let full_cmd = format!(
                    "cd ~/Desktop && export PATH=\"{0}:$PATH:/usr/local/bin:/opt/homebrew/bin\" && {1}",
                    node_bin_dir.replace("\"", "\\\""),
                    command
                );
                let script_path = format!("/tmp/ai_toolkit_run_{}.sh", std::process::id());
                std::fs::write(&script_path, &full_cmd).map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
                let output = Command::new("osascript")
                    .args(["-e", &format!(
                        "tell application \"Fig\"\n\
                         activate\n\
                         delay 0.5\n\
                         tell application \"System Events\"\n\
                         keystroke \"bash -c \\\"source {}\\\"\" & return\n\
                         end tell",
                        script_path
                    )])
                    .output()
                    .map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    return Err(format!("启动 {} 失败: {} {}", agent_id, stdout, stderr));
                }
            }
            "kaku" => {
                // Kaku 是基于 WezTerm 的终端
                let full_cmd = format!(
                    "cd ~/Desktop && export PATH=\"{0}:$PATH:/usr/local/bin:/opt/homebrew/bin\" && {1}",
                    node_bin_dir.replace("\"", "\\\""),
                    command
                );
                let script_path = format!("/tmp/ai_toolkit_run_{}.sh", std::process::id());
                std::fs::write(&script_path, &full_cmd).map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
                let output = Command::new("osascript")
                    .args(["-e", &format!(
                        "tell application \"Kaku\"\n\
                         activate\n\
                         delay 0.5\n\
                         tell application \"System Events\"\n\
                         keystroke \"bash -c \\\"source {}\\\"\" & return\n\
                         end tell",
                        script_path
                    )])
                    .output()
                    .map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    return Err(format!("启动 {} 失败: {} {}", agent_id, stdout, stderr));
                }
            }
            _ => {
                return Err(format!("不支持的终端: {}", term_id));
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        use std::env;

        let desktop_path = env::var("USERPROFILE")
            .map(|p| format!("{}\\Desktop", p))
            .unwrap_or_else(|_| "C:\\Users\\Public\\Desktop".to_string());

        match term_id.as_str() {
            "windows-terminal" => {
                let full_cmd = format!(
                    "cd /d \"{}\" && set PATH=\"{};%PATH%\" && {}",
                    desktop_path,
                    node_bin_dir.replace("\\", "\\\\"),
                    command
                );
                Command::new("wt")
                    .args(["new-tab", "powershell", "-c", &full_cmd])
                    .spawn()
                    .map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
            }
            "pwsh" => {
                let full_cmd = format!(
                    "cd \"{}\"; $env:PATH=\"{};$env:PATH\"; {}",
                    desktop_path,
                    node_bin_dir.replace("\\", "\\\\"),
                    command
                );
                Command::new("pwsh")
                    .args(["-NoExit", "-c", &full_cmd])
                    .spawn()
                    .map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
            }
            "powershell" => {
                let full_cmd = format!(
                    "cd \"{}\"; $env:PATH=\"{};$env:PATH\"; {}",
                    desktop_path,
                    node_bin_dir.replace("\\", "\\\\"),
                    command
                );
                Command::new("powershell")
                    .args(["-NoExit", "-c", &full_cmd])
                    .spawn()
                    .map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
            }
            "cmd" => {
                let full_cmd = format!(
                    "cd /d \"{}\" && set PATH=\"{};%PATH%\" && {}",
                    desktop_path,
                    node_bin_dir.replace("\\", "\\\\"),
                    command
                );
                Command::new("cmd")
                    .args(["/c", "start", "cmd", "/k", &full_cmd])
                    .spawn()
                    .map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
            }
            "git-bash" => {
                let full_cmd = format!(
                    "cd \"{}\" && export PATH=\"{}:$PATH\" && {}",
                    desktop_path.replace("\\", "/"),
                    node_bin_dir.replace("\\", "/"),
                    command
                );
                let git_bash_path = r"C:\Program Files\Git\bin\bash.exe";
                Command::new(git_bash_path)
                    .args(["-c", &full_cmd])
                    .spawn()
                    .map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
            }
            _ => {
                return Err(format!("不支持的终端: {}", term_id));
            }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        return Err("启动功能仅支持 macOS 和 Windows".to_string());
    }

    Ok(())
}
