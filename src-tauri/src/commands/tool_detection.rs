use crate::app_state::{AppState, InstalledToolsReport};
use crate::tool_detection::detect_all_tools;
use tauri::{AppHandle, Emitter, State};

/// 获取已安装 Agent 的扫描结果，首个调用不会返回尚未扫描的空缓存。
#[tauri::command]
pub async fn get_installed_tools(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<InstalledToolsReport, String> {
    let report = state
        .installed_tools
        .read()
        .map_err(|e| e.to_string())?
        .clone();
    if report.detected_at != 0 {
        return Ok(report);
    }
    refresh_installed_tools(state, app).await
}

/// 扫描只读取本地安装信息，结果供所有模块共用。
#[tauri::command]
pub async fn refresh_installed_tools(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<InstalledToolsReport, String> {
    let report = tauri::async_runtime::spawn_blocking(detect_all_tools)
        .await
        .map_err(|e| e.to_string())??;
    {
        let mut cache = state.installed_tools.write().map_err(|e| e.to_string())?;
        *cache = report.clone();
    }
    app.emit("installed-tools-updated", &report)
        .map_err(|e| e.to_string())?;
    Ok(report)
}
