use indexmap::IndexMap;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::agents::{normalized_path_key, resolve_path};
use crate::database::McpServer;
use crate::error::AppError;
use crate::mcp::AppType;

/// 同步指定应用的 MCP 配置到其配置文件
pub fn sync_app_config(app: &AppType, servers: &[McpServer]) -> Result<(), AppError> {
    let app = &app.canonical();
    let config_path = resolve_path(&get_config_path_for_app(app)?);

    if matches!(app, AppType::DeepSeekHarness) {
        return crate::services::deepseek_harness::sync_config(&config_path, servers);
    }
    if matches!(app, AppType::Codex) {
        return sync_codex_config(&config_path, servers);
    }
    if matches!(app, AppType::Continue | AppType::Goose) {
        return sync_agent_yaml_config(&config_path, app, servers);
    }
    if matches!(app, AppType::Hermes) {
        return sync_hermes_config(&config_path, servers);
    }
    if matches!(app, AppType::Zcode) {
        return sync_zcode_config(&config_path, servers);
    }

    sync_json_config(&config_path, app, servers)
}

pub(crate) fn sync_json_config(
    config_path: &PathBuf,
    app: &AppType,
    servers: &[McpServer],
) -> Result<(), AppError> {
    // 读取现有配置（保留非 MCP 字段）
    let mut config: serde_json::Value = if Path::new(&config_path).exists() {
        let content = fs::read_to_string(&config_path).map_err(|e| {
            AppError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;
        let value = crate::import::parse_json_config(&content).map_err(|error| {
            AppError::Parse(format!(
                "{} 配置无法解析，未覆盖原文件：{error}",
                crate::agents::get_agent_name(app)
            ))
        })?;
        if !value.is_object() {
            return Err(AppError::Parse(format!(
                "{} 配置必须是 JSON 对象，未覆盖原文件",
                crate::agents::get_agent_name(app)
            )));
        }
        value
    } else {
        // 如果文件不存在，创建目录
        if let Some(parent) = Path::new(&config_path).parent() {
            fs::create_dir_all(parent).map_err(|e| {
                AppError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })?;
        }
        serde_json::json!({})
    };

    // 构建 MCP 服务器对象
    let legacy_kilo = matches!(app, AppType::Kilo)
        && config_path.file_name().and_then(|name| name.to_str()) == Some("mcp_settings.json");
    let mut mcp_servers = match app {
        AppType::Kilo if legacy_kilo => build_mcp_json(servers),
        AppType::OpenCode | AppType::Kilo => build_opencode_mcp_json(servers),
        AppType::MimoCode => build_mimo_mcp_json(servers),
        _ => build_mcp_json(servers),
    };
    if matches!(app, AppType::KimiCode) {
        for entry in mcp_servers
            .values_mut()
            .filter_map(serde_json::Value::as_object_mut)
        {
            let transport = entry.remove("type").or(entry.remove("transport"));
            if transport.as_ref().and_then(serde_json::Value::as_str) == Some("sse") {
                entry.insert("transport".into(), "sse".into());
            }
        }
    }

    // 根据应用类型确定键名
    let key = match app {
        AppType::Kilo if legacy_kilo => "mcpServers",
        AppType::OpenCode | AppType::Kilo | AppType::MimoCode => "mcp",
        _ => "mcpServers",
    };

    // 更新配置
    if let Some(obj) = config.as_object_mut() {
        obj.insert(key.to_string(), serde_json::Value::Object(mcp_servers));
    }

    // 原子写入
    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| AppError::Serialization(e.to_string()))?;

    atomic_write(&config_path, &content)?;

    Ok(())
}

fn sync_agent_yaml_config(
    path: &PathBuf,
    app: &AppType,
    servers: &[McpServer],
) -> Result<(), AppError> {
    let mut config: serde_json::Value = if path.is_file() {
        let content = fs::read_to_string(path)?;
        let yaml: serde_yaml::Value = serde_yaml::from_str(&content).map_err(|error| {
            AppError::Parse(format!("YAML 配置无法解析，未覆盖原文件：{error}"))
        })?;
        serde_json::to_value(yaml).map_err(|error| AppError::Parse(error.to_string()))?
    } else {
        serde_json::json!({})
    };
    let root = config
        .as_object_mut()
        .ok_or_else(|| AppError::Parse("YAML 配置必须是对象，未覆盖原文件".into()))?;
    if matches!(app, AppType::Continue) {
        let mut entries: Vec<_> = root
            .get("mcpServers")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
            .filter(|entry| entry.get("uses").is_some())
            .cloned()
            .collect();
        for server in servers {
            let mut entry = build_mcp_entry_json(server);
            entry.insert("name".into(), server.id.clone().into());
            if server.server.url.is_some() {
                entry.insert(
                    "type".into(),
                    if server.server.spec_type.as_deref() == Some("sse") {
                        "sse"
                    } else {
                        "streamable-http"
                    }
                    .into(),
                );
                if let Some(headers) = entry.remove("headers") {
                    let options = entry
                        .entry("requestOptions")
                        .or_insert_with(|| serde_json::json!({}));
                    if let Some(options) = options.as_object_mut() {
                        options.insert("headers".into(), headers);
                    }
                }
            }
            entries.push(serde_json::Value::Object(entry));
        }
        root.entry("name").or_insert_with(|| "Local Agent".into());
        root.entry("version").or_insert_with(|| "1.0.0".into());
        root.entry("schema").or_insert_with(|| "v1".into());
        root.insert("mcpServers".into(), entries.into());
    } else {
        let mut extensions: serde_json::Map<String, serde_json::Value> = root
            .get("extensions")
            .and_then(|value| value.as_object())
            .into_iter()
            .flatten()
            .filter(|(_, entry)| {
                !matches!(
                    entry.get("type").and_then(|value| value.as_str()),
                    Some("stdio" | "sse" | "streamable_http")
                )
            })
            .map(|(id, entry)| (id.clone(), entry.clone()))
            .collect();
        for server in servers {
            if extensions.contains_key(&server.id) {
                return Err(AppError::Parse(format!(
                    "Goose 内置扩展 {} 与 MCP 同名，未覆盖原文件",
                    server.id
                )));
            }
            let mut entry = build_mcp_entry_json(server);
            for (source, target) in [("command", "cmd"), ("env", "envs"), ("url", "uri")] {
                if let Some(value) = entry.remove(source) {
                    entry.insert(target.into(), value);
                }
            }
            entry.insert("name".into(), server.name.clone().into());
            entry.insert("enabled".into(), true.into());
            let transport = if server.server.url.is_some() {
                if server.server.spec_type.as_deref() == Some("sse") {
                    "sse"
                } else {
                    "streamable_http"
                }
            } else {
                "stdio"
            };
            entry.insert("type".into(), transport.into());
            extensions.insert(server.id.clone(), serde_json::Value::Object(entry));
        }
        root.insert("extensions".into(), extensions.into());
    }
    let content = serde_yaml::to_string(&config)
        .map_err(|error| AppError::Serialization(error.to_string()))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    atomic_write(path, &content)
}

fn sync_codex_config(path: &PathBuf, servers: &[McpServer]) -> Result<(), AppError> {
    // 读取 TOML
    let mut config: toml::Value = if Path::new(path).exists() {
        let content = fs::read_to_string(path).map_err(|e| {
            AppError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;
        toml::from_str(&content).unwrap_or(toml::Value::Table(toml::map::Map::new()))
    } else {
        // 创建目录
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent).map_err(|e| {
                AppError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })?;
        }
        toml::Value::Table(toml::map::Map::new())
    };

    // 构建 mcp_servers 表
    let mut mcp_table = toml::Table::new();
    for server in servers {
        let mut server_entry = toml::Table::new();
        if let Some(cmd) = &server.server.command {
            server_entry.insert("command".to_string(), toml::Value::String(cmd.clone()));
        }
        if let Some(args) = &server.server.args {
            let arr: Vec<toml::Value> = args
                .iter()
                .map(|a| toml::Value::String(a.clone()))
                .collect();
            server_entry.insert("args".to_string(), toml::Value::Array(arr));
        }
        if let Some(env) = &server.server.env {
            let mut env_table = toml::Table::new();
            for (k, v) in env {
                env_table.insert(k.clone(), toml::Value::String(v.clone()));
            }
            server_entry.insert("env".to_string(), toml::Value::Table(env_table));
        }
        if let Some(url) = &server.server.url {
            server_entry.insert("url".into(), toml::Value::String(url.clone()));
        }
        if let Some(headers) = &server.server.headers {
            server_entry.insert(
                "http_headers".into(),
                toml::Value::Table(
                    headers
                        .iter()
                        .map(|(key, value)| (key.clone(), toml::Value::String(value.clone())))
                        .collect(),
                ),
            );
        }
        if let Some(cwd) = &server.server.cwd {
            server_entry.insert("cwd".into(), toml::Value::String(cwd.clone()));
        }
        for (key, value) in &server.server.extra {
            if matches!(
                key.as_str(),
                "enabled"
                    | "disabled"
                    | "type"
                    | "command"
                    | "args"
                    | "env"
                    | "url"
                    | "http_headers"
                    | "cwd"
            ) {
                continue;
            }
            if let Ok(value) = toml::Value::try_from(value) {
                server_entry.insert(key.clone(), value);
            }
        }
        mcp_table.insert(server.id.clone(), toml::Value::Table(server_entry));
    }

    if let toml::Value::Table(root) = &mut config {
        root.insert("mcp_servers".to_string(), toml::Value::Table(mcp_table));
    }

    // 写入 TOML
    let content =
        toml::to_string_pretty(&config).map_err(|e| AppError::Serialization(e.to_string()))?;

    atomic_write(path, &content)?;
    Ok(())
}

fn sync_hermes_config(path: &PathBuf, servers: &[McpServer]) -> Result<(), AppError> {
    let mut config: serde_yaml::Value = if Path::new(path).exists() {
        let content = fs::read_to_string(path).map_err(|e| {
            AppError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;
        serde_yaml::from_str(&content).unwrap_or(serde_yaml::Value::Mapping(Default::default()))
    } else {
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent).map_err(|e| {
                AppError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })?;
        }
        serde_yaml::Value::Mapping(Default::default())
    };

    if !config.is_mapping() {
        config = serde_yaml::Value::Mapping(Default::default());
    }

    if let serde_yaml::Value::Mapping(root) = &mut config {
        root.insert(
            serde_yaml::Value::String("mcp_servers".to_string()),
            build_hermes_mcp_yaml(servers)?,
        );
    }

    let content =
        serde_yaml::to_string(&config).map_err(|e| AppError::Serialization(e.to_string()))?;
    atomic_write(path, &content)?;
    Ok(())
}

fn build_hermes_mcp_yaml(servers: &[McpServer]) -> Result<serde_yaml::Value, AppError> {
    let mut mcp_servers = serde_yaml::Mapping::new();

    for server in servers {
        let json_entry = serde_json::Value::Object(build_mcp_entry_json(server));
        let yaml_entry =
            serde_yaml::to_value(json_entry).map_err(|e| AppError::Serialization(e.to_string()))?;
        mcp_servers.insert(serde_yaml::Value::String(server.id.clone()), yaml_entry);
    }

    Ok(serde_yaml::Value::Mapping(mcp_servers))
}

fn sync_zcode_config(path: &PathBuf, servers: &[McpServer]) -> Result<(), AppError> {
    // 读取现有配置（保留 provider、plugins、hooks 等非 MCP 字段）
    let mut config: serde_json::Value = if Path::new(path).exists() {
        let content = fs::read_to_string(path).map_err(|e| {
            AppError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent).map_err(|e| {
                AppError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })?;
        }
        serde_json::json!({})
    };

    // ZCode 使用嵌套的 mcp.servers 键
    if let Some(obj) = config.as_object_mut() {
        let mcp = obj
            .entry("mcp".to_string())
            .or_insert_with(|| serde_json::json!({}));
        if let Some(mcp_obj) = mcp.as_object_mut() {
            mcp_obj.insert(
                "servers".to_string(),
                serde_json::Value::Object(build_zcode_mcp_json(servers)),
            );
        }
    }

    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| AppError::Serialization(e.to_string()))?;

    atomic_write(path, &content)?;
    Ok(())
}

