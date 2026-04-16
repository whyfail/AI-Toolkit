use indexmap::IndexMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::agents::resolve_path;
use crate::database::McpServer;
use crate::error::AppError;
use crate::mcp::AppType;

/// 同步指定应用的 MCP 配置到其配置文件
pub fn sync_app_config(app: &AppType, servers: &[McpServer]) -> Result<(), AppError> {
    let config_path = resolve_path(&get_config_path_for_app(app)?);

    if matches!(app, AppType::Codex) {
        return sync_codex_config(&config_path, servers);
    }

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
    // OpenCode 使用专用的配置格式
    let mcp_servers = if matches!(app, AppType::OpenCode) {
        build_opencode_mcp_json(servers)
    } else {
        build_mcp_json(servers)
    };

    // 根据应用类型确定键名
    let key = match app {
        AppType::OpenCode => "mcp",
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

fn build_mcp_json(servers: &[McpServer]) -> serde_json::Map<String, serde_json::Value> {
    let mut mcp_servers = serde_json::Map::new();
    for server in servers {
        let mut entry = serde_json::Map::new();

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
        // 保留其他额外字段
        for (k, v) in &server.server.extra {
            entry.insert(k.clone(), v.clone());
        }

        mcp_servers.insert(server.id.clone(), serde_json::Value::Object(entry));
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
        if server.server.url.is_some() {
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

/// 同步所有应用的 MCP 配置（包括没有任何启用服务器的应用，以清除残留配置）
pub fn sync_all_live_configs(servers: &IndexMap<String, McpServer>) -> Result<(), AppError> {
    // 遍历所有应用类型，确保即使没有启用服务器也会同步（清除配置文件中的残留）
    for app in AppType::all() {
        let app_servers: Vec<McpServer> = servers
            .values()
            .filter(|s| s.apps.is_enabled_for(&app))
            .cloned()
            .collect();
        sync_app_config(&app, &app_servers)?;
    }

    Ok(())
}

fn get_config_path_for_app(app: &AppType) -> Result<String, AppError> {
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
        AppType::TraeSoloCn => {
            if cfg!(windows) {
                "%APPDATA%\\TRAE SOLO CN\\User\\mcp.json"
            } else {
                "~/Library/Application Support/TRAE SOLO CN/User/mcp.json"
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
    }
    .to_string())
}

fn atomic_write(path: &PathBuf, content: &str) -> Result<(), AppError> {
    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, content).map_err(|e| {
        AppError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            e.to_string(),
        ))
    })?;
    fs::rename(&temp_path, path).map_err(|e| {
        AppError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            e.to_string(),
        ))
    })?;
    Ok(())
}
