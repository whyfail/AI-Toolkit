use crate::mcp::AppType;
use crate::services::tool_manager::{build_tool_info, ToolManagerService};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMethodInfo {
    pub index: usize,
    pub method_type: String,
    pub name: String,
    pub package: Option<String>,
    pub url: Option<String>,
    pub command: String,
    pub needs_confirm: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub app_type: String,
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
    pub latest_version: Option<String>,
    pub detected_method: Option<String>,
    pub methods: Vec<ToolMethodInfo>,
    pub homepage: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallRequest {
    pub app_type: String,
    pub method_index: usize,
}

#[tauri::command]
pub async fn get_tool_infos() -> Result<Vec<ToolInfo>, String> {
    use futures::future;

    // 先收集所有 AppType，避免临时值问题
    let apps: Vec<_> = AppType::all();

    // 并行处理所有工具：每个工具独立执行，最后合并结果
    let futures: Vec<_> = apps
        .iter()
        .map(|app| async move {
            let info = match build_tool_info(app).await {
                Some(info) => info,
                None => return None,
            };
            // 返回基础安装信息 + 安装方式检测，版本号延迟到手动扫描时获取
            Some(info)
        })
        .collect();

    // 并行等待所有任务完成
    let results = future::join_all(futures).await;

    // 过滤掉 None 并收集结果
    let tools: Vec<ToolInfo> = results.into_iter().filter_map(|x| x).collect();
    Ok(tools)
}

#[tauri::command]
pub async fn get_tool_info(app_type: String) -> Result<ToolInfo, String> {
    let app = AppType::from_str(&app_type)?;
    let mut info = build_tool_info(&app).await.ok_or("Unknown app type")?;
    if info.installed {
        info.version = ToolManagerService::get_version(&app).await;
        info.latest_version = ToolManagerService::get_latest_version(&app).await;
        info.detected_method =
            ToolManagerService::detect_install_method(&app)
                .await
                .map(|m| m.display_name().to_string());
    }
    Ok(info)
}

#[tauri::command]
pub async fn install_tool(app_type: String, method_index: usize) -> Result<(), String> {
    let app = AppType::from_str(&app_type)?;
    let install_info = app.get_install_info().ok_or("Unknown app type")?;
    let method = install_info
        .methods
        .get(method_index)
        .ok_or("Invalid method index")?;
    ToolManagerService::install(&app, method).await
}

#[tauri::command]
pub async fn update_tool(app_type: String) -> Result<(), String> {
    let app = AppType::from_str(&app_type)?;
    ToolManagerService::update(&app).await
}

#[tauri::command]
pub async fn uninstall_tool(app_type: String) -> Result<(), String> {
    let app = AppType::from_str(&app_type)?;
    ToolManagerService::uninstall(&app).await
}

#[tauri::command]
pub fn get_tool_homepage(app_type: String) -> Result<String, String> {
    let app = AppType::from_str(&app_type)?;
    let install_info = app.get_install_info().ok_or("Unknown app type")?;
    Ok(install_info.homepage)
}

/// 后台批量扫描所有已安装工具的版本号和最新版本号，返回更新后的 ToolInfo 列表
#[tauri::command]
pub async fn scan_all_tool_versions() -> Result<Vec<ToolInfo>, String> {
    use futures::future;

    let apps: Vec<_> = AppType::all();

    let futures: Vec<_> = apps
        .iter()
        .map(|app| async move {
            let mut info = match build_tool_info(app).await {
                Some(info) => info,
                None => return None,
            };
            if info.installed {
                info.version = ToolManagerService::get_version(app).await;
                info.latest_version = ToolManagerService::get_latest_version(app).await;
            }
            Some(info)
        })
        .collect();

    let results = future::join_all(futures).await;
    let tools: Vec<ToolInfo> = results.into_iter().filter_map(|x| x).collect();
    Ok(tools)
}