fn build_zcode_mcp_json(servers: &[McpServer]) -> serde_json::Map<String, serde_json::Value> {
    let mut entries = serde_json::Map::new();

    for server in servers {
        let mut entry = build_mcp_entry_json(server);
        // ZCode 条目要求显式的 type；缺省时按形态推断（url → http，否则 stdio）
        entry.entry("type".to_string()).or_insert_with(|| {
            if server.server.url.is_some() {
                serde_json::Value::String("http".to_string())
            } else {
                serde_json::Value::String("stdio".to_string())
            }
        });
        entries.insert(server.id.clone(), serde_json::Value::Object(entry));
    }

    entries
}

fn build_mcp_entry_json(server: &McpServer) -> serde_json::Map<String, serde_json::Value> {
    let mut entry = serde_json::Map::new();

    if let Some(spec_type) = &server.server.spec_type {
        entry.insert(
            "type".to_string(),
            serde_json::Value::String(spec_type.clone()),
        );
    }

    if let Some(cmd) = &server.server.command {
        entry.insert(
            "command".to_string(),
            serde_json::Value::String(cmd.clone()),
        );
    }
    if let Some(args) = &server.server.args {
        entry.insert(
            "args".to_string(),
            serde_json::Value::Array(
                args.iter()
                    .map(|a| serde_json::Value::String(a.clone()))
                    .collect(),
            ),
        );
    }
    if let Some(env) = &server.server.env {
        let env_map: serde_json::Map<String, serde_json::Value> = env
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();
        entry.insert("env".to_string(), serde_json::Value::Object(env_map));
    }
    if let Some(cwd) = &server.server.cwd {
        entry.insert("cwd".to_string(), serde_json::Value::String(cwd.clone()));
    }
    if let Some(url) = &server.server.url {
        entry.insert("url".to_string(), serde_json::Value::String(url.clone()));
    }
    if let Some(headers) = &server.server.headers {
        let headers_map: serde_json::Map<String, serde_json::Value> = headers
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();
        entry.insert(
            "headers".to_string(),
            serde_json::Value::Object(headers_map),
        );
    }
    for (k, v) in &server.server.extra {
        entry.insert(k.clone(), v.clone());
    }

    entry
}

