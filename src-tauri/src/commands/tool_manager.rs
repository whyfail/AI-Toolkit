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
    let mut tools = Vec::new();
    for app in AppType::all() {
        if let Some(mut info) = build_tool_info(&app).await {
            if info.installed {
                info.version = ToolManagerService::get_version(&app).await;
                info.latest_version = ToolManagerService::get_latest_version(&app).await;
                info.detected_method = ToolManagerService::detect_install_method(&app).await
                    .map(|m| match m {
                        crate::services::tool_manager::InstallMethodType::Brew => "Homebrew".to_string(),
                        crate::services::tool_manager::InstallMethodType::Npm => "npm".to_string(),
                        crate::services::tool_manager::InstallMethodType::Curl => "curl 脚本".to_string(),
                        crate::services::tool_manager::InstallMethodType::Winget => "Winget".to_string(),
                        crate::services::tool_manager::InstallMethodType::Scoop => "Scoop".to_string(),
                        crate::services::tool_manager::InstallMethodType::Custom => "自定义".to_string(),
                    });
            }
            tools.push(info);
        }
    }
    Ok(tools)
}

#[tauri::command]
pub async fn get_tool_info(app_type: String) -> Result<ToolInfo, String> {
    let app = AppType::from_str(&app_type)?;
    let mut info = build_tool_info(&app).await.ok_or("Unknown app type")?;
    if info.installed {
        info.version = ToolManagerService::get_version(&app).await;
        info.latest_version = ToolManagerService::get_latest_version(&app).await;
        info.detected_method = ToolManagerService::detect_install_method(&app).await
            .map(|m| match m {
                crate::services::tool_manager::InstallMethodType::Brew => "Homebrew".to_string(),
                crate::services::tool_manager::InstallMethodType::Npm => "npm".to_string(),
                crate::services::tool_manager::InstallMethodType::Curl => "curl 脚本".to_string(),
                crate::services::tool_manager::InstallMethodType::Winget => "Winget".to_string(),
                crate::services::tool_manager::InstallMethodType::Scoop => "Scoop".to_string(),
                crate::services::tool_manager::InstallMethodType::Custom => "自定义".to_string(),
            });
    }
    Ok(info)
}

#[tauri::command]
pub async fn install_tool(app_type: String, method_index: usize) -> Result<(), String> {
    let app = AppType::from_str(&app_type)?;
    let install_info = app.get_install_info().ok_or("Unknown app type")?;
    let method = install_info.methods.get(method_index).ok_or("Invalid method index")?;
    ToolManagerService::install(&app, method).await
}

#[tauri::command]
pub async fn update_tool(app_type: String) -> Result<(), String> {
    let app = AppType::from_str(&app_type)?;
    ToolManagerService::update(&app).await
}

#[tauri::command]
pub fn get_tool_homepage(app_type: String) -> Result<String, String> {
    let app = AppType::from_str(&app_type)?;
    let install_info = app.get_install_info().ok_or("Unknown app type")?;
    Ok(install_info.homepage)
}
