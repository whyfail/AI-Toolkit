use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::database::Database;
use crate::error::AppError;
use crate::lock_conn;
use crate::mcp::AppType;

/// MCP 服务器应用启用状态
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct McpApps {
    #[serde(default)]
    pub qwen_code: bool,
    #[serde(default)]
    pub claude: bool,
    #[serde(default)]
    pub codex: bool,
    #[serde(default)]
    pub gemini: bool,
    #[serde(default)]
    pub opencode: bool,
    #[serde(default)]
    pub trae: bool,
    #[serde(default)]
    pub trae_cn: bool,
    #[serde(default)]
    pub trae_solo_cn: bool,
    #[serde(default)]
    pub qoder: bool,
    #[serde(default)]
    pub qodercli: bool,
    #[serde(default)]
    pub codebuddy: bool,
}

impl McpApps {
    pub fn is_enabled_for(&self, app: &AppType) -> bool {
        match app {
            AppType::QwenCode => self.qwen_code,
            AppType::Claude => self.claude,
            AppType::Codex => self.codex,
            AppType::Gemini => self.gemini,
            AppType::OpenCode => self.opencode,
            AppType::Trae => self.trae,
            AppType::TraeCn => self.trae_cn,
            AppType::TraeSoloCn => self.trae_solo_cn,
            AppType::Qoder => self.qoder,
            AppType::Qodercli => self.qodercli,
            AppType::CodeBuddy => self.codebuddy,
        }
    }

    pub fn set_enabled_for(&mut self, app: &AppType, enabled: bool) {
        match app {
            AppType::QwenCode => self.qwen_code = enabled,
            AppType::Claude => self.claude = enabled,
            AppType::Codex => self.codex = enabled,
            AppType::Gemini => self.gemini = enabled,
            AppType::OpenCode => self.opencode = enabled,
            AppType::Trae => self.trae = enabled,
            AppType::TraeCn => self.trae_cn = enabled,
            AppType::TraeSoloCn => self.trae_solo_cn = enabled,
            AppType::Qoder => self.qoder = enabled,
            AppType::Qodercli => self.qodercli = enabled,
            AppType::CodeBuddy => self.codebuddy = enabled,
        }
    }
}

/// MCP 服务器配置（宽松结构）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct McpServerSpec {
    #[serde(rename = "type", default)]
    pub spec_type: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Option<Vec<String>>,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// MCP 服务器条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServer {
    pub id: String,
    pub name: String,
    pub server: McpServerSpec,
    #[serde(default)]
    pub apps: McpApps,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub docs: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl Database {
    /// 获取所有 MCP 服务器
    pub fn get_all_mcp_servers(&self) -> Result<IndexMap<String, McpServer>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn
            .prepare(
                "SELECT id, name, server_config, description, homepage, docs, tags,
                        enabled_qwen_code, enabled_claude, enabled_codex, enabled_gemini,
                        enabled_opencode, enabled_trae, enabled_trae_cn,
                        enabled_trae_solo_cn, enabled_qoder, enabled_codebuddy
                 FROM mcp_servers
                 ORDER BY name ASC, id ASC",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;

        let server_iter = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let server_config_str: String = row.get(2)?;
                let description: Option<String> = row.get(3)?;
                let homepage: Option<String> = row.get(4)?;
                let docs: Option<String> = row.get(5)?;
                let tags_str: String = row.get(6)?;
                let enabled_qwen_code: bool = row.get(7)?;
                let enabled_claude: bool = row.get(8)?;
                let enabled_codex: bool = row.get(9)?;
                let enabled_gemini: bool = row.get(10)?;
                let enabled_opencode: bool = row.get(11)?;
                let enabled_trae: bool = row.get(12)?;
                let enabled_trae_cn: bool = row.get(13)?;
                let enabled_trae_solo_cn: bool = row.get(14)?;
                let enabled_qoder: bool = row.get(15)?;
                let enabled_codebuddy: bool = row.get(16)?;

                let server: McpServerSpec =
                    serde_json::from_str(&server_config_str).unwrap_or_default();
                let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();

                Ok((
                    id.clone(),
                    McpServer {
                        id,
                        name,
                        server,
                        apps: McpApps {
                            qwen_code: enabled_qwen_code,
                            claude: enabled_claude,
                            codex: enabled_codex,
                            gemini: enabled_gemini,
                            opencode: enabled_opencode,
                            trae: enabled_trae,
                            trae_cn: enabled_trae_cn,
                            trae_solo_cn: enabled_trae_solo_cn,
                            qoder: enabled_qoder,
                            qodercli: false,
                            codebuddy: enabled_codebuddy,
                        },
                        description,
                        homepage,
                        docs,
                        tags,
                    },
                ))
            })
            .map_err(|e| AppError::Database(e.to_string()))?;

        let mut servers = IndexMap::new();
        for server_res in server_iter {
            let (id, server) = server_res.map_err(|e| AppError::Database(e.to_string()))?;
            servers.insert(id, server);
        }
        Ok(servers)
    }

    /// 保存 MCP 服务器
    pub fn save_mcp_server(&self, server: &McpServer) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute(
            "INSERT OR REPLACE INTO mcp_servers (
                id, name, server_config, description, homepage, docs, tags,
                enabled_qwen_code, enabled_claude, enabled_codex, enabled_gemini,
                enabled_opencode, enabled_trae, enabled_trae_cn,
                enabled_trae_solo_cn, enabled_qoder, enabled_codebuddy, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                      strftime('%s', 'now') * 1000)",
            rusqlite::params![
                server.id,
                server.name,
                serde_json::to_string(&server.server).map_err(|e| {
                    AppError::Database(format!("Failed to serialize server config: {}", e))
                })?,
                server.description,
                server.homepage,
                server.docs,
                serde_json::to_string(&server.tags)
                    .map_err(|e| AppError::Database(format!("Failed to serialize tags: {}", e)))?,
                server.apps.qwen_code,
                server.apps.claude,
                server.apps.codex,
                server.apps.gemini,
                server.apps.opencode,
                server.apps.trae,
                server.apps.trae_cn,
                server.apps.trae_solo_cn,
                server.apps.qoder,
                server.apps.codebuddy,
            ],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    /// 删除 MCP 服务器
    pub fn delete_mcp_server(&self, id: &str) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute(
            "DELETE FROM mcp_servers WHERE id = ?1",
            rusqlite::params![id],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }
}