fn build_mcp_json(servers: &[McpServer]) -> serde_json::Map<String, serde_json::Value> {
    let mut mcp_servers = serde_json::Map::new();
    for server in servers {
        mcp_servers.insert(
            server.id.clone(),
            serde_json::Value::Object(build_mcp_entry_json(server)),
        );
    }
    mcp_servers
}

/// 为 OpenCode 构建 MCP 服务器配置（符合 opencode schema）
/// OpenCode 要求: type 必填, command 是 string[] (命令+参数合并), 环境变量用 environment
fn build_opencode_mcp_json(servers: &[McpServer]) -> serde_json::Map<String, serde_json::Value> {
    let mut mcp_servers = serde_json::Map::new();
    for server in servers {
        let mut entry = serde_json::Map::new();

        // 判断连接类型：有 url 则为 remote，否则为 local
        let is_remote = server.server.url.is_some()
            || matches!(
                server.server.spec_type.as_deref(),
                Some("remote" | "http" | "sse")
            );

        if is_remote {
            entry.insert(
                "type".to_string(),
                serde_json::Value::String("remote".to_string()),
            );
            if let Some(url) = &server.server.url {
                entry.insert("url".to_string(), serde_json::Value::String(url.clone()));
            }
            if let Some(headers) = &server.server.headers {
                let headers_map: serde_json::Map<String, serde_json::Value> = headers
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect();
                entry.insert(
                    "headers".to_string(),
                    serde_json::Value::Object(headers_map),
                );
            }
        } else {
            entry.insert(
                "type".to_string(),
                serde_json::Value::String("local".to_string()),
            );
            // OpenCode 的 command 是 string[]，合并 command + args
            let mut command_vec: Vec<serde_json::Value> = Vec::new();
            if let Some(cmd) = &server.server.command {
                command_vec.push(serde_json::Value::String(cmd.clone()));
            }
            if let Some(args) = &server.server.args {
                for arg in args {
                    command_vec.push(serde_json::Value::String(arg.clone()));
                }
            }
            if !command_vec.is_empty() {
                entry.insert("command".to_string(), serde_json::Value::Array(command_vec));
            }
            // OpenCode 用 environment 而非 env
            if let Some(env) = &server.server.env {
                if !env.is_empty() {
                    let env_map: serde_json::Map<String, serde_json::Value> = env
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect();
                    entry.insert(
                        "environment".to_string(),
                        serde_json::Value::Object(env_map),
                    );
                }
            }
        }

        mcp_servers.insert(server.id.clone(), serde_json::Value::Object(entry));
    }
    mcp_servers
}

