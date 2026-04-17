pub mod agents;
pub mod app_state;
pub mod commands;
pub mod core;
pub mod database;
pub mod error;
pub mod import;
pub mod mcp;
pub mod services;
pub mod skill_core;
pub mod tool_detection;

use agents::{get_last_detected_agents, save_detected_agents};
use app_state::AppState;
use database::Database;
use tauri::{Emitter, Manager, async_runtime::spawn};
use std::time::Duration;
use tool_detection::detect_all_tools;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
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
                        let current_names: Vec<String> = report.agents
                            .iter()
                            .filter(|a| a.exists)
                            .map(|a| a.id.clone())
                            .collect();
                        let new_agents: Vec<_> = report.agents
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
            // 应用配置命令
            commands::app::get_app_configs,
            commands::app::import_mcp_from_app,
            // Agent 检测命令
            commands::agents::detect_agents,
            commands::agents::sync_agent_mcp,
            commands::agents::open_config_file,
            commands::agents::launch_agent,
            commands::agents::get_terminals,
            // 技能管理命令
            commands::skills::get_managed_skills,
            commands::skills::get_tool_status,
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
            // 更新命令
            commands::update::check_update,
            commands::update::install_update,
            // 工具管理命令
            commands::tool_manager::get_tool_infos,
            commands::tool_manager::get_tool_info,
            commands::tool_manager::install_tool,
            commands::tool_manager::update_tool,
            commands::tool_manager::get_tool_homepage,
            // 统一工具检测命令
            commands::tool_detection::get_installed_tools,
            commands::tool_detection::refresh_installed_tools,
            // 应用信息命令
            commands::app::get_version,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
