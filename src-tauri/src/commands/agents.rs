use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tauri::State;

use crate::agents::{
    detect_all_agents, get_agent_config_paths, get_agent_name, resolve_path, DetectedAgent,
};
use crate::app_state::AppState;
use crate::database::McpApps;
use crate::import::import_from_path;
use crate::mcp::AppType;
use crate::services::sync;
use crate::services::tool_manager::which_binary;
use crate::utils::SuppressConsole;
use std::process::Command;
#[cfg(target_os = "macos")]
use std::{fs, os::unix::fs::PermissionsExt};

#[cfg(windows)]
fn powershell_escape_single_quoted(input: &str) -> String {
    input.replace('\'', "''")
}

#[cfg(target_os = "macos")]
fn shell_escape_single_quoted(input: &str) -> String {
    input.replace('\'', "'\\''")
}

#[cfg(target_os = "macos")]
fn yaml_escape_single_quoted(input: &str) -> String {
    input.replace('\'', "''")
}

#[cfg(target_os = "macos")]
fn resolve_preferred_macos_terminal(state: &AppState) -> String {
    let preferred = state
        .db
        .get_setting("default_terminal")
        .ok()
        .flatten()
        .unwrap_or_else(|| "terminal".to_string());

    let available = [
        (
            "terminal",
            [
                "/System/Applications/Utilities/Terminal.app",
                "/Applications/Terminal.app",
            ]
            .iter()
            .any(|path| std::path::Path::new(path).exists()),
        ),
        (
            "iterm",
            ["/Applications/iTerm.app", "/Applications/iTerm2.app"]
                .iter()
                .any(|path| std::path::Path::new(path).exists()),
        ),
        (
            "warp",
            ["/Applications/Warp.app", "/Applications/Warp Preview.app"]
                .iter()
                .any(|path| std::path::Path::new(path).exists()),
        ),
        (
            "ghostty",
            ["/Applications/Ghostty.app"]
                .iter()
                .any(|path| std::path::Path::new(path).exists()),
        ),
    ];

    if available
        .iter()
        .any(|(id, is_available)| *id == preferred && *is_available)
    {
        preferred
    } else {
        "terminal".to_string()
    }
}

#[cfg(target_os = "windows")]
fn resolve_preferred_windows_terminal(state: &AppState) -> String {
    let preferred = state
        .db
        .get_setting("default_terminal")
        .ok()
        .flatten()
        .unwrap_or_else(|| "windows-terminal".to_string());

    let has_wt = which_binary("wt").is_some();
    let has_powershell = which_binary("powershell").is_some();
    let has_cmd = which_binary("cmd").is_some();

    match preferred.as_str() {
        "windows-terminal" if has_wt => preferred,
        "powershell" if has_powershell => preferred,
        "command-prompt" if has_cmd => preferred,
        _ if has_wt => "windows-terminal".to_string(),
        _ if has_powershell => "powershell".to_string(),
        _ => "command-prompt".to_string(),
    }
}