/// 为 Mimo Code 构建 MCP 服务器配置。
/// Mimo Code 使用顶层 `mcp`，本地 command 为 string[]，环境变量字段为 environment。
fn build_mimo_mcp_json(servers: &[McpServer]) -> serde_json::Map<String, serde_json::Value> {
    let mut mcp_servers = serde_json::Map::new();
    for server in servers {
        let mut entry = serde_json::Map::new();

        let is_remote = server.server.url.is_some()
            || matches!(
                server.server.spec_type.as_deref(),
                Some("remote" | "http" | "sse")
            );

        if is_remote {
            entry.insert(
                "type".to_string(),
                serde_json::Value::String("remote".to_string()),
            );
            if let Some(url) = &server.server.url {
                entry.insert("url".to_string(), serde_json::Value::String(url.clone()));
            }
            if let Some(headers) = &server.server.headers {
                let headers_map: serde_json::Map<String, serde_json::Value> = headers
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect();
                entry.insert(
                    "headers".to_string(),
                    serde_json::Value::Object(headers_map),
                );
            }
        } else {
            entry.insert(
                "type".to_string(),
                serde_json::Value::String("local".to_string()),
            );
            let mut command_vec: Vec<serde_json::Value> = Vec::new();
            if let Some(cmd) = &server.server.command {
                command_vec.push(serde_json::Value::String(cmd.clone()));
            }
            if let Some(args) = &server.server.args {
                for arg in args {
                    command_vec.push(serde_json::Value::String(arg.clone()));
                }
            }
            if !command_vec.is_empty() {
                entry.insert("command".to_string(), serde_json::Value::Array(command_vec));
            }
            if let Some(env) = &server.server.env {
                if !env.is_empty() {
                    let env_map: serde_json::Map<String, serde_json::Value> = env
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect();
                    entry.insert(
                        "environment".to_string(),
                        serde_json::Value::Object(env_map),
                    );
                }
            }
        }

        for (key, value) in &server.server.extra {
            if !matches!(
                key.as_str(),
                "type" | "command" | "environment" | "url" | "headers"
            ) {
                entry.insert(key.clone(), value.clone());
            }
        }

        mcp_servers.insert(server.id.clone(), serde_json::Value::Object(entry));
    }
    mcp_servers
}

