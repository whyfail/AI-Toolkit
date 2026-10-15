use crate::agents::detect_all_agents;
use crate::app_state::InstalledToolsReport;
use crate::commands::agents::AgentInfo;
use crate::skill_core::tool_adapters::get_tool_status_for_agents;
use std::time::SystemTime;

/// 执行统一的工具检测
pub fn detect_all_tools() -> Result<InstalledToolsReport, String> {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // 检测 Agent 工具
    let detected = detect_all_agents();
    let tool_statuses = get_tool_status_for_agents(&detected).map_err(|e| e.to_string())?;
    let agents: Vec<AgentInfo> = detected.into_iter().map(AgentInfo::from).collect();

    Ok(InstalledToolsReport {
        agents,
        tool_statuses,
        detected_at: now,
    })
}
