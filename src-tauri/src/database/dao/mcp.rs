use indexmap::IndexMap;
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;

use crate::database::Database;
use crate::error::AppError;
use crate::lock_conn;
use crate::mcp::AppType;

/// MCP 服务器应用启用状态
#[derive(Debug, Clone, Default)]
pub struct McpApps {
    pub qwen_code: bool,
    pub claude: bool,
    pub codex: bool,
    pub gemini: bool,
    pub opencode: bool,
    pub trae: bool,
    pub trae_cn: bool,
    pub trae_work: bool,
    pub trae_solo_cn: bool,
    pub qoder: bool,
    pub qodercli: bool,
    pub codebuddy: bool,
    pub hermes: bool,
    pub mimo_code: bool,
    pub workbuddy: bool,
    pub workbuddy_cn: bool,
    pub zcode: bool,
    pub cursor: bool,
    pub windsurf: bool,
    pub cline: bool,
    pub roo: bool,
    pub kilo: bool,
    pub continue_agent: bool,
    pub goose: bool,
    pub kiro: bool,
    pub kimi_code: bool,
    pub minimax_code: bool,
}

impl McpApps {
    pub fn normalized(&self) -> Self {
        let mut apps = self.clone();
        apps.trae |= apps.trae_work;
        apps.trae_cn |= apps.trae_solo_cn;
        apps.trae_work = false;
        apps.trae_solo_cn = false;
        apps.workbuddy |= apps.workbuddy_cn;
        apps.workbuddy_cn = false;
        apps
    }

    pub fn is_enabled_for(&self, app: &AppType) -> bool {
        match app {
            AppType::QwenCode => self.qwen_code,
            AppType::Claude => self.claude,
            AppType::Codex => self.codex,
            AppType::Gemini => self.gemini,
            AppType::OpenCode => self.opencode,
            AppType::Trae | AppType::TraeWork => self.trae || self.trae_work,
            AppType::TraeCn | AppType::TraeSoloCn => self.trae_cn || self.trae_solo_cn,
            AppType::Qoder => self.qoder,
            AppType::Qodercli => self.qodercli,
            AppType::CodeBuddy => self.codebuddy,
            AppType::Hermes => self.hermes,
            AppType::MimoCode => self.mimo_code,
            AppType::WorkBuddy | AppType::WorkBuddyCn => self.workbuddy || self.workbuddy_cn,
            AppType::Zcode => self.zcode,
            AppType::Cursor => self.cursor,
            AppType::Windsurf => self.windsurf,
            AppType::Cline => self.cline,
            AppType::Roo => self.roo,
            AppType::Kilo => self.kilo,
            AppType::Continue => self.continue_agent,
            AppType::Goose => self.goose,
            AppType::Kiro => self.kiro,
            AppType::KimiCode => self.kimi_code,
            AppType::MinimaxCode => self.minimax_code,
        }
    }

    pub fn set_enabled_for(&mut self, app: &AppType, enabled: bool) {
        match app {
            AppType::QwenCode => self.qwen_code = enabled,
            AppType::Claude => self.claude = enabled,
            AppType::Codex => self.codex = enabled,
            AppType::Gemini => self.gemini = enabled,
            AppType::OpenCode => self.opencode = enabled,
            AppType::Trae | AppType::TraeWork => {
                self.trae = enabled;
                self.trae_work = false;
            }
            AppType::TraeCn | AppType::TraeSoloCn => {
                self.trae_cn = enabled;
                self.trae_solo_cn = false;
            }
            AppType::Qoder => self.qoder = enabled,
            AppType::Qodercli => self.qodercli = enabled,
            AppType::CodeBuddy => self.codebuddy = enabled,
            AppType::Hermes => self.hermes = enabled,
            AppType::MimoCode => self.mimo_code = enabled,
            AppType::WorkBuddy | AppType::WorkBuddyCn => {
                self.workbuddy = enabled;
                self.workbuddy_cn = false;
            }
            AppType::Zcode => self.zcode = enabled,
            AppType::Cursor => self.cursor = enabled,
            AppType::Windsurf => self.windsurf = enabled,
            AppType::Cline => self.cline = enabled,
            AppType::Roo => self.roo = enabled,
            AppType::Kilo => self.kilo = enabled,
            AppType::Continue => self.continue_agent = enabled,
            AppType::Goose => self.goose = enabled,
            AppType::Kiro => self.kiro = enabled,
            AppType::KimiCode => self.kimi_code = enabled,
            AppType::MinimaxCode => self.minimax_code = enabled,
        }
    }
}

