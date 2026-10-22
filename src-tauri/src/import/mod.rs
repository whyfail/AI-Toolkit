use indexmap::IndexMap;
use serde::Deserialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use crate::agents::{get_agent_config_paths, normalized_path_key};
use crate::database::{McpApps, McpServer, McpServerSpec};
use crate::mcp::AppType;

/// 导入结果
pub struct ImportResult {
    pub servers: IndexMap<String, McpServer>,
    pub app: AppType,
    pub source_path: String,
}

/// 尝试从指定路径导入 MCP 配置
pub fn import_from_path(app: AppType, path: &PathBuf) -> Option<ImportResult> {
    if !path.exists() {
        return None;
    }

    let app = app.canonical();
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return None,
    };

    let extension = path.extension().and_then(|s| s.to_str());
    let servers = if matches!(app, AppType::DeepSeekHarness) {
        crate::services::deepseek_harness::parse_servers(&content).ok()
    } else if extension == Some("toml") {
        parse_mcp_toml(&content, &app)
    } else if matches!(extension, Some("yaml" | "yml")) {
        parse_mcp_yaml(&content, &app)
    } else {
        parse_mcp_json(&content, &app)
    }?;

    Some(ImportResult {
        servers,
        app,
        source_path: path.to_string_lossy().to_string(),
    })
}

/// 解析 MCP JSON 配置（支持多种格式）
pub(crate) fn parse_json_config(
    content: &str,
) -> Result<serde_json::Value, crate::error::AppError> {
    serde_json::from_str(content)
        .or_else(|_| json5::from_str(content))
        .map_err(|error| crate::error::AppError::Parse(error.to_string()))
}

fn parse_mcp_json(content: &str, app: &AppType) -> Option<IndexMap<String, McpServer>> {
    let json: serde_json::Value = match parse_json_config(content) {
        Ok(v) => v,
        Err(_) => return None,
    };

    let mut servers = IndexMap::new();

    // Claude Code (~/.claude.json): { "mcpServers": { "id": { "command": "...", "args": [] } } }
    // Qwen Code (~/.qwen/settings.json): { "mcpServers": { "id": { ... } } }
    if let Some(mcp_servers) = json.get("mcpServers") {
        if let Some(obj) = mcp_servers.as_object() {
            for (id, config) in obj {
                if let Some(server) = parse_server_config(id, config, app) {
                    servers.insert(id.clone(), server);
                }
            }
        }
    }
    // ZCode (~/.zcode/cli/config.json): { "mcp": { "servers": { "id": { ... } } } }
    else if matches!(app, AppType::Zcode) {
        if let Some(obj) = json
            .get("mcp")
            .and_then(|value| value.get("servers"))
            .and_then(|value| value.as_object())
        {
            for (id, config) in obj {
                if let Some(server) = parse_server_config(id, config, app) {
                    servers.insert(id.clone(), server);
                }
            }
        }
    }
    // Trae / OpenCode / Mimo Code: { "mcp": { "id": { ... } } }
    else if let Some(mcp) = json.get("mcp") {
        if let Some(obj) = mcp.as_object() {
            for (id, config) in obj {
                if let Some(server) = parse_server_config(id, config, app) {
                    servers.insert(id.clone(), server);
                }
            }
        }
    }
    // Gemini (~/.gemini/settings.json): { "mcpServers": [...] } 数组格式
    else if let Some(mcp_servers) = json.get("mcpServers") {
        if let Some(arr) = mcp_servers.as_array() {
            for (i, config) in arr.iter().enumerate() {
                let id_str = config
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .unwrap_or_else(|| format!("gemini-mcp-{}", i));
                if let Some(server) = parse_server_config(&id_str, config, app) {
                    servers.insert(id_str, server);
                }
            }
        }
    }
    // 直接是服务器对象
    else if let Some(obj) = json.as_object() {
        for (id, config) in obj {
            if let Some(server) = parse_server_config(id, config, app) {
                servers.insert(id.clone(), server);
            }
        }
    }

    if servers.is_empty() {
        None
    } else {
        Some(servers)
    }
}

/// 解析 TOML 配置（Codex ~/.codex/config.toml）
fn parse_mcp_toml(content: &str, app: &AppType) -> Option<IndexMap<String, McpServer>> {
    let toml_value: toml::Value = match toml::from_str(content) {
        Ok(v) => v,
        Err(_) => return None,
    };

    let mut servers = IndexMap::new();

    // Codex 格式: [mcp_servers.id]
    // 将 TOML 转为 JSON Value 以便统一处理
    let json_value: serde_json::Value = match serde_json::to_value(&toml_value) {
        Ok(v) => v,
        Err(_) => return None,
    };

    if let Some(mcp_servers) = json_value.get("mcp_servers") {
        if let Some(obj) = mcp_servers.as_object() {
            for (id, config) in obj {
                if let Some(server) = parse_server_config(id, config, app) {
                    servers.insert(id.clone(), server);
                }
            }
        }
    }

    if servers.is_empty() {
        None
    } else {
        Some(servers)
    }
}

