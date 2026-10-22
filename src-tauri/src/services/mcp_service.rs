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
        let _guard = state.mcp_gate.lock();
        state.db.save_mcp_server(&server)?;
        // 同步到配置文件
        let servers = state.db.get_all_mcp_servers()?;
        sync::sync_all_live_configs(&servers)?;
        Ok(())
    }

    /// 删除 MCP 服务器
    pub fn delete_server(state: &tauri::State<AppState>, id: &str) -> Result<(), AppError> {
        let _guard = state.mcp_gate.lock();
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
        let _guard = state.mcp_gate.lock();
        if enabled && !crate::services::local_detection::is_agent_installed(&app) {
            return Err(AppError::NotFound(format!(
                "未检测到已安装的 {}",
                crate::agents::get_agent_name(&app)
            )));
        }
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
        let _guard = state.mcp_gate.lock();
        let mut imported = Vec::new();
        let mut paths = std::collections::HashSet::new();
        for agent in crate::agents::detect_all_agents() {
            let app = agent.app_type;
            let path = resolve_path(&sync::get_config_path_for_app(&app)?);
            if paths.insert(crate::agents::normalized_path_key(&path)) {
                imported.extend(read_servers_from_app(app)?);
            }
        }

        save_imported_servers(state, imported)
    }

    /// 从指定应用导入 MCP 服务器
    pub fn import_from_app(
        state: &tauri::State<AppState>,
        app: AppType,
    ) -> Result<usize, AppError> {
        let _guard = state.mcp_gate.lock();
        save_imported_servers(state, read_servers_from_app(app.canonical())?)
    }
}

