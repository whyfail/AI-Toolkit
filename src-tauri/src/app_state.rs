use serde::Serialize;
use crate::database::Database;
use std::sync::RwLock;
use crate::skill_core::tool_adapters::ToolStatus;
use crate::commands::agents::AgentInfo;

/// 已安装工具的统一报告（全局缓存）
#[derive(Clone, Debug, Serialize)]
pub struct InstalledToolsReport {
    /// Agent 工具列表（MCP 服务器用）
    pub agents: Vec<AgentInfo>,
    /// 工具状态列表（Skills 管理用）
    pub tool_statuses: Vec<ToolStatus>,
    /// 检测时间戳
    pub detected_at: i64,
}

impl Default for InstalledToolsReport {
    fn default() -> Self {
        Self {
            agents: Vec::new(),
            tool_statuses: Vec::new(),
            detected_at: 0,
        }
    }
}

/// 应用全局状态
pub struct AppState {
    pub db: Database,
    /// 已安装工具的缓存（启动时检测一次）
    pub installed_tools: RwLock<InstalledToolsReport>,
}

impl AppState {
    pub fn new(db: Database) -> Self {
        Self {
            db,
            installed_tools: RwLock::new(InstalledToolsReport::default()),
        }
    }
}