/// 解析 YAML 配置（Hermes ~/.hermes/config.yaml）
fn parse_mcp_yaml(content: &str, app: &AppType) -> Option<IndexMap<String, McpServer>> {
    if matches!(app, AppType::Continue | AppType::Goose) {
        return parse_agent_yaml_servers(content, app)
            .ok()
            .filter(|servers| !servers.is_empty());
    }
    let yaml_value: serde_yaml::Value = match serde_yaml::from_str(content) {
        Ok(v) => v,
        Err(_) => return None,
    };

    let json_value: serde_json::Value = match serde_json::to_value(&yaml_value) {
        Ok(v) => v,
        Err(_) => return None,
    };

    let mut servers = IndexMap::new();
    if let Some(mcp_servers) = json_value.get("mcp_servers") {
        if let Some(obj) = mcp_servers.as_object() {
            for (id, config) in obj {
                if let Some(server) = parse_server_config(id, config, app) {
                    servers.insert(id.clone(), server);
                }
            }
        }
    }

    if servers.is_empty() {
        None
    } else {
        Some(servers)
    }
}

pub(crate) fn parse_agent_yaml_servers(
    content: &str,
    app: &AppType,
) -> Result<IndexMap<String, McpServer>, crate::error::AppError> {
    use crate::error::AppError;
    let yaml: serde_yaml::Value =
        serde_yaml::from_str(content).map_err(|error| AppError::Parse(error.to_string()))?;
    let config = serde_json::to_value(yaml).map_err(|error| AppError::Parse(error.to_string()))?;
    if !config.is_object() {
        return Err(AppError::Parse("Agent YAML 配置必须是对象".into()));
    }
    let mut servers = IndexMap::new();
    if matches!(app, AppType::Continue) {
        if let Some(entries) = config.get("mcpServers") {
            let entries = entries
                .as_array()
                .ok_or_else(|| AppError::Parse("Continue mcpServers 必须是数组".into()))?;
            for entry in entries {
                if entry.get("uses").is_some() {
                    continue;
                }
                let Some(name) = entry.get("name").and_then(|value| value.as_str()) else {
                    continue;
                };
                let mut entry = entry.clone();
                if let Some(headers) = entry.pointer("/requestOptions/headers").cloned() {
                    entry["headers"] = headers;
                }
                if let Some(server) = parse_server_config(name, &entry, app) {
                    servers.insert(name.to_string(), server);
                }
            }
        }
    } else if let Some(entries) = config.get("extensions") {
        let entries = entries
            .as_object()
            .ok_or_else(|| AppError::Parse("Goose extensions 必须是对象".into()))?;
        for (id, entry) in entries {
            if !matches!(
                entry.get("type").and_then(|value| value.as_str()),
                Some("stdio" | "sse" | "streamable_http")
            ) {
                continue;
            }
            let mut entry = entry.clone();
            for (source, target) in [("cmd", "command"), ("envs", "env"), ("uri", "url")] {
                if let Some(value) = entry.get(source).cloned() {
                    entry[target] = value;
                }
            }
            if let Some(server) = parse_server_config(id, &entry, app) {
                servers.insert(id.clone(), server);
            }
        }
    }
    Ok(servers)
}

