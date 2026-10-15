use crate::commands::agents::AgentInfo;
use crate::credits::CreditManager;
use crate::database::Database;
use crate::error::AppError;
use crate::skill_core::tool_adapters::ToolStatus;
use serde::Serialize;
use std::sync::Arc;
use std::sync::RwLock;

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
    pub credits: Arc<CreditManager>,
    pub mcp_gate: parking_lot::Mutex<()>,
    /// 已安装工具的缓存（启动时检测一次）
    pub installed_tools: RwLock<InstalledToolsReport>,
}

impl AppState {
    pub fn new(db: Database) -> Result<Self, AppError> {
        let credits = CreditManager::new(db.conn.clone()).map_err(AppError::Database)?;
        Ok(Self {
            db,
            credits,
            mcp_gate: parking_lot::Mutex::new(()),
            installed_tools: RwLock::new(InstalledToolsReport::default()),
        })
    }
}