/// 同步有效配置（包括已存在但没有启用服务器的配置，以清除残留）。
pub fn sync_all_live_configs(servers: &IndexMap<String, McpServer>) -> Result<(), AppError> {
    sync_detected_configs(servers, &crate::agents::detect_all_agents())
}

fn sync_detected_configs(
    servers: &IndexMap<String, McpServer>,
    agents: &[crate::agents::DetectedAgent],
) -> Result<(), AppError> {
    let mut synced_paths = HashSet::new();
    for agent in agents.iter().filter(|agent| agent.exists) {
        let app = agent.app_type.clone();
        let config_path = resolve_path(&get_config_path_for_app(&app)?);
        if matches!(app, AppType::Trae | AppType::TraeCn)
            && !config_path.exists()
            && !crate::services::local_detection::has_canonical_trae_installation(&app)
        {
            continue;
        }
        if !synced_paths.insert(normalized_path_key(&config_path)) {
            continue;
        }
        let app_servers: Vec<McpServer> = servers
            .values()
            .filter(|s| s.apps.is_enabled_for(&app))
            .cloned()
            .collect();
        if app_servers.is_empty() && !config_path.is_file() {
            continue;
        }
        sync_app_config(&app, &app_servers)?;
    }

    Ok(())
}

pub(crate) fn get_config_path_for_app(app: &AppType) -> Result<String, AppError> {
    Ok(crate::agents::get_canonical_mcp_config_path(app)
        .to_string_lossy()
        .into_owned())
}

