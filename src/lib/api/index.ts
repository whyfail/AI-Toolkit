import { invoke } from "@tauri-apps/api/core";
import type { McpServer, AppConfigInfo, ToolInfo } from "@/types";
import type { AgentInfo, ToolStatus, ToolAdapter, DetectedSkill, InstalledToolsReport } from "@/contexts/InstalledToolsContext";

// Re-export types for external use
export type { AgentInfo, ToolStatus, ToolAdapter, DetectedSkill, InstalledToolsReport };

// MCP API
export const mcpApi = {
  // 获取所有 MCP 服务器
  async getAllServers(): Promise<Record<string, McpServer>> {
    return invoke<Record<string, McpServer>>("get_mcp_servers");
  },

  // 添加或更新 MCP 服务器
  async upsertServer(server: McpServer): Promise<void> {
    return invoke("upsert_mcp_server", { server });
  },

  // 删除 MCP 服务器
  async deleteServer(id: string): Promise<void> {
    return invoke("delete_mcp_server", { id });
  },

  // 切换应用启用状态
  async toggleApp(
    serverId: string,
    app: string,
    enabled: boolean
  ): Promise<void> {
    return invoke("toggle_mcp_app", { serverId, app, enabled });
  },

  // 从所有应用导入
  async importFromApps(): Promise<number> {
    return invoke<number>("import_mcp_from_apps");
  },
};

// 应用配置 API
export const appApi = {
  // 获取应用配置
  async getAppConfigs(): Promise<AppConfigInfo[]> {
    return invoke<AppConfigInfo[]>("get_app_configs");
  },

  // 从指定应用导入
  async importFromApp(appId: string): Promise<number> {
    return invoke<number>("import_mcp_from_app", { appId });
  },
};

// 工具管理 API
export const toolApi = {
  // 获取所有工具信息
  async getToolInfos(): Promise<ToolInfo[]> {
    return invoke<ToolInfo[]>("get_tool_infos");
  },

  // 获取单个工具信息
  async getToolInfo(appType: string): Promise<ToolInfo> {
    return invoke<ToolInfo>("get_tool_info", { appType });
  },

  // 安装工具
  async installTool(appType: string, methodIndex: number): Promise<void> {
    return invoke("install_tool", { appType, methodIndex });
  },

  // 更新工具
  async updateTool(appType: string): Promise<void> {
    return invoke("update_tool", { appType });
  },

  // 获取工具主页 URL
  async getToolHomepage(appType: string): Promise<string> {
    return invoke<string>("get_tool_homepage", { appType });
  },

  // 获取已安装工具的缓存数据（启动时检测一次）
  async getInstalledTools(): Promise<InstalledToolsReport> {
    return invoke<InstalledToolsReport>("get_installed_tools");
  },

  // 手动刷新已安装工具的检测（工具管理模块的刷新按钮）
  async refreshInstalledTools(): Promise<InstalledToolsReport> {
    return invoke<InstalledToolsReport>("refresh_installed_tools");
  },
};
