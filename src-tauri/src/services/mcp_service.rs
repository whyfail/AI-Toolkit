use crate::agents::resolve_path;
use crate::app_state::AppState;
use crate::database::{McpApps, McpServer, McpServerSpec};
use crate::error::AppError;
use crate::mcp::AppType;
use crate::services::sync;
use indexmap::IndexMap;
use std::fs;
use std::path::Path;

/// MCP 服务业务逻辑层
pub struct McpService;

impl McpService {
    /// 获取所有 MCP 服务器
    pub fn get_all_servers(
        state: &tauri::State<AppState>,
    ) -> Result<IndexMap<String, McpServer>, AppError> {
        state.db.get_all_mcp_servers()
    }

    /// 添加或更新 MCP 服务器
    pub fn upsert_server(
        state: &tauri::State<AppState>,
        server: McpServer,
    ) -> Result<(), AppError> {
        state.db.save_mcp_server(&server)?;
        // 同步到配置文件
        let servers = state.db.get_all_mcp_servers()?;
        sync::sync_all_live_configs(&servers)?;
        Ok(())
    }

    /// 删除 MCP 服务器
    pub fn delete_server(state: &tauri::State<AppState>, id: &str) -> Result<(), AppError> {
        state.db.delete_mcp_server(id)?;
        // 同步到配置文件
        let servers = state.db.get_all_mcp_servers()?;
        sync::sync_all_live_configs(&servers)?;
        Ok(())
    }

    /// 切换指定应用的启用状态
    pub fn toggle_app(
        state: &tauri::State<AppState>,
        server_id: &str,
        app: AppType,
        enabled: bool,
    ) -> Result<(), AppError> {
        let mut servers = state.db.get_all_mcp_servers()?;

        if let Some(server) = servers.get_mut(server_id) {
            server.apps.set_enabled_for(&app, enabled);
            state.db.save_mcp_server(server)?;
            // 同步到配置文件
            sync::sync_all_live_configs(&servers)?;
            Ok(())
        } else {
            Err(AppError::NotFound(format!(
                "MCP server not found: {}",
                server_id
            )))
        }
    }

    /// 从所有应用导入 MCP 服务器
    pub fn import_from_apps(state: &tauri::State<AppState>) -> Result<usize, AppError> {
        let mut imported = Vec::new();
        for app in [
            AppType::Claude,
            AppType::Codex,
            AppType::Gemini,
            AppType::OpenCode,
            AppType::QwenCode,
            AppType::Trae,
            AppType::TraeCn,
            AppType::TraeSoloCn,
            AppType::Qoder,
            AppType::Qodercli,
            AppType::CodeBuddy,
        ] {
            imported.extend(read_servers_from_app(app)?);
        }

        save_imported_servers(state, imported)
    }

    /// 从指定应用导入 MCP 服务器
    pub fn import_from_app(
        state: &tauri::State<AppState>,
        app: AppType,
    ) -> Result<usize, AppError> {
        save_imported_servers(state, read_servers_from_app(app)?)
    }
}

fn read_servers_from_app(app: AppType) -> Result<Vec<(AppType, McpServer)>, AppError> {
    let config_path = resolve_path(&sync::get_config_path_for_app(&app)?);
    if !config_path.exists() {
        return Ok(vec![]);
    }

    let servers = if matches!(app, AppType::Codex) {
        import_codex_servers(&config_path)?
    } else {
        import_json_servers(&config_path, &app)?
    };

    Ok(servers
        .into_iter()
        .map(|server| (app.clone(), server))
        .collect())
}

fn save_imported_servers(
    state: &tauri::State<AppState>,
    imported: Vec<(AppType, McpServer)>,
) -> Result<usize, AppError> {
    if imported.is_empty() {
        return Ok(0);
    }

    let mut imported_count = 0;
    for (app, mut server) in imported {
        server.apps.set_enabled_for(&app, true);
        state.db.save_mcp_server(&server)?;
        imported_count += 1;
    }

    let servers = state.db.get_all_mcp_servers()?;
    sync::sync_all_live_configs(&servers)?;

    Ok(imported_count)
}

fn import_json_servers(config_path: &Path, app: &AppType) -> Result<Vec<McpServer>, AppError> {
    let content = fs::read_to_string(config_path)?;
    let config: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| AppError::Parse(e.to_string()))?;
    let key = if matches!(app, AppType::OpenCode) {
        "mcp"
    } else {
        "mcpServers"
    };

    let Some(servers) = config.get(key).and_then(|value| value.as_object()) else {
        return Ok(vec![]);
    };

    servers
        .iter()
        .map(|(id, value)| {
            let spec = if matches!(app, AppType::OpenCode) {
                parse_opencode_server(value)?
            } else {
                serde_json::from_value::<McpServerSpec>(value.clone())
                    .map_err(|e| AppError::Parse(e.to_string()))?
            };
            Ok(imported_server(id, spec))
        })
        .collect()
}