pub(crate) fn atomic_write(path: &PathBuf, content: &str) -> Result<(), AppError> {
    let parent = path.parent().ok_or_else(|| {
        AppError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "config path has no parent directory",
        ))
    })?;
    let mut temp_file = tempfile::NamedTempFile::new_in(parent).map_err(|e| {
        AppError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            e.to_string(),
        ))
    })?;
    temp_file.write_all(content.as_bytes()).map_err(|e| {
        AppError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            e.to_string(),
        ))
    })?;
    temp_file.as_file().sync_all().map_err(|e| {
        AppError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            e.to_string(),
        ))
    })?;
    temp_file.persist(path).map_err(|e| {
        AppError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            e.error.to_string(),
        ))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{McpApps, McpServerSpec};
    use std::collections::HashMap;
    use std::io::Write;

    fn temp_file(name: &str, content: &str) -> tempfile::TempPath {
        let mut file = tempfile::Builder::new()
            .prefix(name)
            .tempfile()
            .expect("create temp file");
        file.write_all(content.as_bytes()).expect("write temp file");
        file.into_temp_path()
    }

    fn adapter_fixture_server(id: &str, remote: bool) -> McpServer {
        McpServer {
            id: id.into(),
            name: id.into(),
            server: McpServerSpec {
                spec_type: Some(if remote { "http" } else { "stdio" }.into()),
                command: (!remote).then(|| "node".into()),
                args: (!remote).then(|| vec!["server.js".into()]),
                env: (!remote).then(|| HashMap::from([("EXAMPLE".into(), "value".into())])),
                url: remote.then(|| "https://example.com/mcp".into()),
                headers: remote.then(|| HashMap::from([("X-Example".into(), "value".into())])),
                ..Default::default()
            },
            apps: McpApps::default(),
            description: None,
            homepage: None,
            docs: None,
            tags: vec![],
        }
    }

    #[test]
    fn kimi_and_minimax_roundtrip_transports_and_preserve_native_settings() {
        let directory = tempfile::tempdir().unwrap();
        let mut events = adapter_fixture_server("events", true);
        events.server.spec_type = Some("sse".into());
        let mut local = adapter_fixture_server("local", false);
        local.server.cwd = Some("/fixture/workspace".into());
        local.server.extra.insert("enabled".into(), true.into());
        let servers = vec![local, adapter_fixture_server("remote", true), events];
        for app in [AppType::KimiCode, AppType::MinimaxCode] {
            let path = directory.path().join(format!("{}.json", app.name()));
            fs::write(&path, r#"{"theme":"system","mcpServers":{}}"#).unwrap();
            sync_json_config(&path, &app, &servers).unwrap();
            let config: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            assert_eq!(config["theme"], "system");
            let entries = &config["mcpServers"];
            if app == AppType::KimiCode {
                assert_eq!(entries["events"]["transport"], "sse");
                assert!(entries["events"].get("type").is_none());
                assert!(entries["local"].get("type").is_none());
                assert!(entries["remote"].get("type").is_none());
            } else {
                assert_eq!(entries["events"]["type"], "sse");
                assert_eq!(entries["local"]["type"], "stdio");
                assert_eq!(entries["remote"]["type"], "http");
            }
            let imported = crate::import::import_from_path(app.clone(), &path).unwrap();
            let parsed = crate::services::mcp_service::import_json_servers(&path, &app).unwrap();
            assert_eq!(imported.servers.len(), servers.len());
            assert_eq!(parsed.len(), servers.len());
            for server in &servers {
                assert_eq!(imported.servers[&server.id].server, server.server);
                assert_eq!(
                    parsed.iter().find(|entry| entry.id == server.id).unwrap().server,
                    server.server
                );
            }
        }
    }

    #[test]
    fn continue_and_goose_roundtrip_local_and_remote_servers_without_removing_native_settings() {
        let directory = tempfile::tempdir().unwrap();
        let servers = vec![
            adapter_fixture_server("local", false),
            adapter_fixture_server("remote", true),
        ];
        for (app, original) in [
            (AppType::Continue, "name: Existing\nversion: 1.0.0\nschema: v1\nmodels: []\nmcpServers:\n  - uses: owner/shared-server\n"),
            (AppType::Goose, "GOOSE_MODEL: existing-model\nextensions:\n  developer:\n    type: builtin\n    name: developer\n    enabled: true\n"),
        ] {
            let path = directory.path().join(format!("{}.yaml", app.name()));
            fs::write(&path, original).unwrap();
            sync_agent_yaml_config(&path, &app, &servers).unwrap();
            let content = fs::read_to_string(&path).unwrap();
            let config: serde_yaml::Value = serde_yaml::from_str(&content).unwrap();
            if app == AppType::Continue {
                assert_eq!(config["name"].as_str(), Some("Existing"));
                assert_eq!(config["mcpServers"][0]["uses"].as_str(), Some("owner/shared-server"));
            } else {
                assert_eq!(config["GOOSE_MODEL"].as_str(), Some("existing-model"));
                assert_eq!(config["extensions"]["developer"]["type"].as_str(), Some("builtin"));
            }
            let imported = crate::import::parse_agent_yaml_servers(&content, &app).unwrap();
            assert_eq!(imported.len(), 2);
            assert_eq!(imported["local"].server.command.as_deref(), Some("node"));
            assert_eq!(imported["local"].server.args, servers[0].server.args);
            assert_eq!(imported["local"].server.env, servers[0].server.env);
            assert_eq!(imported["remote"].server.spec_type.as_deref(), Some("http"));
            assert_eq!(imported["remote"].server.url, servers[1].server.url);
            assert_eq!(imported["remote"].server.headers, servers[1].server.headers);
        }
    }

    #[test]
    fn native_and_legacy_kilo_jsonc_preserve_settings_and_use_the_correct_mcp_schema() {
        let directory = tempfile::tempdir().unwrap();
        let servers = vec![adapter_fixture_server("local", false)];
        for (filename, key) in [("kilo.jsonc", "mcp"), ("mcp_settings.json", "mcpServers")] {
            let path = directory.path().join(filename);
            fs::write(
                &path,
                "{ // personal config\n \"model\": \"existing-model\",\n}",
            )
            .unwrap();
            sync_json_config(&path, &AppType::Kilo, &servers).unwrap();
            let content = fs::read_to_string(&path).unwrap();
            let config = crate::import::parse_json_config(&content).unwrap();
            assert_eq!(config["model"], "existing-model");
            assert!(config[key]["local"].is_object());
            let imported = crate::import::import_from_path(AppType::Kilo, &path).unwrap();
            assert_eq!(
                imported.servers["local"].server.args,
                servers[0].server.args
            );
        }
    }

    #[test]
    fn invalid_adapter_configurations_are_preserved_and_goose_builtin_collisions_fail() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.yaml");
        let original = "extensions: [broken";
        fs::write(&path, original).unwrap();
        assert!(sync_agent_yaml_config(&path, &AppType::Goose, &[]).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        let original = "extensions:\n  developer:\n    type: builtin\n";
        fs::write(&path, original).unwrap();
        assert!(sync_agent_yaml_config(
            &path,
            &AppType::Goose,
            &[adapter_fixture_server("developer", false)]
        )
        .is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
    }

    #[test]
    fn codex_shared_config_preserves_remote_servers_headers_and_non_mcp_settings() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.toml");
        fs::write(&path, "model = \"existing-model\"\n").unwrap();
        let server = adapter_fixture_server("remote", true);
        sync_codex_config(&path, &[server.clone()]).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        let config: toml::Value = toml::from_str(&content).unwrap();
        assert_eq!(config["model"].as_str(), Some("existing-model"));
        let imported = crate::import::import_from_path(AppType::Codex, &path).unwrap();
        assert_eq!(imported.servers["remote"].server.url, server.server.url);
        assert_eq!(
            imported.servers["remote"].server.headers,
            server.server.headers
        );
    }

    #[test]
    fn syncs_hermes_yaml_mcp_servers() {
        let file = temp_file(
            "mcp-hermes",
            r#"
model: openrouter/demo
terminal:
  enabled: true
mcp_servers:
  old:
    command: old
"#,
        );
        let path = file.to_path_buf();
        let server = McpServer {
            id: "demo".to_string(),
            name: "demo".to_string(),
            server: McpServerSpec {
                command: Some("npx".to_string()),
                args: Some(vec!["-y".to_string(), "demo-server".to_string()]),
                env: Some(HashMap::from([("TOKEN".to_string(), "secret".to_string())])),
                ..Default::default()
            },
            apps: McpApps::default(),
            description: None,
            homepage: None,
            docs: None,
            tags: vec![],
        };

        sync_hermes_config(&path, &[server]).expect("sync hermes yaml");

        let content = std::fs::read_to_string(&path).expect("read yaml");
        let yaml: serde_yaml::Value = serde_yaml::from_str(&content).expect("parse yaml");
        assert_eq!(
            yaml.get("model").and_then(|value| value.as_str()),
            Some("openrouter/demo")
        );
        assert!(yaml.get("terminal").is_some());
        let servers = yaml
            .get("mcp_servers")
            .and_then(|value| value.as_mapping())
            .expect("mcp servers");
        assert_eq!(servers.len(), 1);
        let demo = servers
            .get(serde_yaml::Value::String("demo".to_string()))
            .expect("demo server");
        assert_eq!(
            demo.get("command").and_then(|value| value.as_str()),
            Some("npx")
        );
        assert_eq!(
            demo.get("env")
                .and_then(|value| value.get("TOKEN"))
                .and_then(|value| value.as_str()),
            Some("secret")
        );
    }

    #[test]
    fn syncs_zcode_nested_mcp_servers_and_preserves_other_fields() {
        let file = temp_file(
            "mcp-zcode",
            r#"{"provider":{"default":"glm"},"plugins":{"enabledPlugins":{}},"mcp":{"servers":{"old":{"type":"stdio","command":"old"}}}}"#,
        );
        let path = file.to_path_buf();
        let server = McpServer {
            id: "demo".to_string(),
            name: "demo".to_string(),
            server: McpServerSpec {
                command: Some("npx".to_string()),
                args: Some(vec!["-y".to_string(), "demo-server".to_string()]),
                env: Some(HashMap::from([("TOKEN".to_string(), "secret".to_string())])),
                ..Default::default()
            },
            apps: McpApps::default(),
            description: None,
            homepage: None,
            docs: None,
            tags: vec![],
        };

        sync_zcode_config(&path, &[server]).expect("sync zcode config");

        let content = std::fs::read_to_string(&path).expect("read config");
        let json: serde_json::Value = serde_json::from_str(&content).expect("parse config");
        assert_eq!(
            json.get("provider")
                .and_then(|value| value.get("default"))
                .and_then(|value| value.as_str()),
            Some("glm")
        );
        assert!(json.get("plugins").is_some());
        let servers = json
            .get("mcp")
            .and_then(|value| value.get("servers"))
            .and_then(|value| value.as_object())
            .expect("nested servers map");
        assert_eq!(servers.len(), 1);
        let demo = servers.get("demo").expect("demo server");
        assert_eq!(
            demo.get("type").and_then(|value| value.as_str()),
            Some("stdio")
        );
        assert_eq!(
            demo.get("command").and_then(|value| value.as_str()),
            Some("npx")
        );
        assert_eq!(
            demo.get("env")
                .and_then(|value| value.get("TOKEN"))
                .and_then(|value| value.as_str()),
            Some("secret")
        );
    }

    #[test]
    fn syncs_mimo_mcp_servers() {
        let server = McpServer {
            id: "demo".to_string(),
            name: "demo".to_string(),
            server: McpServerSpec {
                command: Some("npx".to_string()),
                args: Some(vec!["-y".to_string(), "demo-server".to_string()]),
                env: Some(HashMap::from([("TOKEN".to_string(), "secret".to_string())])),
                ..Default::default()
            },
            apps: McpApps::default(),
            description: None,
            homepage: None,
            docs: None,
            tags: vec![],
        };

        let json = build_mimo_mcp_json(&[server]);
        let demo = json.get("demo").expect("demo server");
        assert_eq!(
            demo.get("type").and_then(|value| value.as_str()),
            Some("local")
        );
        assert_eq!(
            demo.get("command")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str())
                        .collect::<Vec<_>>()
                }),
            Some(vec!["npx", "-y", "demo-server"])
        );
        assert_eq!(
            demo.get("environment")
                .and_then(|value| value.get("TOKEN"))
                .and_then(|value| value.as_str()),
            Some("secret")
        );
    }

    #[test]
    fn syncs_workbuddy_standard_mcp_and_preserves_other_top_level_fields() {
        let file = temp_file(
            "mcp-workbuddy",
            r#"{"theme":"system","mcpServers":{"old":{"command":"old"}}}"#,
        );
        let path = file.to_path_buf();
        let server = McpServer {
            id: "remote".to_string(),
            name: "remote".to_string(),
            server: McpServerSpec {
                spec_type: Some("sse".to_string()),
                url: Some("https://example.com/events".to_string()),
                headers: Some(HashMap::from([(
                    "Authorization".to_string(),
                    "Bearer token".to_string(),
                )])),
                ..Default::default()
            },
            apps: McpApps::default(),
            description: None,
            homepage: None,
            docs: None,
            tags: vec![],
        };

        sync_json_config(&path, &AppType::WorkBuddy, &[server]).expect("sync WorkBuddy config");

        let content = std::fs::read_to_string(&path).expect("read WorkBuddy config");
        let json: serde_json::Value = serde_json::from_str(&content).expect("parse config");
        assert_eq!(
            json.get("theme").and_then(|value| value.as_str()),
            Some("system")
        );
        let remote = json
            .get("mcpServers")
            .and_then(|value| value.get("remote"))
            .expect("remote MCP server");
        assert_eq!(
            remote.get("type").and_then(|value| value.as_str()),
            Some("sse")
        );
        assert_eq!(
            remote.get("url").and_then(|value| value.as_str()),
            Some("https://example.com/events")
        );
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn workbuddy_editions_resolve_to_one_sync_target() {
        let international = resolve_path(
            &get_config_path_for_app(&AppType::WorkBuddy).expect("international config path"),
        );
        let china = resolve_path(
            &get_config_path_for_app(&AppType::WorkBuddyCn).expect("China config path"),
        );
        let paths = HashSet::from([
            normalized_path_key(&international),
            normalized_path_key(&china),
        ]);
        assert_eq!(paths.len(), 1);
    }

    #[test]
    fn does_not_create_configuration_for_an_uninstalled_agent_even_when_enabled() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mimocode.json");
        let mut apps = McpApps::default();
        apps.set_enabled_for(&AppType::MimoCode, true);
        let server = McpServer {
            id: "demo".into(),
            name: "Demo".into(),
            server: McpServerSpec {
                command: Some("node".into()),
                ..Default::default()
            },
            apps,
            description: None,
            homepage: None,
            docs: None,
            tags: vec![],
        };
        let servers = IndexMap::from([(server.id.clone(), server)]);
        let agents = vec![crate::agents::DetectedAgent {
            app_type: AppType::MimoCode,
            name: "Mimo Code".into(),
            config_path: path.to_string_lossy().into_owned(),
            exists: false,
            mcp_count: 0,
        }];
        sync_detected_configs(&servers, &agents).unwrap();
        assert!(!path.exists());
    }
}
