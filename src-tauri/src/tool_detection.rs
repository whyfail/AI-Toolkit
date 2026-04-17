use crate::agents::detect_all_agents;
use crate::app_state::InstalledToolsReport;
use crate::skill_core::tool_adapters::get_all_tool_status;
use crate::commands::agents::AgentInfo;
use std::time::SystemTime;

/// 执行统一的工具检测
pub fn detect_all_tools() -> Result<InstalledToolsReport, String> {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // 检测 Agent 工具
    let agents: Vec<AgentInfo> = detect_all_agents()
        .into_iter()
        .map(AgentInfo::from)
        .collect();

    // 检测所有工具状态（Skills 管理用）
    let tool_statuses = get_all_tool_status().map_err(|e| e.to_string())?;

    Ok(InstalledToolsReport {
        agents,
        tool_statuses,
        detected_at: now,
    })
}
