use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use tauri::State;

use crate::agents::resolve_path;
use crate::app_state::AppState;
use crate::database::{McpServer, SkillRecord};
use crate::mcp::AppType;
use crate::services::sync;
use crate::utils::SuppressConsole;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancementSnapshot {
    pub id: String,
    pub reason: String,
    pub created_at: i64,
    pub server_count: usize,
    pub config_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDetail {
    pub snapshot: EnhancementSnapshot,
    pub servers: IndexMap<String, McpServer>,
    pub configs: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckItem {
    pub id: String,
    pub name: String,
    pub scope: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictItem {
    pub key: String,
    pub scope: String,
    pub message: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityFinding {
    pub scope: String,
    pub key: String,
    pub message: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolkitExport {
    pub version: u32,
    pub exported_at: i64,
    pub mcp_servers: IndexMap<String, McpServer>,
    pub skills: Vec<ExportedSkill>,
    pub settings: HashMap<String, String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedSkill {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub source_type: String,
    pub source_ref: Option<String>,
    pub source_subpath: Option<String>,
    pub central_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreview {
    pub mcp_count: usize,
    pub skill_count: usize,
    pub conflicts: Vec<ConflictItem>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub imported_mcp: usize,
    pub skipped_mcp: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchPreset {
    pub id: String,
    pub name: String,
    pub agent_id: String,
    pub working_dir: String,
    pub enabled_mcp_servers: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskLogEntry {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub status: String,
    pub created_at: i64,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavePresetParams {
    pub id: Option<String>,
    pub name: String,
    pub agent_id: String,
    pub working_dir: String,
    pub enabled_mcp_servers: Vec<String>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn toolkit_dir() -> Result<PathBuf, String> {
    dirs::home_dir()
        .ok_or_else(|| "无法找到用户目录".to_string())
        .map(|home| home.join(".ai-toolkit"))
}

fn snapshots_dir() -> Result<PathBuf, String> {
    Ok(toolkit_dir()?.join("snapshots"))
}

fn logs_path() -> Result<PathBuf, String> {
    Ok(toolkit_dir()?.join("task-logs.json"))
}

fn presets_path() -> Result<PathBuf, String> {
    Ok(toolkit_dir()?.join("launch-presets.json"))
}

fn snapshot_file(id: &str) -> Result<PathBuf, String> {
    Ok(snapshots_dir()?.join(format!("{id}.json")))
}

fn read_live_configs() -> HashMap<String, String> {
    let mut configs = HashMap::new();
    for app in AppType::all() {
        if let Ok(path) = sync::get_config_path_for_app(&app) {
            let resolved = resolve_path(&path);
            if let Ok(content) = fs::read_to_string(&resolved) {
                configs.insert(app.name().to_string(), content);
            }
        }
    }
    configs
}

fn snapshot_from_detail(detail: &SnapshotDetail) -> EnhancementSnapshot {
    detail.snapshot.clone()
}

fn read_snapshot_detail(path: &Path) -> Result<SnapshotDetail, String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

fn write_json_file<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, content).map_err(|e| e.to_string())?;
    fs::rename(&temp_path, path).map_err(|e| e.to_string())
}

pub fn create_snapshot_for_state(
    state: &AppState,
    reason: &str,
) -> Result<EnhancementSnapshot, String> {
    let servers = state.db.get_all_mcp_servers().map_err(|e| e.to_string())?;
    let configs = read_live_configs();
    let created_at = now_ms();
    let id = format!("snapshot-{created_at}");
    let snapshot = EnhancementSnapshot {
        id: id.clone(),
        reason: reason.to_string(),
        created_at,
        server_count: servers.len(),
        config_count: configs.len(),
    };
    let detail = SnapshotDetail {
        snapshot: snapshot.clone(),
        servers,
        configs,
    };
    write_json_file(&snapshot_file(&id)?, &detail)?;
    append_task_log(
        "snapshot",
        "已创建配置快照",
        &format!("{}，包含 {} 个 MCP Server", reason, snapshot.server_count),
        "success",
    )
    .ok();
    Ok(snapshot)
}

pub(crate) fn append_task_log(
    kind: &str,
    title: &str,
    detail: &str,
    status: &str,
) -> Result<(), String> {
    let mut logs = read_task_logs().unwrap_or_default();
    logs.insert(
        0,
        TaskLogEntry {
            id: format!("log-{}", now_ms()),
            kind: kind.to_string(),
            title: title.to_string(),
            detail: detail.to_string(),
            status: status.to_string(),
            created_at: now_ms(),
        },
    );
    logs.truncate(80);
    write_json_file(&logs_path()?, &logs)
}

fn read_task_logs() -> Result<Vec<TaskLogEntry>, String> {
    let path = logs_path()?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

fn read_presets() -> Result<Vec<LaunchPreset>, String> {
    let path = presets_path()?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

fn write_presets(presets: &[LaunchPreset]) -> Result<(), String> {
    write_json_file(&presets_path()?, &presets)
}

fn command_exists(command: &str) -> bool {
    if command.trim().is_empty() {
        return false;
    }
    let status = if cfg!(windows) {
        Command::new("where")
            .suppress_console()
            .arg(command)
            .status()
    } else {
        Command::new("sh")
            .arg("-lc")
            .arg(format!("command -v {}", shell_escape(command)))
            .status()
    };
    status.map(|s| s.success()).unwrap_or(false)
}

fn shell_escape(input: &str) -> String {
    format!("'{}'", input.replace('\'', "'\\''"))
}

fn sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_uppercase();
    [
        "TOKEN", "SECRET", "PASSWORD", "PASS", "API_KEY", "KEY", "AUTH",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn redact_servers(servers: &IndexMap<String, McpServer>) -> IndexMap<String, McpServer> {
    let mut redacted = servers.clone();
    for server in redacted.values_mut() {
        if let Some(env) = server.server.env.as_mut() {
            for (key, value) in env.iter_mut() {
                if sensitive_key(key) && !value.is_empty() {
                    *value = "***REDACTED***".to_string();
                }
            }
        }
        if let Some(headers) = server.server.headers.as_mut() {
            for (key, value) in headers.iter_mut() {
                if sensitive_key(key) && !value.is_empty() {
                    *value = "***REDACTED***".to_string();
                }
            }
        }
    }
    redacted
}

fn exported_skills(records: Vec<SkillRecord>) -> Vec<ExportedSkill> {
    records
        .into_iter()
        .map(|skill| ExportedSkill {
            id: skill.id,
            name: skill.name,
            description: skill.description,
            source_type: skill.source_type,
            source_ref: skill.source_ref,
            source_subpath: skill.source_subpath,
            central_path: skill.central_path,
        })
        .collect()
}

#[tauri::command]
pub async fn create_config_snapshot(
    state: State<'_, AppState>,
    reason: String,
) -> Result<EnhancementSnapshot, String> {
    create_snapshot_for_state(&state, &reason)
}

#[tauri::command]
pub async fn list_config_snapshots() -> Result<Vec<EnhancementSnapshot>, String> {
    let dir = snapshots_dir()?;
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut snapshots = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.path().extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        if let Ok(detail) = read_snapshot_detail(&entry.path()) {
            snapshots.push(snapshot_from_detail(&detail));
        }
    }
    snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(snapshots)
}

#[tauri::command]
pub async fn get_config_snapshot(id: String) -> Result<SnapshotDetail, String> {
    read_snapshot_detail(&snapshot_file(&id)?)
}

#[tauri::command]
pub async fn restore_config_snapshot(state: State<'_, AppState>, id: String) -> Result<(), String> {
    create_snapshot_for_state(&state, "恢复前自动备份")?;
    let detail = read_snapshot_detail(&snapshot_file(&id)?)?;
    {
        let conn = state.db.conn.lock();
        conn.execute("DELETE FROM mcp_servers", [])
            .map_err(|e| e.to_string())?;
    }
    for server in detail.servers.values() {
        state
            .db
            .save_mcp_server(server)
            .map_err(|e| e.to_string())?;
    }
    for (app_id, content) in &detail.configs {
        if let Ok(app) = AppType::from_str(app_id) {
            if let Ok(path) = sync::get_config_path_for_app(&app) {
                let resolved = resolve_path(&path);
                if let Some(parent) = resolved.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let mut file = fs::File::create(resolved).map_err(|e| e.to_string())?;
                file.write_all(content.as_bytes())
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    append_task_log("restore", "已恢复配置快照", &id, "success").ok();
    Ok(())
}

#[tauri::command]
pub async fn run_mcp_health_check(
    state: State<'_, AppState>,
) -> Result<Vec<HealthCheckItem>, String> {
    let servers = state.db.get_all_mcp_servers().map_err(|e| e.to_string())?;
    let mut items = Vec::new();

    for server in servers.values() {
        if server.server.command.is_none() && server.server.url.is_none() {
            items.push(HealthCheckItem {
                id: server.id.clone(),
                name: server.name.clone(),
                scope: "mcp".to_string(),
                status: "error".to_string(),
                message: "缺少 command 或 url".to_string(),
            });
            continue;
        }

        if let Some(command) = &server.server.command {
            let ok = command_exists(command);
            items.push(HealthCheckItem {
                id: server.id.clone(),
                name: server.name.clone(),
                scope: "command".to_string(),
                status: if ok { "ok" } else { "error" }.to_string(),
                message: if ok {
                    format!("命令可用: {command}")
                } else {
                    format!("找不到命令: {command}")
                },
            });
        }

        if let Some(env) = &server.server.env {
            for (key, value) in env {
                if value.trim().is_empty() || value == "***REDACTED***" {
                    items.push(HealthCheckItem {
                        id: server.id.clone(),
                        name: server.name.clone(),
                        scope: "env".to_string(),
                        status: "warn".to_string(),
                        message: format!("环境变量 {key} 为空或已脱敏"),
                    });
                }
            }
        }

        let enabled_count = AppType::all()
            .iter()
            .filter(|app| server.apps.is_enabled_for(app))
            .count();
        if enabled_count == 0 {
            items.push(HealthCheckItem {
                id: server.id.clone(),
                name: server.name.clone(),
                scope: "target".to_string(),
                status: "warn".to_string(),
                message: "未同步到任何工具".to_string(),
            });
        }
    }

    for app in AppType::all() {
        let path = sync::get_config_path_for_app(&app).map_err(|e| e.to_string())?;
        let resolved = resolve_path(&path);
        let exists = resolved.exists();
        items.push(HealthCheckItem {
            id: app.name().to_string(),
            name: app
                .get_install_info()
                .map(|info| info.name)
                .unwrap_or_else(|| app.name().to_string()),
            scope: "config".to_string(),
            status: if exists { "ok" } else { "warn" }.to_string(),
            message: if exists {
                format!("配置文件存在: {}", resolved.display())
            } else {
                format!("配置文件暂不存在: {}", resolved.display())
            },
        });
    }

    append_task_log(
        "health",
        "已完成 MCP 健康检查",
        &format!("{} 项结果", items.len()),
        "success",
    )
    .ok();
    Ok(items)
}

#[tauri::command]
pub async fn export_toolkit_config(state: State<'_, AppState>) -> Result<ToolkitExport, String> {
    let servers = state.db.get_all_mcp_servers().map_err(|e| e.to_string())?;
    let skills = state.db.get_all_skills().map_err(|e| e.to_string())?;
    let mut settings = HashMap::new();
    if let Ok(Some(default_terminal)) = state.db.get_setting("default_terminal") {
        settings.insert("default_terminal".to_string(), default_terminal);
    }
    let findings = scan_security_findings_for_servers(&servers);
    let warnings = findings
        .into_iter()
        .map(|finding| format!("{}: {}", finding.scope, finding.message))
        .collect();
    Ok(ToolkitExport {
        version: 1,
        exported_at: now_ms(),
        mcp_servers: redact_servers(&servers),
        skills: exported_skills(skills),
        settings,
        warnings,
    })
}

#[tauri::command]
pub async fn preview_toolkit_import(
    state: State<'_, AppState>,
    content: String,
) -> Result<ImportPreview, String> {
    let parsed: ToolkitExport = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    let current = state.db.get_all_mcp_servers().map_err(|e| e.to_string())?;
    let mut conflicts = Vec::new();
    for (id, incoming) in &parsed.mcp_servers {
        if let Some(existing) = current.get(id) {
            if serde_json::to_value(existing).ok() != serde_json::to_value(incoming).ok() {
                conflicts.push(ConflictItem {
                    key: id.clone(),
                    scope: "mcp".to_string(),
                    message: format!("MCP Server {id} 已存在且配置不同"),
                    severity: "warn".to_string(),
                });
            }
        }
    }
    Ok(ImportPreview {
        mcp_count: parsed.mcp_servers.len(),
        skill_count: parsed.skills.len(),
        conflicts,
        warnings: parsed.warnings,
    })
}

#[tauri::command]
pub async fn import_toolkit_config(
    state: State<'_, AppState>,
    content: String,
    overwrite: bool,
) -> Result<ImportResult, String> {
    create_snapshot_for_state(&state, "导入配置前自动备份")?;
    let parsed: ToolkitExport = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    let current = state.db.get_all_mcp_servers().map_err(|e| e.to_string())?;
    let mut imported_mcp = 0;
    let mut skipped_mcp = 0;
    for (id, server) in parsed.mcp_servers {
        if current.contains_key(&id) && !overwrite {
            skipped_mcp += 1;
            continue;
        }
        state
            .db
            .save_mcp_server(&server)
            .map_err(|e| e.to_string())?;
        imported_mcp += 1;
    }
    let servers = state.db.get_all_mcp_servers().map_err(|e| e.to_string())?;
    sync::sync_all_live_configs(&servers).map_err(|e| e.to_string())?;
    append_task_log(
        "import",
        "已导入配置包",
        &format!("导入 {imported_mcp} 个 MCP，跳过 {skipped_mcp} 个"),
        "success",
    )
    .ok();
    Ok(ImportResult {
        imported_mcp,
        skipped_mcp,
    })
}

fn scan_security_findings_for_servers(
    servers: &IndexMap<String, McpServer>,
) -> Vec<SecurityFinding> {
    let mut findings = Vec::new();
    for server in servers.values() {
        if let Some(env) = &server.server.env {
            for (key, value) in env {
                if sensitive_key(key) {
                    findings.push(SecurityFinding {
                        scope: server.name.clone(),
                        key: key.clone(),
                        message: "环境变量看起来包含敏感信息，导出时会默认脱敏".to_string(),
                        severity: if value == "***REDACTED***" {
                            "warn"
                        } else {
                            "info"
                        }
                        .to_string(),
                    });
                }
            }
        }
        if let Some(headers) = &server.server.headers {
            for key in headers.keys() {
                if sensitive_key(key) {
                    findings.push(SecurityFinding {
                        scope: server.name.clone(),
                        key: key.clone(),
                        message: "请求头看起来包含敏感信息，分享前请确认".to_string(),
                        severity: "warn".to_string(),
                    });
                }
            }
        }
    }
    findings
}

#[tauri::command]
pub async fn scan_security_findings(
    state: State<'_, AppState>,
) -> Result<Vec<SecurityFinding>, String> {
    let servers = state.db.get_all_mcp_servers().map_err(|e| e.to_string())?;
    Ok(scan_security_findings_for_servers(&servers))
}

#[tauri::command]
pub async fn detect_config_conflicts(
    state: State<'_, AppState>,
) -> Result<Vec<ConflictItem>, String> {
    let servers = state.db.get_all_mcp_servers().map_err(|e| e.to_string())?;
    let mut conflicts = Vec::new();
    let mut names: HashMap<String, Vec<String>> = HashMap::new();
    for server in servers.values() {
        names
            .entry(server.name.to_ascii_lowercase())
            .or_default()
            .push(server.id.clone());
    }
    for (name, ids) in names {
        if ids.len() > 1 {
            conflicts.push(ConflictItem {
                key: name,
                scope: "mcp-name".to_string(),
                message: format!("多个 MCP Server 使用相同名称: {}", ids.join(", ")),
                severity: "warn".to_string(),
            });
        }
    }

    let skills = state.db.get_all_skills().map_err(|e| e.to_string())?;
    let mut skill_names: HashMap<String, Vec<String>> = HashMap::new();
    for skill in skills {
        skill_names
            .entry(skill.name.to_ascii_lowercase())
            .or_default()
            .push(skill.id);
    }
    for (name, ids) in skill_names {
        if ids.len() > 1 {
            conflicts.push(ConflictItem {
                key: name,
                scope: "skill-name".to_string(),
                message: format!("多个 Skill 使用相同名称: {}", ids.join(", ")),
                severity: "warn".to_string(),
            });
        }
    }
    Ok(conflicts)
}

#[tauri::command]
pub async fn list_task_logs() -> Result<Vec<TaskLogEntry>, String> {
    read_task_logs()
}

#[tauri::command]
pub async fn record_task_log(
    kind: String,
    title: String,
    detail: String,
    status: String,
) -> Result<(), String> {
    append_task_log(&kind, &title, &detail, &status)
}

#[tauri::command]
pub async fn get_onboarding_checklist(
    state: State<'_, AppState>,
) -> Result<Vec<OnboardingChecklistItem>, String> {
    let servers = state.db.get_all_mcp_servers().map_err(|e| e.to_string())?;
    let skills = state.db.get_all_skills().map_err(|e| e.to_string())?;
    let snapshots = list_config_snapshots().await.unwrap_or_default();
    let has_terminal = state
        .db
        .get_setting("default_terminal")
        .map_err(|e| e.to_string())?
        .is_some();
    let installed_tools = state
        .installed_tools
        .read()
        .map(|report| report.agents.iter().filter(|agent| agent.exists).count())
        .unwrap_or(0);
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
            done: !skills.is_empty(),
            detail: format!("当前管理 {} 个 Skill", skills.len()),
        },
        OnboardingChecklistItem {
            id: "terminal".to_string(),
            title: "设置默认终端".to_string(),
            done: has_terminal,
            detail: if has_terminal {
                "已配置".to_string()
            } else {
                "使用系统默认".to_string()
            },
        },
        OnboardingChecklistItem {
            id: "backup".to_string(),
            title: "创建首个配置快照".to_string(),
            done: !snapshots.is_empty(),
            detail: format!("已有 {} 个快照", snapshots.len()),
        },
    ])
}

#[tauri::command]
pub async fn list_launch_presets() -> Result<Vec<LaunchPreset>, String> {
    read_presets()
}

#[tauri::command]
pub async fn save_launch_preset(params: SavePresetParams) -> Result<LaunchPreset, String> {
    let mut presets = read_presets()?;
    let now = now_ms();
    let id = params.id.unwrap_or_else(|| format!("preset-{now}"));
    let preset = LaunchPreset {
        id: id.clone(),
        name: params.name,
        agent_id: params.agent_id,
        working_dir: params.working_dir,
        enabled_mcp_servers: params.enabled_mcp_servers,
        created_at: presets
            .iter()
            .find(|preset| preset.id == id)
            .map(|preset| preset.created_at)
            .unwrap_or(now),
        updated_at: now,
    };
    if let Some(existing) = presets.iter_mut().find(|item| item.id == id) {
        *existing = preset.clone();
    } else {
        presets.push(preset.clone());
    }
    write_presets(&presets)?;
    append_task_log("preset", "已保存启动预设", &preset.name, "success").ok();
    Ok(preset)
}

#[tauri::command]
pub async fn delete_launch_preset(id: String) -> Result<(), String> {
    let mut presets = read_presets()?;
    presets.retain(|preset| preset.id != id);
    write_presets(&presets)
}

#[tauri::command]
pub async fn bulk_import_mcp_servers(
    state: State<'_, AppState>,
    servers: Vec<BulkMcpServerInput>,
    apps: HashMap<String, bool>,
    overwrite: bool,
) -> Result<BulkMcpImportResult, String> {
    create_snapshot_for_state(&state, "批量导入 MCP 前自动备份")?;
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
        for (app_id, enabled) in &apps {
            if let Ok(app) = AppType::from_str(app_id) {
                mcp_apps.set_enabled_for(&app, *enabled);
            }
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
    append_task_log(
        "mcp-import",
        "已批量导入 MCP",
        &format!("导入 {imported} 个，覆盖 {overwritten} 个"),
        "success",
    )
    .ok();
    Ok(BulkMcpImportResult {
        imported,
        overwritten,
    })
}

#[tauri::command]
pub async fn preview_skill_updates(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let skills = state.db.get_all_skills().map_err(|e| e.to_string())?;
    let mut preview = Vec::new();
    for skill in skills {
        let source = skill
            .source_ref
            .clone()
            .unwrap_or_else(|| skill.central_path.clone());
        let has_remote = source.starts_with("http://") || source.starts_with("https://");
        preview.push(json!({
            "id": skill.id,
            "name": skill.name,
            "source": source,
            "status": if has_remote { "可检查远端更新" } else { "本地技能" },
            "impact": "更新后会刷新中央仓库内容，并影响已同步目标",
            "last_sync_at": skill.last_sync_at,
        }));
    }
    Ok(preview)
}

#[tauri::command]
pub async fn build_share_package_summary(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let export = export_toolkit_config(state).await?;
    let server_ids: HashSet<String> = export.mcp_servers.keys().cloned().collect();
    Ok(json!({
        "mcp_count": export.mcp_servers.len(),
        "skill_count": export.skills.len(),
        "server_ids": server_ids,
        "warnings": export.warnings,
        "exported_at": export.exported_at,
    }))
}
