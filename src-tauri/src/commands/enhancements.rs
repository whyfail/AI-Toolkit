use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use tauri::State;

use crate::app_state::AppState;
use crate::database::McpServer;
use crate::mcp::AppType;
use crate::services::sync;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingChecklistItem {
    pub id: String,
    pub title: String,
    pub done: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkMcpServerInput {
    pub id: String,
    pub name: String,
    pub server: crate::database::McpServerSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkMcpImportResult {
    pub imported: usize,
    pub overwritten: usize,
}

#[tauri::command]
pub async fn get_onboarding_checklist(
    state: State<'_, AppState>,
) -> Result<Vec<OnboardingChecklistItem>, String> {
    let servers = state.db.get_all_mcp_servers().map_err(|e| e.to_string())?;
    let installed_tools = state
        .installed_tools
        .read()
        .map(|report| report.agents.iter().filter(|agent| agent.exists).count())
        .unwrap_or(0);
    let managed_skill_count = crate::commands::skills::get_managed_skills(state)
        .await
        .map(|skills| skills.len())?;
    Ok(vec![
        OnboardingChecklistItem {
            id: "scan-tools".to_string(),
            title: "扫描已安装工具".to_string(),
            done: installed_tools > 0,
            detail: format!("检测到 {installed_tools} 个工具"),
        },
        OnboardingChecklistItem {
            id: "import-mcp".to_string(),
            title: "导入或添加 MCP".to_string(),
            done: !servers.is_empty(),
            detail: format!("当前管理 {} 个 MCP Server", servers.len()),
        },
        OnboardingChecklistItem {
            id: "skills".to_string(),
            title: "安装或导入 Skills".to_string(),
            done: managed_skill_count > 0,
            detail: format!("当前管理 {} 个 Skill", managed_skill_count),
        },
    ])
}

#[tauri::command]
pub async fn bulk_import_mcp_servers(
    state: State<'_, AppState>,
    servers: Vec<BulkMcpServerInput>,
    apps: HashMap<String, bool>,
    overwrite: bool,
) -> Result<BulkMcpImportResult, String> {
    let _guard = state.mcp_gate.lock();
    let current = state.db.get_all_mcp_servers().map_err(|e| e.to_string())?;
    let mut imported = 0;
    let mut overwritten = 0;
    for input in servers {
        if current.contains_key(&input.id) && !overwrite {
            continue;
        }
        if current.contains_key(&input.id) {
            overwritten += 1;
        }
        let mut mcp_apps = crate::database::McpApps::default();
        let mut canonical_apps = HashMap::new();
        for (app_id, enabled) in &apps {
            if let Ok(app) = AppType::from_str(app_id) {
                canonical_apps
                    .entry(app.canonical())
                    .and_modify(|value| *value |= *enabled)
                    .or_insert(*enabled);
            }
        }
        for (app, enabled) in canonical_apps {
            mcp_apps.set_enabled_for(&app, enabled);
        }
        state
            .db
            .save_mcp_server(&McpServer {
                id: input.id,
                name: input.name,
                server: input.server,
                apps: mcp_apps,
                description: None,
                homepage: None,
                docs: None,
                tags: vec![],
            })
            .map_err(|e| e.to_string())?;
        imported += 1;
    }
    let all_servers = state.db.get_all_mcp_servers().map_err(|e| e.to_string())?;
    sync::sync_all_live_configs(&all_servers).map_err(|e| e.to_string())?;
    Ok(BulkMcpImportResult {
        imported,
        overwritten,
    })
}
