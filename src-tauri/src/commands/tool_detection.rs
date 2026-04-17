use tauri::State;
use crate::app_state::{AppState, InstalledToolsReport};
use crate::tool_detection::detect_all_tools;

/// 获取已安装工具的缓存数据（如果缓存为空则自动检测）
#[tauri::command]
pub fn get_installed_tools(state: State<'_, AppState>) -> InstalledToolsReport {
    let cache = state.installed_tools.read().unwrap_or_else(|e| e.into_inner()).clone();

    // 如果缓存为空（首次加载），自动触发检测
    if cache.tool_statuses.is_empty() && cache.agents.is_empty() {
        if let Ok(report) = detect_all_tools() {
            if let Ok(mut cache) = state.installed_tools.write() {
                *cache = report.clone();
            }
            return report;
        }
    }

    cache
}

/// 手动刷新已安装工具的检测（工具管理模块的刷新按钮）
#[tauri::command]
pub async fn refresh_installed_tools(state: State<'_, AppState>) -> Result<InstalledToolsReport, String> {
    let report = detect_all_tools()?;
    let mut cache = state.installed_tools.write().map_err(|e| e.to_string())?;
    *cache = report.clone();
    Ok(report)
}
