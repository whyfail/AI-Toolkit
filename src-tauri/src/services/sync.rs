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
    let config_path = resolve_path(&get_config_path_for_app(app)?);

    if matches!(app, AppType::Codex) {
        return sync_codex_config(&config_path, servers);
    }
    if matches!(app, AppType::Hermes) {
        return sync_hermes_config(&config_path, servers);
    }

    sync_json_config(&config_path, app, servers)
}

fn sync_json_config(
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
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
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
    let mcp_servers = match app {
        AppType::OpenCode => build_opencode_mcp_json(servers),
        AppType::MimoCode => build_mimo_mcp_json(servers),
        _ => build_mcp_json(servers),
    };

    // 根据应用类型确定键名
    let key = match app {
        AppType::OpenCode | AppType::MimoCode => "mcp",
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

/// 同步所有应用的 MCP 配置（包括没有任何启用服务器的应用，以清除残留配置）
pub fn sync_all_live_configs(servers: &IndexMap<String, McpServer>) -> Result<(), AppError> {
    let mut synced_paths = HashSet::new();
    for app in AppType::all() {
        let config_path = resolve_path(&get_config_path_for_app(&app)?);
        if !synced_paths.insert(normalized_path_key(&config_path)) {
            continue;
        }
        let app_servers: Vec<McpServer> = servers
            .values()
            .filter(|s| s.apps.is_enabled_for(&app))
            .cloned()
            .collect();
        sync_app_config(&app, &app_servers)?;
    }

    Ok(())
}

pub(crate) fn get_config_path_for_app(app: &AppType) -> Result<String, AppError> {
    Ok(match app {
        AppType::QwenCode => "~/.qwen/settings.json",
        AppType::Claude => {
            if cfg!(windows) {
                "%USERPROFILE%\\.claude.json"
            } else {
                "~/.claude.json"
            }
        }
        AppType::Codex => {
            if cfg!(windows) {
                "%USERPROFILE%\\.codex\\config.toml"
            } else {
                "~/.codex/config.toml"
            }
        }
        AppType::Gemini => {
            if cfg!(windows) {
                "%USERPROFILE%\\.gemini\\settings.json"
            } else {
                "~/.gemini/settings.json"
            }
        }
        AppType::OpenCode => {
            if cfg!(windows) {
                "%USERPROFILE%\\.config\\opencode\\opencode.json"
            } else {
                "~/.config/opencode/opencode.json"
            }
        }
        AppType::Trae => {
            if cfg!(windows) {
                "%APPDATA%\\Trae\\User\\mcp.json"
            } else {
                "~/Library/Application Support/Trae/User/mcp.json"
            }
        }
        AppType::TraeCn => {
            if cfg!(windows) {
                "%APPDATA%\\Trae CN\\User\\mcp.json"
            } else {
                "~/Library/Application Support/Trae CN/User/mcp.json"
            }
        }
        AppType::TraeWork => {
            if cfg!(windows) {
                "%APPDATA%\\TRAE Work\\User\\mcp.json"
            } else {
                "~/Library/Application Support/TRAE Work/User/mcp.json"
            }
        }
        AppType::TraeSoloCn => {
            if cfg!(windows) {
                "%APPDATA%\\TRAE Work CN\\User\\mcp.json"
            } else {
                "~/Library/Application Support/TRAE Work CN/User/mcp.json"
            }
        }
        AppType::Qoder => {
            if cfg!(windows) {
                "%APPDATA%\\Qoder\\SharedClientCache\\mcp.json"
            } else {
                "~/Library/Application Support/Qoder/SharedClientCache/mcp.json"
            }
        }
        AppType::Qodercli => {
            if cfg!(windows) {
                "%USERPROFILE%\\.qodercli\\settings.json"
            } else {
                "~/.qodercli/settings.json"
            }
        }
        AppType::CodeBuddy => {
            if cfg!(windows) {
                "%USERPROFILE%\\.codebuddy\\mcp.json"
            } else {
                "~/.codebuddy/mcp.json"
            }
        }
        AppType::Hermes => {
            if cfg!(windows) {
                "%USERPROFILE%\\.hermes\\config.yaml"
            } else {
                "~/.hermes/config.yaml"
            }
        }
        AppType::MimoCode => {
            if cfg!(windows) {
                "%USERPROFILE%\\.config\\mimocode\\mimocode.json"
            } else {
                "~/.config/mimocode/mimocode.json"
            }
        }
        AppType::WorkBuddy | AppType::WorkBuddyCn => {
            if cfg!(windows) {
                "%USERPROFILE%\\.workbuddy\\.mcp.json"
            } else {
                "~/.workbuddy/.mcp.json"
            }
        }
    }
    .to_string())
}

fn atomic_write(path: &PathBuf, content: &str) -> Result<(), AppError> {
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

    fn temp_file(name: &str, content: &str) -> tempfile::NamedTempFile {
        let mut file = tempfile::Builder::new()
            .prefix(name)
            .tempfile()
            .expect("create temp file");
        file.write_all(content.as_bytes()).expect("write temp file");
        file
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

        sync_hermes_config(&file.path().to_path_buf(), &[server]).expect("sync hermes yaml");

        let content = std::fs::read_to_string(file.path()).expect("read yaml");
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

        sync_json_config(&file.path().to_path_buf(), &AppType::WorkBuddy, &[server])
            .expect("sync WorkBuddy config");

        let content = std::fs::read_to_string(file.path()).expect("read WorkBuddy config");
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
        assert!(!file.path().with_extension("tmp").exists());
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
    fn sync_all_writes_mimo_config_when_enabled() {
        let temp_home = tempfile::tempdir().expect("temp home");
        let old_home = std::env::var_os("HOME");
        let old_userprofile = std::env::var_os("USERPROFILE");
        let old_appdata = std::env::var_os("APPDATA");
        std::env::set_var("HOME", temp_home.path());
        std::env::set_var("USERPROFILE", temp_home.path());
        std::env::set_var("APPDATA", temp_home.path());

        let mut apps = McpApps::default();
        apps.set_enabled_for(&AppType::MimoCode, true);
        let server = McpServer {
            id: "demo".to_string(),
            name: "demo".to_string(),
            server: McpServerSpec {
                command: Some("npx".to_string()),
                args: Some(vec!["-y".to_string(), "demo-server".to_string()]),
                env: Some(HashMap::from([("TOKEN".to_string(), "secret".to_string())])),
                ..Default::default()
            },
            apps,
            description: None,
            homepage: None,
            docs: None,
            tags: vec![],
        };
        let mut servers = IndexMap::new();
        servers.insert(server.id.clone(), server);

        let result = sync_all_live_configs(&servers);

        if let Some(home) = old_home {
            std::env::set_var("HOME", home);
        } else {
            std::env::remove_var("HOME");
        }
        if let Some(userprofile) = old_userprofile {
            std::env::set_var("USERPROFILE", userprofile);
        } else {
            std::env::remove_var("USERPROFILE");
        }
        if let Some(appdata) = old_appdata {
            std::env::set_var("APPDATA", appdata);
        } else {
            std::env::remove_var("APPDATA");
        }

        result.expect("sync all configs");
        let config_path = temp_home
            .path()
            .join(".config")
            .join("mimocode")
            .join("mimocode.json");
        let content = std::fs::read_to_string(config_path).expect("read mimo config");
        let json: serde_json::Value = serde_json::from_str(&content).expect("parse mimo config");
        let demo = json
            .get("mcp")
            .and_then(|value| value.get("demo"))
            .expect("demo server");
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
    }
}