/// 解析单个服务器配置
fn parse_server_config(id: &str, config: &serde_json::Value, app: &AppType) -> Option<McpServer> {
    let config_obj = config.as_object()?;

    // 提取基本字段
    // OpenCode / Mimo Code 格式: command 是 string[] (如 ["npx", "-y", "xxx"])，无 args
    // 其他工具格式: command 是 string，args 是 string[]
    let (command, args) = if let Some(cmd_val) = config_obj.get("command") {
        if let Some(arr) = cmd_val.as_array() {
            // command 是数组: 第一个元素作为 command，其余作为 args
            let mut items: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            let cmd = items.first().map(|s| s.clone());
            let a = if items.len() > 1 {
                Some(items.split_off(1))
            } else {
                None
            };
            (cmd, a)
        } else {
            // command 是字符串 (标准格式)
            let cmd = cmd_val.as_str().map(String::from);
            let a = config_obj
                .get("args")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                });
            (cmd, a)
        }
    } else {
        (None, None)
    };
    // 同时支持 "env" (标准格式) 和 "environment" (OpenCode / Mimo Code 格式)
    let env = config_obj
        .get("env")
        .or_else(|| config_obj.get("environment"))
        .and_then(|v| {
            v.as_object().map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|vs| (k.clone(), vs.to_string())))
                    .collect()
            })
        });
    let url = config_obj
        .get("url")
        .or_else(|| config_obj.get("serverUrl"))
        .or_else(|| config_obj.get("httpUrl"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let headers = config_obj
        .get("headers")
        .or_else(|| config_obj.get("http_headers"))
        .and_then(|v| {
            v.as_object().map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|vs| (k.clone(), vs.to_string())))
                    .collect()
            })
        });

    let cwd = config_obj
        .get("cwd")
        .and_then(|v| v.as_str())
        .map(String::from);

    let spec_type = config_obj
        .get(if matches!(app, AppType::KimiCode) { "transport" } else { "type" })
        .or_else(|| config_obj.get("type"))
        .and_then(|v| v.as_str())
        .map(|transport| {
            match transport {
                "local" => "stdio",
                "remote" | "streamable-http" | "streamable_http" => "http",
                other => other,
            }
            .to_string()
        })
        .or_else(|| {
            if command.is_some() {
                Some("stdio".to_string())
            } else if url.is_some() {
                Some("http".to_string())
            } else {
                None
            }
        });

    let known_keys = [
        "type",
        "command",
        "args",
        "env",
        "environment",
        "cwd",
        "url",
        "headers",
        "name",
        "description",
        "docs",
        "homepage",
        "cmd",
        "envs",
        "uri",
        "serverUrl",
        "httpUrl",
        "http_headers",
    ];
    let extra = config_obj
        .iter()
        .filter(|(key, _)| !known_keys.contains(&key.as_str()))
        .filter(|(key, _)| !matches!(app, AppType::KimiCode) || key.as_str() != "transport")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();

    // 构建服务器规范
    let server = McpServerSpec {
        spec_type,
        command,
        args,
        env,
        cwd,
        url,
        headers,
        extra,
    };

    // 创建应用状态
    let mut apps = McpApps::default();
    apps.set_enabled_for(
        app,
        config_obj.get("enabled").and_then(|value| value.as_bool()) != Some(false)
            && config_obj.get("disabled").and_then(|value| value.as_bool()) != Some(true),
    );

    // 从配置中提取名称和描述
    let name = config_obj
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(id)
        .to_string();

    let description = config_obj
        .get("description")
        .and_then(|v| v.as_str())
        .map(String::from);
    let docs = config_obj
        .get("docs")
        .and_then(|v| v.as_str())
        .map(String::from);
    let homepage = config_obj
        .get("homepage")
        .and_then(|v| v.as_str())
        .map(String::from);

    Some(McpServer {
        id: id.to_string(),
        name,
        server,
        apps,
        description,
        homepage,
        docs,
        tags: vec![format!("imported-from-{}", app.name())],
    })
}

/// 从所有支持的来源导入 MCP 配置
pub fn import_all() -> IndexMap<String, McpServer> {
    let mut all_servers: IndexMap<String, McpServer> = IndexMap::new();
    let mut scanned_paths = HashSet::new();

    // Iterate through all apps and check their OS-specific paths
    for agent in crate::agents::detect_all_agents() {
        let app = agent.app_type.canonical();
        let paths = startup_config_paths(&app);

        // Check each possible path for this app
        for path in &paths {
            if !scanned_paths.insert(normalized_path_key(path)) {
                continue;
            }
            if let Some(result) = import_from_path(app.clone(), path) {
                for (_, server) in result.servers {
                    merge_imported_server(&mut all_servers, &app, server);
                }
                // Break after finding the first valid path for this app
                break;
            }
        }
    }

    all_servers
}

fn startup_config_paths(app: &AppType) -> Vec<PathBuf> {
    if app.is_trae() {
        vec![crate::agents::get_canonical_mcp_config_path(app)]
    } else {
        get_agent_config_paths(app)
    }
}