/// 检测 Node.js 环境并返回需要添加到 PATH 的路径
/// 返回 Ok(bin_dir_path) 或 Err(error_message)
#[cfg(not(windows))]
fn detect_node_environment() -> Result<String, String> {
    let home = std::env::var("HOME").unwrap_or_default();

    // 先检测 node 是否已经可用 (直接用 which)
    if let Ok(output) = Command::new("which")
        .suppress_console()
        .arg("node")
        .output()
    {
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
            if let Some(newest) = entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.starts_with('v') {
                        e.path().join("bin/node").exists().then_some(name)
                    } else {
                        None
                    }
                })
                .max()
            {
                return Ok(format!("{}/.nvm/versions/node/{}/bin", home, newest));
            }
        }
    }

    // 检查 fnm
    let fnm_dir = format!("{}/.fnm", home);
    if std::path::Path::new(&fnm_dir).exists() {
        if let Ok(output) = Command::new("fnm")
            .suppress_console()
            .arg("current")
            .output()
        {
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
    if let Ok(output) = Command::new("where")
        .suppress_console()
        .arg("node")
        .output()
    {
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
    if let Ok(output) = Command::new("fnm")
        .suppress_console()
        .arg("current")
        .output()
    {
        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !version.is_empty() {
                let fnm_path = format!(
                    "{}\\AppData\\Roaming\\fnm\\versions\\{}\\installation",
                    home, version
                );
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
    let nvm_symlink = std::env::var("NVM_SYMLINK").unwrap_or_default();
    if !nvm_symlink.is_empty() {
        if std::path::Path::new(&nvm_symlink).join("node.exe").exists() {
            return Ok(nvm_symlink);
        }
    }
    if !nvm_home.is_empty() {
        if let Ok(entries) = std::fs::read_dir(&nvm_home) {
            if let Some(version_dir) = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .find(|path| path.join("node.exe").exists())
            {
                return Ok(version_dir.to_string_lossy().to_string());
            }
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
    if let Ok(output) = Command::new("cmd")
        .suppress_console()
        .args(["/C", "npm config get prefix"])
        .output()
    {
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

    let full_path = paths
        .first()
        .ok_or_else(|| format!("No config path found for {}", agent_id))?;

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
        let path_str = full_path.to_string_lossy().to_string();
        Command::new("cmd")
            .suppress_console()
            .args(["/c", "start", "", &path_str])
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
        AppType::Hermes => Some("hermes".to_string()),
    }
}

fn get_agent_launch_args(app: &AppType) -> Vec<String> {
    match app {
        AppType::Hermes => vec!["chat".to_string()],
        _ => vec![],
    }
}

fn launch_desktop_agent(app_type: &AppType) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        for app_name in crate::services::tool_manager::mac_app_names(app_type) {
            let app_path = format!("/Applications/{}", app_name);
            if std::path::Path::new(&app_path).exists() {
                Command::new("open")
                    .suppress_console()
                    .arg(&app_path)
                    .spawn()
                    .map_err(|e| format!("启动 {} 失败: {}", get_agent_name(app_type), e))?;
                return Ok(true);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(exe_path) = crate::services::tool_manager::find_app_executable_windows(app_type)
        {
            let exe_path = exe_path.to_string_lossy().to_string();
            let escaped_exe_path = powershell_escape_single_quoted(&exe_path);
            Command::new("powershell")
                .suppress_console()
                .args([
                    "-NoProfile",
                    "-Command",
                    &format!("Start-Process -FilePath '{}'", escaped_exe_path),
                ])
                .spawn()
                .map_err(|e| format!("启动 {} 失败: {}", get_agent_name(app_type), e))?;
            return Ok(true);
        }
    }

    Ok(false)
}

/// 启动 Agent 工具（打开默认终端并运行命令）
#[tauri::command]
pub async fn launch_agent(
    state: State<'_, AppState>,
    agent_id: String,
    working_dir: Option<String>,
    launch_kind: Option<String>,
) -> Result<(), String> {
    let app_type = AppType::from_str(&agent_id).map_err(|e| e.to_string())?;
    let launch_kind = launch_kind.unwrap_or_else(|| "auto".to_string());

    if launch_kind == "desktop" {
        if launch_desktop_agent(&app_type)? {
            return Ok(());
        }
        return Err(format!("未检测到 {} 桌面端", get_agent_name(&app_type)));
    }

    let Some(command) = get_agent_launch_command(&app_type) else {
        return Err(format!(
            "{} 没有 CLI 命令，无法启动",
            get_agent_name(&app_type)
        ));
    };
    let command_args = get_agent_launch_args(&app_type);

    let resolved_command = match which_binary(&command) {
        Some(path) => path,
        None => {
            if launch_kind == "auto" {
                if launch_desktop_agent(&app_type)? {
                    return Ok(());
                }
            }
            return Err(format!(
                "未检测到 {} 可执行文件，请先安装 {}",
                command,
                get_agent_name(&app_type)
            ));
        }
    };

    // Node-based CLIs need the user's package-manager bin path; Hermes only needs its own binary.
    let node_bin_dir = if matches!(app_type, AppType::Hermes) {
        String::new()
    } else {
        detect_node_environment().map_err(|e| format!("{}: 请先安装 Node.js", e))?
    };

    #[cfg(target_os = "macos")]
    {
        let terminal_app = resolve_preferred_macos_terminal(&state);
        let work_dir = working_dir
            .as_deref()
            .filter(|dir| !dir.trim().is_empty())
            .map(resolve_path)
            .or_else(|| dirs::desktop_dir().or_else(dirs::home_dir))
            .ok_or("无法获取启动目录")?
            .to_string_lossy()
            .to_string();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let script_path = format!(
            "/tmp/ai_toolkit_run_{}_{}.command",
            std::process::id(),
            timestamp
        );
        let escaped_work_dir = shell_escape_single_quoted(&work_dir);
        let escaped_command = shell_escape_single_quoted(&resolved_command);
        let escaped_args = command_args
            .iter()
            .map(|arg| format!("'{}'", shell_escape_single_quoted(arg)))
            .collect::<Vec<_>>()
            .join(" ");
        let path_export = if node_bin_dir.is_empty() {
            "export PATH=$PATH:/usr/local/bin:/opt/homebrew/bin".to_string()
        } else {
            format!(
                "export PATH='{}':$PATH:/usr/local/bin:/opt/homebrew/bin",
                shell_escape_single_quoted(&node_bin_dir)
            )
        };
        let shell_command = format!(
            "{} && '{}'{}{}",
            path_export,
            escaped_command,
            if escaped_args.is_empty() { "" } else { " " },
            escaped_args
        );
        let full_cmd = format!(
            "#!/bin/zsh\n\
             rm -f \"$0\" 2>/dev/null &\n\
             cd '{}' || exit 1\n\
             {}\n\
             exec $SHELL -l\n",
            escaped_work_dir, shell_command
        );
        fs::write(&script_path, &full_cmd).map_err(|e| format!("写入脚本失败: {}", e))?;
        fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("设置脚本权限失败: {}", e))?;

        let script = if terminal_app == "iterm" {
            format!(
                "tell application \"iTerm2\"\n\
                 activate\n\
                 create window with default profile command \"source {0}\"\n\
                 end tell",
                script_path
            )
        } else if terminal_app == "ghostty" {
            format!(
                "tell application \"Ghostty\"\n\
                 activate\n\
                 set ghosttyWindow to new window\n\
                 set ghosttyTerminal to focused terminal of selected tab of ghosttyWindow\n\
                 input text \"source {0}\" to ghosttyTerminal\n\
                 send key \"enter\" to ghosttyTerminal\n\
                 end tell",
                script_path
            )
        } else if terminal_app == "warp" {
            let (warp_app, warp_scheme) = if std::path::Path::new("/Applications/Warp.app").exists()
            {
                ("Warp", "warp")
            } else {
                ("Warp Preview", "warppreview")
            };
            let home = dirs::home_dir().ok_or("无法获取 Home 目录")?;
            let launch_dir = home.join(".warp").join("launch_configurations");
            fs::create_dir_all(&launch_dir)
                .map_err(|e| format!("创建 Warp 配置目录失败: {}", e))?;
            let launch_file_name = format!("ai-toolkit-{}.yaml", agent_id);
            let launch_file_path = launch_dir.join(&launch_file_name);
            let yaml_work_dir = yaml_escape_single_quoted(&work_dir);
            let yaml_command = yaml_escape_single_quoted(&shell_command);
            let launch_yaml = format!(
                "---\nname: AI Toolkit {0}\nwindows:\n  - tabs:\n      - title: {0}\n        layout:\n          cwd: '{1}'\n          commands:\n            - exec: '{2}'\n",
                get_agent_name(&app_type),
                yaml_work_dir,
                yaml_command
            );
            fs::write(&launch_file_path, launch_yaml)
                .map_err(|e| format!("写入 Warp 启动配置失败: {}", e))?;

            let warp_uri = format!("{warp_scheme}://launch/{launch_file_name}");
            let output = Command::new("open")
                .suppress_console()
                .args(["-a", warp_app, &warp_uri])
                .output()
                .map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("启动 {} 失败: {}", agent_id, stderr));
            }
            return Ok(());
        } else {
            format!(
                "tell application \"Terminal\"\n\
                 activate\n\
                 do script \"source {0}\"\n\
                 end tell",
                script_path
            )
        };
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
        let preferred_terminal = resolve_preferred_windows_terminal(&state);
        let desktop_path = working_dir
            .as_deref()
            .filter(|dir| !dir.trim().is_empty())
            .map(resolve_path)
            .or_else(|| dirs::desktop_dir().or_else(|| dirs::home_dir()))
            .ok_or("无法获取桌面路径")?
            .to_string_lossy()
            .to_string();
        let powershell_desktop_path = powershell_escape_single_quoted(&desktop_path);
        let powershell_command = powershell_escape_single_quoted(&resolved_command);
        let powershell_path_update = if node_bin_dir.is_empty() {
            String::new()
        } else {
            format!(
                "$env:PATH = '{};' + $env:PATH; ",
                powershell_escape_single_quoted(&node_bin_dir)
            )
        };
        let powershell_args = command_args
            .iter()
            .map(|arg| format!("'{}'", powershell_escape_single_quoted(arg)))
            .collect::<Vec<_>>()
            .join(",");
        let command_prompt_args = command_args
            .iter()
            .map(|arg| format!(" \"{}\"", arg.replace('"', "\\\"")))
            .collect::<String>();

        let powershell_full_cmd = if powershell_args.is_empty() {
            format!(
                "Set-Location -LiteralPath '{}'; {}& '{}'",
                powershell_desktop_path, powershell_path_update, powershell_command
            )
        } else {
            format!(
                "Set-Location -LiteralPath '{}'; {}& '{}' {}",
                powershell_desktop_path,
                powershell_path_update,
                powershell_command,
                powershell_args
            )
        };

        let command_prompt_cmd = if node_bin_dir.is_empty() {
            format!(
                "cd /d \"{}\" && \"{}\"{}",
                desktop_path, resolved_command, command_prompt_args
            )
        } else {
            format!(
                "cd /d \"{}\" && set \"PATH={};%PATH%\" && \"{}\"{}",
                desktop_path, node_bin_dir, resolved_command, command_prompt_args
            )
        };

        if preferred_terminal == "windows-terminal" {
            let try_windows_terminal = Command::new("wt")
                .args([
                    "new-tab",
                    "powershell",
                    "-NoExit",
                    "-Command",
                    &powershell_full_cmd,
                ])
                .spawn();
            if try_windows_terminal.is_ok() {
                return Ok(());
            }
        }

        if preferred_terminal == "powershell" || preferred_terminal == "windows-terminal" {
            let ps_start = format!(
                "Start-Process powershell -ArgumentList '-NoExit','-Command','{}'",
                powershell_full_cmd.replace('\'', "''")
            );
            let try_powershell = Command::new("powershell")
                .suppress_console()
                .args(["-NoProfile", "-Command", &ps_start])
                .spawn();
            if try_powershell.is_ok() {
                return Ok(());
            }
        }

        Command::new("cmd")
            .suppress_console()
            .args(["/C", "start", "", "cmd", "/K", &command_prompt_cmd])
            .spawn()
            .map_err(|e| format!("启动 {} 失败: {}", agent_id, e))?;
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        return Err("启动功能仅支持 macOS 和 Windows".to_string());
    }

    Ok(())
}