impl<'de> Deserialize<'de> for McpApps {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let values = HashMap::<String, serde_json::Value>::deserialize(deserializer)?;
        let mut apps = Self::default();
        for (name, enabled) in [
            ("qwen-code", &mut apps.qwen_code),
            ("claude", &mut apps.claude),
            ("codex", &mut apps.codex),
            ("gemini", &mut apps.gemini),
            ("opencode", &mut apps.opencode),
            ("trae", &mut apps.trae),
            ("trae-cn", &mut apps.trae_cn),
            ("trae-work", &mut apps.trae_work),
            ("trae-solo-cn", &mut apps.trae_solo_cn),
            ("qoder", &mut apps.qoder),
            ("qodercli", &mut apps.qodercli),
            ("codebuddy", &mut apps.codebuddy),
            ("hermes", &mut apps.hermes),
            ("mimo-code", &mut apps.mimo_code),
            ("workbuddy", &mut apps.workbuddy),
            ("workbuddy-cn", &mut apps.workbuddy_cn),
            ("zcode", &mut apps.zcode),
            ("cursor", &mut apps.cursor),
            ("windsurf", &mut apps.windsurf),
            ("cline", &mut apps.cline),
            ("roo", &mut apps.roo),
            ("kilo", &mut apps.kilo),
            ("continue", &mut apps.continue_agent),
            ("goose", &mut apps.goose),
            ("kiro", &mut apps.kiro),
            ("kimi-code", &mut apps.kimi_code),
            ("minimax-code", &mut apps.minimax_code),
        ] {
            if let Some(value) = values.get(name) {
                *enabled =
                    serde_json::from_value(value.clone()).map_err(serde::de::Error::custom)?;
            }
        }
        Ok(apps.normalized())
    }
}

impl Serialize for McpApps {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let apps = self.normalized();
        let mut map = serializer.serialize_map(Some(24))?;
        for (name, enabled) in [
            ("qwen-code", apps.qwen_code),
            ("claude", apps.claude),
            ("codex", apps.codex),
            ("gemini", apps.gemini),
            ("opencode", apps.opencode),
            ("trae", apps.trae),
            ("trae-cn", apps.trae_cn),
            ("qoder", apps.qoder),
            ("qodercli", apps.qodercli),
            ("codebuddy", apps.codebuddy),
            ("hermes", apps.hermes),
            ("mimo-code", apps.mimo_code),
            ("workbuddy", apps.workbuddy),
            ("zcode", apps.zcode),
            ("cursor", apps.cursor),
            ("windsurf", apps.windsurf),
            ("cline", apps.cline),
            ("roo", apps.roo),
            ("kilo", apps.kilo),
            ("continue", apps.continue_agent),
            ("goose", apps.goose),
            ("kiro", apps.kiro),
            ("kimi-code", apps.kimi_code),
            ("minimax-code", apps.minimax_code),
        ] {
            map.serialize_entry(name, &enabled)?;
        }
        map.end()
    }
}

