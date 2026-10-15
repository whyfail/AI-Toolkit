use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_state::AppState;
use crate::core::central_repo::{
    get_skills_install_location, normalize_skills_install_location,
    resolve_central_repo_path_for_location, set_skills_install_location, AGENTS_SKILLS_LOCATION_ID,
    AI_TOOLKIT_SKILLS_LOCATION_ID, SKILLS_INSTALL_LOCATION_SETTING_KEY,
};
use crate::mcp::AppType;
use crate::services::McpService;

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
pub struct SkillsInstallLocationOption {
    pub id: String,
    pub label: String,
    pub path: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsInstallPreferences {
    pub selected: String,
    pub options: Vec<SkillsInstallLocationOption>,
}

pub fn init_skills_install_location(state: &AppState) {
    let stored = state
        .db
        .get_setting(SKILLS_INSTALL_LOCATION_SETTING_KEY)
        .ok()
        .flatten()
        .and_then(|id| normalize_skills_install_location(&id).map(str::to_string))
        .unwrap_or_else(|| AGENTS_SKILLS_LOCATION_ID.to_string());
    let _ = set_skills_install_location(&stored);
}

#[tauri::command]
pub async fn get_skills_install_preferences() -> Result<SkillsInstallPreferences, String> {
    let options = [
        (
            AGENTS_SKILLS_LOCATION_ID,
            "官方 Skills 目录（.agents/skills）",
        ),
        (
            AI_TOOLKIT_SKILLS_LOCATION_ID,
            "AI Toolkit 目录（.ai-toolkit/skills）",
        ),
    ]
    .into_iter()
    .map(|(id, label)| {
        resolve_central_repo_path_for_location(id).map(|path| SkillsInstallLocationOption {
            id: id.to_string(),
            label: label.to_string(),
            path: path.to_string_lossy().to_string(),
        })
    })
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;

    Ok(SkillsInstallPreferences {
        selected: get_skills_install_location(),
        options,
    })
}

#[tauri::command]
pub async fn set_skills_install_location_preference(
    state: State<'_, AppState>,
    location_id: String,
) -> Result<(), String> {
    let normalized = set_skills_install_location(&location_id).map_err(|e| e.to_string())?;
    state
        .db
        .set_setting(SKILLS_INSTALL_LOCATION_SETTING_KEY, &normalized)
        .map_err(|e| e.to_string())
}

/// 获取应用配置路径
#[tauri::command]
pub async fn get_app_configs(state: State<'_, AppState>) -> Result<Vec<AppConfigInfo>, String> {
    let report = state.installed_tools.read().map_err(|e| e.to_string())?;
    let configs = report
        .agents
        .iter()
        .filter(|agent| agent.exists)
        .map(|agent| AppConfigInfo {
            id: agent.id.clone(),
            name: agent.name.clone(),
            config_path: agent.config_path.clone(),
            mcp_count: agent.mcp_count,
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
