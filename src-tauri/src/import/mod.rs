use indexmap::IndexMap;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::agents::get_agent_config_paths;
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

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return None,
    };

    let is_toml = path.extension().and_then(|s| s.to_str()) == Some("toml");
    let servers = if is_toml {
        parse_mcp_toml(&content, &app)
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
fn parse_mcp_json(content: &str, app: &AppType) -> Option<IndexMap<String, McpServer>> {
    let json: serde_json::Value = match serde_json::from_str(content) {
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
    // Trae / OpenCode: { "mcp": { "id": { ... } } }
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

/// 解析单个服务器配置
fn parse_server_config(id: &str, config: &serde_json::Value, app: &AppType) -> Option<McpServer> {
    let config_obj = config.as_object()?;

    // 提取基本字段
    // OpenCode 格式: command 是 string[] (如 ["npx", "-y", "xxx"])，无 args
    // 其他工具格式: command 是 string，args 是 string[]
    let (command, args) = if let Some(cmd_val) = config_obj.get("command") {
        if let Some(arr) = cmd_val.as_array() {
            // command 是数组 (OpenCode 格式): 第一个元素作为 command，其余作为 args
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
    // 同时支持 "env" (标准格式) 和 "environment" (OpenCode 格式)
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
        .and_then(|v| v.as_str())
        .map(String::from);
    let headers = config_obj.get("headers").and_then(|v| {
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

    let spec_type = if command.is_some() {
        Some("stdio".to_string())
    } else if url.is_some() {
        Some("http".to_string())
    } else {
        None
    };

    // 构建服务器规范
    let server = McpServerSpec {
        spec_type,
        command,
        args,
        env,
        cwd,
        url,
        headers,
        extra: HashMap::new(),
    };

    // 创建应用状态
    let mut apps = McpApps::default();
    apps.set_enabled_for(app, true);

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

/// 展开 ~ 为 HOME 目录
fn expand_home(path: &str) -> String {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped).to_string_lossy().to_string();
        }
    }
    path.to_string()
}

/// 从所有支持的来源导入 MCP 配置
pub fn import_all() -> IndexMap<String, McpServer> {
    let mut all_servers: IndexMap<String, McpServer> = IndexMap::new();

    // Iterate through all apps and check their OS-specific paths
    for app in AppType::all() {
        let paths = get_agent_config_paths(&app);

        // Check each possible path for this app
        for path in &paths {
            if let Some(result) = import_from_path(app.clone(), path) {
                for (id, server) in result.servers {
                    // If server already exists, merge the app status
                    if let Some(existing) = all_servers.get_mut(&id) {
                        existing.apps.set_enabled_for(&app, true);
                        let tag = format!("imported-from-{}", app.name());
                        if !existing.tags.contains(&tag) {
                            existing.tags.push(tag);
                        }
                    } else {
                        all_servers.insert(id, server);
                    }
                }
                // Break after finding the first valid path for this app
                break;
            }
        }
    }

    all_servers
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct ClaudeMcpConfig {
    #[serde(rename = "mcpServers")]
    mcp_servers: Option<HashMap<String, serde_json::Value>>,
}