fn read_servers_from_app(app: AppType) -> Result<Vec<(AppType, McpServer)>, AppError> {
    let config_path = resolve_path(&sync::get_config_path_for_app(&app)?);
    if !config_path.exists() {
        return Ok(vec![]);
    }

    let servers = match app {
        AppType::Codex => import_codex_servers(&config_path)?,
        AppType::Hermes => import_hermes_servers(&config_path)?,
        AppType::Zcode => import_zcode_servers(&config_path)?,
        AppType::DeepSeekHarness => {
            crate::services::deepseek_harness::parse_servers(&fs::read_to_string(&config_path)?)?
                .into_values()
                .collect()
        }
        AppType::Continue | AppType::Goose => {
            crate::import::parse_agent_yaml_servers(&fs::read_to_string(&config_path)?, &app)?
                .into_values()
                .collect()
        }
        _ => import_json_servers(&config_path, &app)?,
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

    let count = imported
        .iter()
        .map(|(_, server)| server.id.clone())
        .collect::<std::collections::HashSet<_>>()
        .len();
    let current = state.db.get_all_mcp_servers()?;
    let servers = merge_imported_servers(current, imported)?;
    state
        .db
        .save_mcp_servers(&servers.values().cloned().collect::<Vec<_>>())?;
    sync::sync_all_live_configs(&servers)?;
    Ok(count)
}

pub(crate) fn merge_imported_servers(
    mut current: IndexMap<String, McpServer>,
    imported: Vec<(AppType, McpServer)>,
) -> Result<IndexMap<String, McpServer>, AppError> {
    for (app, mut server) in imported {
        if let Some(existing) = current.get_mut(&server.id) {
            if existing.server != server.server {
                return Err(AppError::Parse(format!(
                    "MCP 服务器 {} 存在不同配置，导入已取消，请先处理同名冲突",
                    server.id
                )));
            }
            existing.apps.set_enabled_for(&app.canonical(), true);
        } else {
            server.apps.set_enabled_for(&app.canonical(), true);
            current.insert(server.id.clone(), server);
        }
    }
    Ok(current)
}

pub(crate) fn import_on_startup(db: &crate::database::Database) -> Result<(), AppError> {
    let current = merge_startup_imports(db.get_all_mcp_servers()?, crate::import::import_all());
    db.save_mcp_servers(&current.values().cloned().collect::<Vec<_>>())
}

fn merge_startup_imports(
    mut current: IndexMap<String, McpServer>,
    imported: IndexMap<String, McpServer>,
) -> IndexMap<String, McpServer> {
    for (id, server) in imported {
        if let Some(existing) = current.get_mut(&id) {
            if existing.server != server.server {
                // Keep managed configuration; conflicting files need explicit import.
                continue;
            }
            for app in AppType::all() {
                if !app.is_trae() && server.apps.is_enabled_for(&app) {
                    existing.apps.set_enabled_for(&app, true);
                }
            }
        } else {
            current.insert(id, server);
        }
    }
    current
}

pub(crate) fn import_json_servers(
    config_path: &Path,
    app: &AppType,
) -> Result<Vec<McpServer>, AppError> {
    let content = fs::read_to_string(config_path)?;
    parse_json_servers(&content, app)
}

pub(crate) fn parse_json_servers(content: &str, app: &AppType) -> Result<Vec<McpServer>, AppError> {
    let config = crate::import::parse_json_config(content)?;
    let key = if matches!(app, AppType::OpenCode | AppType::MimoCode)
        || (matches!(app, AppType::Kilo) && config.get("mcpServers").is_none())
        || (app.is_trae() && config.get("mcpServers").is_none() && config.get("mcp").is_some())
    {
        "mcp"
    } else {
        "mcpServers"
    };

    if app.is_trae()
        && (!config.is_object() || config.get(key).is_some_and(|value| !value.is_object()))
    {
        return Err(AppError::Parse(
            "TRAE 配置必须是 JSON 对象，mcpServers 必须是对象".into(),
        ));
    }

    let Some(servers) = config.get(key).and_then(|value| value.as_object()) else {
        return Ok(vec![]);
    };

    servers
        .iter()
        .map(|(id, value)| {
            let mut spec = if matches!(app, AppType::OpenCode | AppType::MimoCode)
                || (matches!(app, AppType::Kilo) && key == "mcp")
            {
                parse_command_array_server(value)?
            } else {
                let mut entry = value.clone();
                if matches!(app, AppType::KimiCode) {
                    if let Some(object) = entry.as_object_mut() {
                        if let Some(transport) = object.remove("transport") {
                            object.insert("type".into(), transport);
                        }
                    }
                }
                serde_json::from_value::<McpServerSpec>(entry)
                    .map_err(|e| AppError::Parse(e.to_string()))?
            };
            if matches!(app, AppType::KimiCode) && spec.spec_type.is_none() {
                spec.spec_type = if spec.command.is_some() {
                    Some("stdio".into())
                } else {
                    spec.url.as_ref().map(|_| "http".into())
                };
            }
            Ok(imported_server(id, spec))
        })
        .collect()
}

fn import_zcode_servers(config_path: &Path) -> Result<Vec<McpServer>, AppError> {
    let content = fs::read_to_string(config_path)?;
    let config: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| AppError::Parse(e.to_string()))?;
    let Some(servers) = config
        .get("mcp")
        .and_then(|value| value.get("servers"))
        .and_then(|value| value.as_object())
    else {
        return Ok(vec![]);
    };

    servers
        .iter()
        .map(|(id, value)| {
            let spec = serde_json::from_value::<McpServerSpec>(value.clone())
                .map_err(|e| AppError::Parse(e.to_string()))?;
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

fn import_hermes_servers(config_path: &Path) -> Result<Vec<McpServer>, AppError> {
    let content = fs::read_to_string(config_path)?;
    let config: serde_yaml::Value =
        serde_yaml::from_str(&content).map_err(|e| AppError::Parse(e.to_string()))?;
    let Some(servers) = config
        .get("mcp_servers")
        .and_then(|value| value.as_mapping())
    else {
        return Ok(vec![]);
    };

    servers
        .iter()
        .filter_map(|(id, value)| id.as_str().map(|id| (id, value)))
        .map(|(id, value)| {
            let json_value =
                serde_json::to_value(value).map_err(|e| AppError::Serialization(e.to_string()))?;
            let spec = serde_json::from_value::<McpServerSpec>(json_value)
                .map_err(|e| AppError::Parse(e.to_string()))?;
            Ok(imported_server(id, spec))
        })
        .collect()
}

fn parse_command_array_server(value: &serde_json::Value) -> Result<McpServerSpec, AppError> {
    let mut spec = McpServerSpec::default();
    let Some(object) = value.as_object() else {
        return Err(AppError::Parse("MCP entry must be an object".to_string()));
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
        .or_else(|| object.get("env"))
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
        .map(|kind| {
            match kind {
                "local" => "stdio",
                "remote" => "http",
                kind => kind,
            }
            .to_string()
        });

    for (key, value) in object {
        if !matches!(
            key.as_str(),
            "type" | "command" | "environment" | "env" | "url" | "headers"
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
    fn merged_import_preserves_metadata_and_existing_switches() {
        let mut existing = imported_server(
            "demo",
            McpServerSpec {
                command: Some("npx".into()),
                ..Default::default()
            },
        );
        existing.name = "My server".into();
        existing.apps.codex = true;
        let current = IndexMap::from([("demo".into(), existing.clone())]);
        let merged = merge_imported_servers(
            current,
            vec![
                (AppType::TraeSoloCn, existing.clone()),
                (AppType::Claude, existing),
            ],
        )
        .unwrap();
        assert_eq!(merged["demo"].name, "My server");
        assert!(
            merged["demo"].apps.codex && merged["demo"].apps.claude && merged["demo"].apps.trae_cn
        );
        assert!(!merged["demo"].apps.trae_solo_cn);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn merged_import_rejects_different_configuration_without_overwriting() {
        let current = IndexMap::from([(
            "demo".into(),
            imported_server(
                "demo",
                McpServerSpec {
                    command: Some("original".into()),
                    ..Default::default()
                },
            ),
        )]);
        let different = imported_server(
            "demo",
            McpServerSpec {
                command: Some("different".into()),
                ..Default::default()
            },
        );
        assert!(merge_imported_servers(current.clone(), vec![(AppType::Trae, different)]).is_err());
        assert_eq!(current["demo"].server.command.as_deref(), Some("original"));
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
    fn imports_legacy_trae_mcp_key() {
        let servers = parse_json_servers(
            r#"{"mcp":{"legacy":{"command":"uvx","args":["demo"]}}}"#,
            &AppType::TraeSoloCn,
        )
        .unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].server.command.as_deref(), Some("uvx"));
        assert_eq!(servers[0].id, "legacy");
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
    fn imports_mimo_command_array() {
        let file = temp_file(
            "mcp-mimo",
            r#"{
              "mcp": {
                "demo": {
                  "type": "local",
                  "command": ["npx", "-y", "demo-server"],
                  "environment": { "TOKEN": "secret" }
                }
              }
            }"#,
        );

        let servers = import_json_servers(file.path(), &AppType::MimoCode).expect("import json");

        assert_eq!(servers.len(), 1);
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

    #[test]
    fn imports_hermes_yaml_mcp_servers() {
        let file = temp_file(
            "mcp-hermes",
            r#"
model: openrouter/demo
mcp_servers:
  demo:
    command: npx
    args:
      - -y
      - demo-server
    env:
      TOKEN: secret
"#,
        );

        let servers = import_hermes_servers(file.path()).expect("import yaml");

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
}

#[cfg(test)]
mod startup_tests {
    use super::*;

    #[test]
    fn startup_preserves_disabled_managed_trae_switches_and_allows_new_ids() {
        let mut managed = imported_server(
            "managed",
            McpServerSpec {
                command: Some("npx".into()),
                ..Default::default()
            },
        );
        managed.name = "My managed server".into();
        managed.apps.set_enabled_for(&AppType::Codex, true);
        let mut from_file = managed.clone();
        from_file.apps.set_enabled_for(&AppType::Trae, true);
        from_file.apps.set_enabled_for(&AppType::TraeCn, true);
        from_file.apps.set_enabled_for(&AppType::Gemini, true);
        from_file.name = "file name".into();
        let mut new_server = imported_server("new", McpServerSpec::default());
        new_server.apps.set_enabled_for(&AppType::TraeCn, true);
        let result = merge_startup_imports(
            IndexMap::from([(managed.id.clone(), managed)]),
            IndexMap::from([
                (from_file.id.clone(), from_file),
                (new_server.id.clone(), new_server),
            ]),
        );
        let saved = &result["managed"];
        assert!(!saved.apps.is_enabled_for(&AppType::Trae));
        assert!(!saved.apps.is_enabled_for(&AppType::TraeCn));
        assert!(saved.apps.is_enabled_for(&AppType::Codex));
        assert!(saved.apps.is_enabled_for(&AppType::Gemini));
        assert_eq!(saved.name, "My managed server");
        assert!(result["new"].apps.is_enabled_for(&AppType::TraeCn));
    }

    #[test]
    fn startup_conflicts_do_not_replace_managed_specs_or_enable_apps() {
        let managed = imported_server(
            "same",
            McpServerSpec {
                command: Some("managed-command".into()),
                ..Default::default()
            },
        );
        let mut from_file = imported_server(
            "same",
            McpServerSpec {
                command: Some("file-command".into()),
                ..Default::default()
            },
        );
        from_file.apps.set_enabled_for(&AppType::TraeCn, true);
        from_file.apps.set_enabled_for(&AppType::Claude, true);
        let result = merge_startup_imports(
            IndexMap::from([(managed.id.clone(), managed)]),
            IndexMap::from([(from_file.id.clone(), from_file)]),
        );
        assert_eq!(
            result["same"].server.command.as_deref(),
            Some("managed-command")
        );
        assert!(!result["same"].apps.is_enabled_for(&AppType::TraeCn));
        assert!(!result["same"].apps.is_enabled_for(&AppType::Claude));
    }
}