fn merge_imported_server(
    servers: &mut IndexMap<String, McpServer>,
    app: &AppType,
    server: McpServer,
) {
    if let Some(existing) = servers.get_mut(&server.id) {
        // 同名异配置保留当前规范来源，冲突交给显式迁移预览。
        if existing.server != server.server {
            return;
        }
        existing.apps.set_enabled_for(&app.canonical(), true);
        let tag = format!("imported-from-{}", app.canonical().name());
        if !existing.tags.contains(&tag) {
            existing.tags.push(tag);
        }
    } else {
        servers.insert(server.id.clone(), server);
    }
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct ClaudeMcpConfig {
    #[serde(rename = "mcpServers")]
    mcp_servers: Option<HashMap<String, serde_json::Value>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_workbuddy_http_and_sse_servers_with_shared_alias_state() {
        let servers = parse_mcp_json(
            r#"{
                "mcpServers": {
                    "local": {"type":"stdio","command":"npx","args":["-y","demo"]},
                    "http": {"type":"http","url":"https://example.com/mcp"},
                    "events": {"type":"sse","url":"https://example.com/events"}
                }
            }"#,
            &AppType::WorkBuddyCn,
        )
        .expect("parse WorkBuddy MCP config");

        assert_eq!(servers["local"].server.spec_type.as_deref(), Some("stdio"));
        assert_eq!(servers["local"].server.command.as_deref(), Some("npx"));
        assert_eq!(servers["http"].server.spec_type.as_deref(), Some("http"));
        assert_eq!(servers["events"].server.spec_type.as_deref(), Some("sse"));
        assert!(servers["http"].apps.workbuddy);
        assert!(!servers["http"].apps.workbuddy_cn);
        assert!(servers["http"].apps.is_enabled_for(&AppType::WorkBuddyCn));
    }

    #[test]
    fn workbuddy_editions_resolve_to_one_bulk_import_path() {
        let mut paths = HashSet::new();
        for app in [AppType::WorkBuddy, AppType::WorkBuddyCn] {
            for path in get_agent_config_paths(&app) {
                paths.insert(normalized_path_key(&path));
            }
        }
        assert_eq!(paths.len(), 1);
    }

    #[test]
    fn legacy_trae_file_imports_as_canonical_target() {
        let directory = tempfile::tempdir().expect("temporary import directory");
        let path = directory.path().join("legacy.json");
        fs::write(
            &path,
            r#"{"mcpServers":{"example":{"command":"npx","args":["demo"]}}}"#,
        )
        .expect("write fixture");
        for (legacy, canonical) in [
            (AppType::TraeWork, AppType::Trae),
            (AppType::TraeSoloCn, AppType::TraeCn),
        ] {
            let result = import_from_path(legacy.clone(), &path).expect("import old target");
            assert_eq!(result.app, canonical);
            let apps = &result.servers["example"].apps;
            assert!(apps.is_enabled_for(&legacy));
            assert!(apps.is_enabled_for(&canonical));
            assert!(!apps.trae_work);
            assert!(!apps.trae_solo_cn);
        }
    }

    #[test]
    fn imported_conflicts_preserve_configuration_and_do_not_merge_switches() {
        let current = parse_mcp_json(
            r#"{"mcpServers":{"example":{"command":"current"}}}"#,
            &AppType::Trae,
        )
        .expect("parse current config");
        let mut merged = current.clone();
        let conflict = parse_mcp_json(
            r#"{"mcpServers":{"example":{"command":"different"}}}"#,
            &AppType::TraeCn,
        )
        .expect("parse conflicting config")
        .swap_remove("example")
        .expect("conflicting server");
        merge_imported_server(&mut merged, &AppType::TraeCn, conflict);
        assert_eq!(merged["example"].server, current["example"].server);
        assert!(!merged["example"].apps.trae_cn);

        let same = parse_mcp_json(
            r#"{"mcpServers":{"example":{"command":"current"}}}"#,
            &AppType::TraeSoloCn,
        )
        .expect("parse identical old config")
        .swap_remove("example")
        .expect("same server");
        merge_imported_server(&mut merged, &AppType::TraeSoloCn, same);
        assert!(merged["example"].apps.trae_cn);
        assert!(!merged["example"].apps.trae_solo_cn);
    }
}

#[cfg(test)]
mod startup_path_tests {
    use super::*;

    #[test]
    fn startup_trae_paths_exclude_work_and_solo_legacy_sources() {
        for app in [
            AppType::Trae,
            AppType::TraeCn,
            AppType::TraeWork,
            AppType::TraeSoloCn,
        ] {
            let paths = startup_config_paths(&app);
            assert_eq!(
                paths,
                vec![crate::agents::get_canonical_mcp_config_path(&app)]
            );
            assert_eq!(paths.len(), 1);
            assert!(!paths[0].to_string_lossy().contains("TRAE Work"));
            assert!(!paths[0].to_string_lossy().contains("TRAE SOLO"));
            assert!(get_agent_config_paths(&app).len() > paths.len());
        }
        assert_eq!(
            startup_config_paths(&AppType::Codex),
            get_agent_config_paths(&AppType::Codex)
        );
    }
}