fn import_codex_servers(config_path: &Path) -> Result<Vec<McpServer>, AppError> {
    let content = fs::read_to_string(config_path)?;
    let config: toml::Value =
        toml::from_str(&content).map_err(|e| AppError::Parse(e.to_string()))?;
    let Some(servers) = config.get("mcp_servers").and_then(|value| value.as_table()) else {
        return Ok(vec![]);
    };

    servers
        .iter()
        .map(|(id, value)| {
            let json_value =
                serde_json::to_value(value).map_err(|e| AppError::Serialization(e.to_string()))?;
            let spec = serde_json::from_value::<McpServerSpec>(json_value)
                .map_err(|e| AppError::Parse(e.to_string()))?;
            Ok(imported_server(id, spec))
        })
        .collect()
}

fn parse_opencode_server(value: &serde_json::Value) -> Result<McpServerSpec, AppError> {
    let mut spec = McpServerSpec::default();
    let Some(object) = value.as_object() else {
        return Err(AppError::Parse(
            "OpenCode MCP entry must be an object".to_string(),
        ));
    };

    if let Some(url) = object.get("url").and_then(|value| value.as_str()) {
        spec.url = Some(url.to_string());
    }
    if let Some(headers) = object.get("headers").and_then(|value| value.as_object()) {
        spec.headers = Some(
            headers
                .iter()
                .filter_map(|(key, value)| value.as_str().map(|v| (key.clone(), v.to_string())))
                .collect(),
        );
    }
    if let Some(command) = object.get("command") {
        match command {
            serde_json::Value::Array(parts) => {
                let parts: Vec<String> = parts
                    .iter()
                    .filter_map(|part| part.as_str().map(ToString::to_string))
                    .collect();
                if let Some((command, args)) = parts.split_first() {
                    spec.command = Some(command.clone());
                    if !args.is_empty() {
                        spec.args = Some(args.to_vec());
                    }
                }
            }
            serde_json::Value::String(command) => {
                spec.command = Some(command.clone());
            }
            _ => {}
        }
    }
    if let Some(environment) = object
        .get("environment")
        .and_then(|value| value.as_object())
    {
        spec.env = Some(
            environment
                .iter()
                .filter_map(|(key, value)| value.as_str().map(|v| (key.clone(), v.to_string())))
                .collect(),
        );
    }

    spec.spec_type = object
        .get("type")
        .and_then(|value| value.as_str())
        .map(ToString::to_string);

    for (key, value) in object {
        if !matches!(
            key.as_str(),
            "type" | "command" | "environment" | "url" | "headers"
        ) {
            spec.extra.insert(key.clone(), value.clone());
        }
    }

    Ok(spec)
}

fn imported_server(id: &str, spec: McpServerSpec) -> McpServer {
    McpServer {
        id: id.to_string(),
        name: id.to_string(),
        server: spec,
        apps: McpApps::default(),
        description: None,
        homepage: None,
        docs: None,
        tags: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_file(name: &str, content: &str) -> tempfile::NamedTempFile {
        let mut file = tempfile::Builder::new()
            .prefix(name)
            .tempfile()
            .expect("create temp file");
        file.write_all(content.as_bytes()).expect("write temp file");
        file
    }

    #[test]
    fn imports_standard_json_mcp_servers() {
        let file = temp_file(
            "mcp-json",
            r#"{
              "mcpServers": {
                "demo": {
                  "command": "npx",
                  "args": ["-y", "demo-server"],
                  "env": { "TOKEN": "secret" }
                }
              }
            }"#,
        );

        let servers = import_json_servers(file.path(), &AppType::Claude).expect("import json");

        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].id, "demo");
        assert_eq!(servers[0].server.command.as_deref(), Some("npx"));
        assert_eq!(
            servers[0].server.args.as_deref(),
            Some(&["-y".to_string(), "demo-server".to_string()][..])
        );
        assert_eq!(
            servers[0]
                .server
                .env
                .as_ref()
                .and_then(|env| env.get("TOKEN"))
                .map(String::as_str),
            Some("secret")
        );
    }

    #[test]
    fn imports_opencode_command_array() {
        let file = temp_file(
            "mcp-opencode",
            r#"{
              "mcp": {
                "demo": {
                  "type": "local",
                  "command": ["node", "server.js"],
                  "environment": { "A": "B" }
                }
              }
            }"#,
        );

        let servers = import_json_servers(file.path(), &AppType::OpenCode).expect("import json");

        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].server.command.as_deref(), Some("node"));
        assert_eq!(
            servers[0].server.args.as_deref(),
            Some(&["server.js".to_string()][..])
        );
        assert_eq!(
            servers[0]
                .server
                .env
                .as_ref()
                .and_then(|env| env.get("A"))
                .map(String::as_str),
            Some("B")
        );
    }

    #[test]
    fn imports_codex_toml_mcp_servers() {
        let file = temp_file(
            "mcp-codex",
            r#"
              [mcp_servers.demo]
              command = "uvx"
              args = ["demo"]
              [mcp_servers.demo.env]
              TOKEN = "secret"
            "#,
        );

        let servers = import_codex_servers(file.path()).expect("import toml");

        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].id, "demo");
        assert_eq!(servers[0].server.command.as_deref(), Some("uvx"));
        assert_eq!(
            servers[0].server.args.as_deref(),
            Some(&["demo".to_string()][..])
        );
        assert_eq!(
            servers[0]
                .server
                .env
                .as_ref()
                .and_then(|env| env.get("TOKEN"))
                .map(String::as_str),
            Some("secret")
        );
    }
}