/// MCP 服务器配置（宽松结构）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
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
                        enabled_trae_work, enabled_trae_solo_cn, enabled_qoder, enabled_qodercli,
                        enabled_codebuddy, enabled_hermes, enabled_mimo_code, enabled_workbuddy,
                        enabled_zcode, enabled_cursor, enabled_windsurf, enabled_cline, enabled_roo, enabled_kilo, enabled_continue, enabled_goose, enabled_kiro, enabled_kimi_code, enabled_minimax_code
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
                let enabled_trae_work: bool = row.get(14)?;
                let enabled_trae_solo_cn: bool = row.get(15)?;
                let enabled_qoder: bool = row.get(16)?;
                let enabled_qodercli: bool = row.get(17)?;
                let enabled_codebuddy: bool = row.get(18)?;
                let enabled_hermes: bool = row.get(19)?;
                let enabled_mimo_code: bool = row.get(20)?;
                let enabled_workbuddy: bool = row.get(21)?;
                let enabled_zcode: bool = row.get(22)?;
                let enabled_cursor: bool = row.get(23)?;
                let enabled_windsurf: bool = row.get(24)?;
                let enabled_cline: bool = row.get(25)?;
                let enabled_roo: bool = row.get(26)?;
                let enabled_kilo: bool = row.get(27)?;
                let enabled_continue: bool = row.get(28)?;
                let enabled_goose: bool = row.get(29)?;
                let enabled_kiro: bool = row.get(30)?;
                let enabled_kimi_code: bool = row.get(31)?;
                let enabled_minimax_code: bool = row.get(32)?;

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
                            trae_work: enabled_trae_work,
                            trae_solo_cn: enabled_trae_solo_cn,
                            qoder: enabled_qoder,
                            qodercli: enabled_qodercli,
                            codebuddy: enabled_codebuddy,
                            hermes: enabled_hermes,
                            mimo_code: enabled_mimo_code,
                            workbuddy: enabled_workbuddy,
                            workbuddy_cn: enabled_workbuddy,
                            zcode: enabled_zcode,
                            cursor: enabled_cursor,
                            windsurf: enabled_windsurf,
                            cline: enabled_cline,
                            roo: enabled_roo,
                            kilo: enabled_kilo,
                            continue_agent: enabled_continue,
                            goose: enabled_goose,
                            kiro: enabled_kiro,
                            kimi_code: enabled_kimi_code,
                            minimax_code: enabled_minimax_code,
                        }
                        .normalized(),
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
        self.save_mcp_servers(std::slice::from_ref(server))
    }

    /// 在一个事务中保存全部 MCP 服务器，失败时不留下部分导入结果。
    pub fn save_mcp_servers(&self, servers: &[McpServer]) -> Result<(), AppError> {
        let mut conn = lock_conn!(self.conn);
        let transaction = conn
            .transaction()
            .map_err(|e| AppError::Database(e.to_string()))?;
        for server in servers {
            save_mcp_server_on_connection(&transaction, server)?;
        }
        transaction
            .commit()
            .map_err(|e| AppError::Database(e.to_string()))
    }

    /// 原子恢复 MCP 快照，由调用方先确认期间没有新增编辑。
    pub fn replace_mcp_servers(&self, servers: &[McpServer]) -> Result<(), AppError> {
        let mut conn = lock_conn!(self.conn);
        let transaction = conn
            .transaction()
            .map_err(|e| AppError::Database(e.to_string()))?;
        transaction
            .execute("DELETE FROM mcp_servers", [])
            .map_err(|e| AppError::Database(e.to_string()))?;
        for server in servers {
            save_mcp_server_on_connection(&transaction, server)?;
        }
        transaction
            .commit()
            .map_err(|e| AppError::Database(e.to_string()))
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

fn save_mcp_server_on_connection(
    conn: &rusqlite::Connection,
    server: &McpServer,
) -> Result<(), AppError> {
    let apps = server.apps.normalized();
    conn.execute(
            "INSERT OR REPLACE INTO mcp_servers (
                id, name, server_config, description, homepage, docs, tags,
                enabled_qwen_code, enabled_claude, enabled_codex, enabled_gemini,
                enabled_opencode, enabled_trae, enabled_trae_cn,
                enabled_trae_work, enabled_trae_solo_cn, enabled_qoder, enabled_qodercli,
                enabled_codebuddy, enabled_hermes, enabled_mimo_code, enabled_workbuddy,
                enabled_zcode, enabled_cursor, enabled_windsurf, enabled_cline, enabled_roo, enabled_kilo, enabled_continue, enabled_goose, enabled_kiro, enabled_kimi_code, enabled_minimax_code, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33,
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
                apps.qwen_code,
                apps.claude,
                apps.codex,
                apps.gemini,
                apps.opencode,
                apps.trae,
                apps.trae_cn,
                apps.trae_work,
                apps.trae_solo_cn,
                apps.qoder,
                apps.qodercli,
                apps.codebuddy,
                apps.hermes,
                apps.mimo_code,
                apps.workbuddy || apps.workbuddy_cn,
                apps.zcode,
                apps.cursor,
                apps.windsurf,
                apps.cline,
                apps.roo,
                apps.kilo,
                apps.continue_agent,
                apps.goose,
                apps.kiro,
                apps.kimi_code,
                apps.minimax_code,
            ],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}
