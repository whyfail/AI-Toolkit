use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use indexmap::IndexMap;
use serde_yaml::Value;

use crate::agents::resolve_path;
use crate::database::{McpApps, McpServer, McpServerSpec};
use crate::error::AppError;
use crate::mcp::AppType;

const MCP_PLUGIN: &str = "@deepseek-ai/dsh-mcp-client";

fn data_root(variable: &str, fallback: &str) -> PathBuf {
    let path = std::env::var(variable)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback.into());
    let path = resolve_path(&path);
    if path.is_absolute() {
        path
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    }
}

pub(crate) fn home_dir() -> PathBuf {
    data_root("DSH_HOME", "~/.dsh")
}

pub(crate) fn shared_skills_dir() -> PathBuf {
    data_root("DSH_AGENTS_HOME", "~/.agents").join("skills")
}

fn invalid(message: impl Into<String>) -> AppError {
    AppError::Parse(format!(
        "DeepSeek Harness：{}，未覆盖原文件",
        message.into()
    ))
}

/// serde_yaml drops unknown standard tags such as !!js. Give the secondary tag
/// handle a unique local prefix while parsing, then restore it when serializing.
/// The prefix never occurs in the input, so literal strings remain unchanged.
fn parse_patch(content: &str) -> Result<(Vec<Value>, String), AppError> {
    let prefix = format!("!ai-toolkit-{}-", uuid::Uuid::new_v4().simple());
    let content = content.trim_start_matches('\u{feff}');
    let has_header = content
        .lines()
        .find(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
        .is_some_and(|line| line == "---" || line.starts_with('%'));
    let source = format!(
        "%TAG !! {prefix}\n{}{content}",
        if has_header { "" } else { "---\n" }
    );
    let value: Value = serde_yaml::from_str(&source).map_err(|error| invalid(error.to_string()))?;
    match value {
        Value::Null => Ok((vec![], prefix)),
        Value::Sequence(patches) => Ok((patches, prefix)),
        _ => Err(invalid("cordis.patch.yml 必须是 YAML patch 数组")),
    }
}

/// Only root inserts belong to the machine-wide scope. Profile and nested Agent
/// scopes are independent and must not be flattened into the global MCP list.
fn root_entries(patches: &[Value]) -> Result<IndexMap<String, Value>, AppError> {
    let mut entries = IndexMap::new();
    for patch in patches {
        if !patch.is_mapping() {
            return Err(invalid("patch 条目必须是对象"));
        }
        if let Some(insert) = patch.get("insert") {
            let insert = insert
                .as_sequence()
                .ok_or_else(|| invalid("insert 必须是数组"))?;
            if patch.get("id").is_none() {
                for entry in insert {
                    if let Some(id) = entry.get("id").and_then(Value::as_str) {
                        entries.insert(id.to_string(), entry.clone());
                    } else if entry.get("name").and_then(Value::as_str) == Some(MCP_PLUGIN) {
                        return Err(invalid("MCP 插入项缺少 id"));
                    }
                }
            }
        } else if let Some(target) = patch
            .get("id")
            .and_then(Value::as_str)
            .and_then(|id| entries.get_mut(id))
        {
            if patch.get("name").is_some() && patch.get("name") != target.get("name") {
                continue;
            }
            let target = target
                .as_mapping_mut()
                .ok_or_else(|| invalid("插件条目必须是对象"))?;
            for (key, value) in patch.as_mapping().unwrap() {
                if key.as_str() != Some("id") && key.as_str() != Some("name") {
                    target.insert(key.clone(), value.clone());
                }
            }
        }
    }
    entries.retain(|_, entry| entry.get("name").and_then(Value::as_str) == Some(MCP_PLUGIN));
    Ok(entries)
}

fn has_tag(value: &Value) -> bool {
    match value {
        Value::Tagged(_) => true,
        Value::Sequence(values) => values.iter().any(has_tag),
        Value::Mapping(values) => values
            .iter()
            .any(|(key, value)| has_tag(key) || has_tag(value)),
        _ => false,
    }
}

fn servers_from_entries(
    entries: &IndexMap<String, Value>,
) -> Result<IndexMap<String, McpServer>, AppError> {
    let mut servers = IndexMap::new();
    for entry in entries.values() {
        let config = entry
            .get("config")
            .ok_or_else(|| invalid("MCP 插件缺少 config"))?;
        if has_tag(config) || entry.get("disabled").is_some_and(has_tag) {
            return Err(invalid(
                "MCP 含动态 YAML 表达式，无法作为静态服务器导入或同步",
            ));
        }
        let mut config: serde_json::Value =
            serde_json::to_value(config).map_err(|error| invalid(error.to_string()))?;
        let config = config
            .as_object_mut()
            .ok_or_else(|| invalid("MCP config 必须是对象"))?;
        let name = config
            .remove("serverName")
            .and_then(|name| name.as_str().map(str::to_string))
            .ok_or_else(|| invalid("MCP 缺少 serverName"))?;
        let transport = match config
            .remove("transport")
            .and_then(|value| value.as_str().map(str::to_string))
            .as_deref()
        {
            Some("stdio") => "stdio",
            Some("streamable-http") => "http",
            _ => return Err(invalid("MCP transport 仅支持 stdio 或 streamable-http")),
        };
        config.insert("type".into(), transport.into());
        let server: McpServerSpec =
            serde_json::from_value(serde_json::Value::Object(config.clone()))
                .map_err(|error| invalid(error.to_string()))?;
        let mut apps = McpApps::default();
        apps.set_enabled_for(
            &AppType::DeepSeekHarness,
            entry.get("disabled").and_then(Value::as_bool) != Some(true),
        );
        let server = McpServer {
            id: name.clone(),
            name: name.clone(),
            server,
            apps,
            description: None,
            homepage: None,
            docs: None,
            tags: vec![],
        };
        if servers.insert(name, server).is_some() {
            return Err(invalid("存在重复的 MCP serverName"));
        }
    }
    Ok(servers)
}

pub(crate) fn parse_servers(content: &str) -> Result<IndexMap<String, McpServer>, AppError> {
    let (patches, _) = parse_patch(content)?;
    servers_from_entries(&root_entries(&patches)?)
}

pub(crate) fn sync_config(path: &PathBuf, servers: &[McpServer]) -> Result<(), AppError> {
    let content = if path.is_file() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };
    let (mut patches, prefix) = parse_patch(&content)?;
    let before = patches.clone();
    let entries = root_entries(&patches)?;
    let existing = servers_from_entries(&entries)?;
    let ids: HashSet<_> = entries.keys().cloned().collect();
    let mut inserted = Vec::new();
    let mut names = HashSet::new();
    for server in servers {
        let name = &server.id;
        if name.is_empty()
            || name.len() > 32
            || !name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        {
            return Err(invalid(format!(
                "MCP ID {name:?} 必须是 1–32 位字母、数字、下划线或连字符"
            )));
        }
        if !names.insert(name) {
            return Err(invalid("存在重复的 MCP ID"));
        }
        let transport = match server.server.spec_type.as_deref() {
            Some("stdio") | None if server.server.command.is_some() => "stdio",
            Some("http" | "streamable-http") | None if server.server.url.is_some() => {
                "streamable-http"
            }
            _ => {
                return Err(invalid(format!(
                    "MCP {name:?} 仅支持带 command 的 stdio 或带 url 的 Streamable HTTP"
                )))
            }
        };
        let mut config =
            serde_json::to_value(&server.server).map_err(|error| invalid(error.to_string()))?;
        let object = config.as_object_mut().unwrap();
        object.retain(|key, value| key != "type" && !value.is_null());
        object.insert("serverName".into(), name.clone().into());
        object.insert("transport".into(), transport.into());
        let previous = entries.values().find(|entry| {
            entry
                .get("config")
                .and_then(|config| config.get("serverName"))
                .and_then(Value::as_str)
                == Some(name)
        });
        let mut entry = previous.cloned().unwrap_or_else(|| {
            serde_yaml::to_value(serde_json::json!({
                "id": format!("ai-toolkit-mcp-{name}"), "name": MCP_PLUGIN
            }))
            .unwrap()
        });
        entry["config"] =
            serde_yaml::to_value(config).map_err(|error| invalid(error.to_string()))?;
        entry
            .as_mapping_mut()
            .unwrap()
            .remove(Value::String("disabled".into()));
        inserted.push(entry);
    }
    // Nothing to manage: avoid rewriting unrelated preferences during a scan/sync.
    if existing.is_empty() && inserted.is_empty() {
        return Ok(());
    }
    patches.retain_mut(|patch| {
        if patch.get("insert").is_none() {
            return !patch
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| ids.contains(id));
        }
        if patch.get("id").is_none() {
            let insert = patch["insert"].as_sequence_mut().unwrap();
            insert.retain(|entry| entry.get("name").and_then(Value::as_str) != Some(MCP_PLUGIN));
            return !insert.is_empty();
        }
        true
    });
    if !inserted.is_empty() {
        let mut patch = serde_yaml::Mapping::new();
        patch.insert("insert".into(), Value::Sequence(inserted));
        patches.push(Value::Mapping(patch));
    }
    if patches == before {
        return Ok(());
    }
    let output = serde_yaml::to_string(&patches)
        .map_err(|error| invalid(error.to_string()))?
        .replace(&prefix, "!!");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    super::sync::atomic_write(path, &output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn server(id: &str, spec: serde_json::Value) -> McpServer {
        McpServer {
            id: id.into(),
            name: id.into(),
            server: serde_json::from_value(spec).unwrap(),
            apps: McpApps::default(),
            description: None,
            homepage: None,
            docs: None,
            tags: vec![],
        }
    }

    #[test]
    fn roundtrips_mcp_and_preserves_preferences_expressions_and_nested_scopes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cordis.patch.yml");
        fs::write(
            &path,
            r#"# user preferences
---
- id: llm-provider
  config:
    apiKey: !!js process.env.MODEL_KEY
    label: 'literal !!js process.env.KEY'
- insert:
  - id: agent-group
    name: cordis:group
    group: true
    config:
    - id: nested-mcp
      name: '@deepseek-ai/dsh-mcp-client'
      config:
        serverName: scoped
        transport: stdio
        command: scoped-tool
  - id: old-entry
    name: '@deepseek-ai/dsh-mcp-client'
    config:
      serverName: local
      transport: stdio
      command: old-command
      toolCallTimeoutMs: 45000
- id: old-entry
  config:
    serverName: local
    transport: stdio
    command: updated-command
    toolCallTimeoutMs: 90000
"#,
        )
        .unwrap();
        let imported = parse_servers(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(imported.len(), 1);
        assert_eq!(
            imported["local"].server.command.as_deref(),
            Some("updated-command")
        );
        let mut local = imported["local"].clone();
        local.server.args = Some(vec!["--serve".into()]);
        local.server.env = Some([("EXAMPLE_KEY".into(), "example".into())].into());
        local.server.cwd = Some("/example".into());
        let remote = server(
            "remote",
            json!({"type":"http", "url":"https://example.com/mcp", "headers":{"X-Test":"example"}}),
        );
        sync_config(&path, &[local.clone(), remote.clone()]).unwrap();
        let output = fs::read_to_string(&path).unwrap();
        assert!(output.contains("!!js process.env.MODEL_KEY"));
        assert!(output.contains("literal !!js process.env.KEY"));
        assert!(output.contains("nested-mcp"));
        assert!(output.contains("serverName: scoped"));
        let reimported = parse_servers(&output).unwrap();
        assert_eq!(reimported["local"].server, local.server);
        assert_eq!(reimported["remote"].server, remote.server);
        assert_eq!(reimported["local"].server.extra["toolCallTimeoutMs"], 90000);
        sync_config(&path, &[local, remote]).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), output);
        sync_config(&path, &[]).unwrap();
        let cleared = fs::read_to_string(&path).unwrap();
        assert!(parse_servers(&cleared).unwrap().is_empty());
        assert!(cleared.contains("nested-mcp"));
        assert!(cleared.contains("!!js process.env.MODEL_KEY"));
    }

    #[test]
    fn creates_home_patch_with_root_insert_and_imports_disabled_servers() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("new-home/cordis.patch.yml");
        sync_config(
            &path,
            &[server("example", json!({"command":"example-tool"}))],
        )
        .unwrap();
        let output = fs::read_to_string(&path).unwrap();
        assert!(output.starts_with("- insert:"));
        assert!(output.contains("transport: stdio"));
        let disabled = format!("{output}- id: ai-toolkit-mcp-example\n  disabled: true\n");
        let imported = parse_servers(&disabled).unwrap();
        assert!(!imported["example"]
            .apps
            .is_enabled_for(&AppType::DeepSeekHarness));
    }

    #[test]
    fn rejects_invalid_or_dynamic_mcp_without_modifying_the_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cordis.patch.yml");
        for original in [
            "{invalid yaml",
            "mcpServers: {}",
            "- insert: wrong\n",
            "- insert:\n  - id: dynamic\n    name: '@deepseek-ai/dsh-mcp-client'\n    config:\n      serverName: dynamic\n      transport: stdio\n      command: example-tool\n      env:\n        TOKEN: !!js process.env.TOKEN\n",
        ] {
            fs::write(&path, original).unwrap();
            assert!(sync_config(&path, &[]).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), original);
        }
        let original = "# preserve me\n- id: llm\n  config:\n    apiKey: !!js process.env.KEY\n";
        fs::write(&path, original).unwrap();
        for invalid_server in [
            server(
                "legacy",
                json!({"type":"sse", "url":"https://example.com/sse"}),
            ),
            server("name.with.dots", json!({"command":"example-tool"})),
            server("missing-command", json!({"type":"stdio"})),
        ] {
            assert!(sync_config(&path, &[invalid_server]).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), original);
        }
        sync_config(&path, &[]).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
    }
}
