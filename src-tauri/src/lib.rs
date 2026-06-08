pub mod agents;
pub mod app_state;
pub mod commands;
pub mod core;
pub mod database;
pub mod error;
pub mod import;
pub mod mcp;
pub mod migration;
pub mod services;
pub mod skill_core;
pub mod tool_detection;
pub mod utils;

use agents::{get_last_detected_agents, save_detected_agents};
use app_state::AppState;
use database::Database;
use std::time::Duration;
use tauri::{async_runtime::spawn, Emitter, Manager};
use tool_detection::detect_all_tools;

#[cfg(not(windows))]
fn push_path_parts(parts: &mut Vec<String>, path: &str) {
    for part in path.split(':') {
        let part = part.trim();
        if !part.is_empty() && !parts.iter().any(|existing| existing == part) {
            parts.push(part.to_string());
        }
    }
}

#[cfg(windows)]
fn expand_path_from_shell() {}

/// 从用户的 shell 环境中获取完整 PATH 并扩展到当前进程环境中。
/// 解决 macOS .app 进程 PATH 不完整的问题（缺少 Homebrew/nvm 等路径）。
#[cfg(not(windows))]
fn expand_path_from_shell() {
    let current_path = std::env::var("PATH").unwrap_or_default();
    let mut paths = Vec::new();
    push_path_parts(&mut paths, &current_path);

    // GUI 启动的 .app 不会继承用户 shell 的 PATH。login shell 覆盖
    // .zprofile/.profile，interactive login shell 额外覆盖 .zshrc 里的
    // nvm/mise/asdf/bun/pnpm 等初始化。
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    for mode in ["-lc", "-lic"] {
        if let Ok(output) = std::process::Command::new(&shell)
            .args([mode, "printf '\\n__AI_TOOLKIT_PATH__%s\\n' \"$PATH\""])
            .output()
        {
            if output.status.success() {
                let shell_path = String::from_utf8_lossy(&output.stdout);
                if let Some(path_line) = shell_path
                    .lines()
                    .rev()
                    .find_map(|line| line.strip_prefix("__AI_TOOLKIT_PATH__"))
                {
                    push_path_parts(&mut paths, path_line);
                }
            }
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        for path in [
            format!("{}/.local/bin", home),
            format!("{}/.cargo/bin", home),
            format!("{}/.volta/bin", home),
            format!("{}/.bun/bin", home),
            format!("{}/Library/pnpm", home),
            format!("{}/.pnpm-global/bin", home),
            format!("{}/.yarn/bin", home),
            format!("{}/.asdf/shims", home),
            format!("{}/.local/share/mise/shims", home),
            format!("{}/.nvmd/bin", home),
        ] {
            push_path_parts(&mut paths, &path);
        }
    }

    for path in [
        "/opt/homebrew/bin",
        "/opt/homebrew/sbin",
        "/usr/local/bin",
        "/usr/local/sbin",
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
    ] {
        push_path_parts(&mut paths, path);
    }

    std::env::set_var("PATH", paths.join(":"));
}

pub fn run() {
    expand_path_from_shell();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // 迁移旧数据目录 ~/.ai-tool-manager/ → ~/.ai-toolkit/
            // 必须在数据库初始化之前执行
            migration::migrate_from_old_dir();

            // 初始化数据库
            let db = Database::new()?;
            app.manage(AppState::new(db));

            // 自动导入所有已存在的 MCP 配置
            let state = app.state::<AppState>();
            let imported_servers = import::import_all();
            for (_, server) in imported_servers {
                let _ = state.db.save_mcp_server(&server);
            }

            // 在后台异步检测新安装的 Agent 工具，延迟发送事件确保前端已就绪
            let app_handle = app.app_handle().clone();
            spawn(async move {
                // 等待 1 秒，确保前端 React 应用已挂载并完成事件监听器注册
                tokio::time::sleep(Duration::from_secs(1)).await;

                // 执行统一的工具检测并缓存结果
                match detect_all_tools() {
                    Ok(report) => {
                        // 检测新安装的工具
                        let previous = get_last_detected_agents();
                        let current_names: Vec<String> = report
                            .agents
                            .iter()
                            .filter(|a| a.exists)
                            .map(|a| a.id.clone())
                            .collect();
                        let new_agents: Vec<_> = report
                            .agents
                            .iter()
                            .filter(|a| a.exists && !previous.contains(&a.id))
                            .cloned()
                            .collect();

                        // 保存检测状态并更新缓存
                        save_detected_agents(&current_names);

                        // 在异步块内获取 state 来更新缓存
                        if let Some(state) = app_handle.try_state::<AppState>() {
                            if let Ok(mut cache) = state.installed_tools.write() {
                                *cache = report.clone();
                            }
                        }

                        let _ = app_handle.emit("installed-tools-updated", &report);

                        // 如果有新工具，通过事件通知前端
                        if !new_agents.is_empty() {
                            let _ = app_handle.emit("agents-detected", &new_agents);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to detect tools: {}", e);
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // MCP 命令
            commands::mcp::get_mcp_servers,
            commands::mcp::upsert_mcp_server,
            commands::mcp::delete_mcp_server,
            commands::mcp::toggle_mcp_app,
            commands::mcp::import_mcp_from_apps,
            commands::mcp::test_mcp_connection,
            // 增强中心命令
            commands::enhancements::create_config_snapshot,
            commands::enhancements::list_config_snapshots,
            commands::enhancements::get_config_snapshot,
            commands::enhancements::restore_config_snapshot,
            commands::enhancements::run_mcp_health_check,
            commands::enhancements::export_toolkit_config,
            commands::enhancements::preview_toolkit_import,
            commands::enhancements::import_toolkit_config,
            commands::enhancements::scan_security_findings,
            commands::enhancements::detect_config_conflicts,
            commands::enhancements::list_task_logs,
            commands::enhancements::record_task_log,
            commands::enhancements::get_onboarding_checklist,
            commands::enhancements::list_launch_presets,
            commands::enhancements::save_launch_preset,
            commands::enhancements::delete_launch_preset,
            commands::enhancements::bulk_import_mcp_servers,
            commands::enhancements::preview_skill_updates,
            commands::enhancements::build_share_package_summary,
            // 应用配置命令
            commands::app::get_app_configs,
            commands::app::get_launch_preferences,
            commands::app::import_mcp_from_app,
            commands::app::set_default_terminal,
            // Agent 检测命令
            commands::agents::detect_agents,
            commands::agents::sync_agent_mcp,
            commands::agents::open_config_file,
            commands::agents::launch_agent,
            // 技能管理命令
            commands::skills::get_managed_skills,
            commands::skills::get_tool_status,
            commands::skills::open_skill_path,
            commands::skills::preview_skill_delete,
            commands::skills::run_skill_health_check,
            commands::skills::detect_skill_name_conflicts,
            commands::skills::get_onboarding_plan,
            commands::skills::install_git,
            commands::skills::list_git_skills,
            commands::skills::install_git_selection,
            commands::skills::install_local_selection,
            commands::skills::sync_skill_to_tool,
            commands::skills::unsync_skill_from_tool,
            commands::skills::import_existing_skill,
            commands::skills::delete_managed_skill,
            commands::skills::update_skill,
            commands::skills::rename_skill,
            commands::skills::get_skill_readme,
            commands::skills::search_skills_online,
            commands::skills::get_featured_skills,
            commands::skills::validate_local_skill,
            // 更新命令
            commands::update::check_update,
            commands::update::install_update,
            // 工具管理命令
            commands::tool_manager::get_tool_infos,
            commands::tool_manager::get_tool_info,
            commands::tool_manager::install_tool,
            commands::tool_manager::update_tool,
            commands::tool_manager::uninstall_tool,
            commands::tool_manager::get_tool_homepage,
            commands::tool_manager::scan_all_tool_versions,
            // 统一工具检测命令
            commands::tool_detection::get_installed_tools,
            commands::tool_detection::refresh_installed_tools,
            // 应用信息命令
            commands::app::get_version,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
