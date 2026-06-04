import { invoke } from "@tauri-apps/api/core";
import type { McpServer, AppConfigInfo, LaunchPreferences, ToolInfo } from "@/types";
import type { AgentInfo, ToolStatus, ToolAdapter, DetectedSkill, InstalledToolsReport } from "@/contexts/InstalledToolsContext";
import type {
  FeaturedSkillDto,
  GitSkillCandidate,
  ManagedSkill,
  OnlineSkillDto,
  OnboardingPlan,
} from "@/components/skills/types";

// Re-export types for external use
export type { AgentInfo, ToolStatus, ToolAdapter, DetectedSkill, InstalledToolsReport };

type UpdateInfo = {
  available: boolean;
  version: string;
  body: string | null;
};

type VersionInfo = {
  version: string;
};

type TestConnectionParams = {
  command: string;
  args: string[];
  env?: Record<string, string>;
};

type TestConnectionResult = {
  success: boolean;
  message: string;
};

type LocalSkillValidation = {
  valid: boolean;
  reason: string | null;
};

export function invokeWithTimeout<T>(
  command: string,
  args?: Record<string, unknown>,
  timeoutMs = 10000,
  timeoutMessage = "加载超时，请尝试重启应用"
): Promise<T> {
  let timeoutId: ReturnType<typeof setTimeout> | undefined;
  const timeoutPromise = new Promise<never>((_, reject) => {
    timeoutId = setTimeout(() => reject(new Error(timeoutMessage)), timeoutMs);
  });

  return Promise.race([invoke<T>(command, args), timeoutPromise]).finally(() => {
    if (timeoutId) {
      clearTimeout(timeoutId);
    }
  });
}

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

  async testConnection(params: TestConnectionParams): Promise<TestConnectionResult> {
    return invoke<TestConnectionResult>("test_mcp_connection", { params });
  },
};

// 应用配置 API
export const appApi = {
  async getVersion(): Promise<VersionInfo> {
    return invoke<VersionInfo>("get_version");
  },

  // 获取应用配置
  async getAppConfigs(): Promise<AppConfigInfo[]> {
    return invoke<AppConfigInfo[]>("get_app_configs");
  },

  async getLaunchPreferences(): Promise<LaunchPreferences> {
    return invoke<LaunchPreferences>("get_launch_preferences");
  },

  async setDefaultTerminal(terminalId: string): Promise<void> {
    return invoke("set_default_terminal", { terminalId });
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

  // 后台批量扫描所有已安装工具的版本号
  async scanAllToolVersions(): Promise<ToolInfo[]> {
    return invoke<ToolInfo[]>("scan_all_tool_versions");
  },

  // 安装工具
  async installTool(appType: string, methodIndex: number): Promise<void> {
    return invoke("install_tool", { appType, methodIndex });
  },

  // 更新工具
  async updateTool(appType: string): Promise<void> {
    return invoke("update_tool", { appType });
  },

  // 卸载工具
  async uninstallTool(appType: string): Promise<void> {
    return invoke("uninstall_tool", { appType });
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

export const updateApi = {
  async checkUpdate(): Promise<UpdateInfo> {
    return invoke<UpdateInfo>("check_update");
  },

  async installUpdate(): Promise<void> {
    return invoke("install_update");
  },
};

export const agentApi = {
  async openConfigFile(agentId: string): Promise<void> {
    return invoke("open_config_file", { agentId });
  },

  async syncAgentMcp(agentId: string, enabledApps: string[]): Promise<number> {
    return invoke<number>("sync_agent_mcp", { agentId, enabledApps });
  },

  async launchAgent(agentId: string): Promise<void> {
    return invoke("launch_agent", { agentId });
  },
};

export const skillsApi = {
  async getManagedSkills(): Promise<ManagedSkill[]> {
    return invokeWithTimeout<ManagedSkill[]>("get_managed_skills");
  },

  async getOnboardingPlan(): Promise<OnboardingPlan> {
    return invokeWithTimeout<OnboardingPlan>(
      "get_onboarding_plan",
      undefined,
      10000,
      "加载超时"
    );
  },

  async getReadme(skillName: string): Promise<string> {
    return invoke<string>("get_skill_readme", { skillName });
  },

  async deleteManagedSkill(skillId: string, skillName: string): Promise<void> {
    return invoke("delete_managed_skill", { skillId, skillName });
  },

  async syncToTool(params: {
    skillId: string;
    skillName: string;
    tool: string;
    sourcePath: string;
  }): Promise<void> {
    return invoke("sync_skill_to_tool", params);
  },

  async unsyncFromTool(skillName: string, tool: string): Promise<void> {
    return invoke("unsync_skill_from_tool", { skillName, tool });
  },

  async updateSkill(skillId: string): Promise<void> {
    return invoke("update_skill", { skillId });
  },

  async validateLocalSkill(path: string): Promise<LocalSkillValidation> {
    return invoke<LocalSkillValidation>("validate_local_skill", { path });
  },

  async getFeaturedSkills(): Promise<FeaturedSkillDto[]> {
    return invoke<FeaturedSkillDto[]>("get_featured_skills");
  },

  async listGitSkills(repoUrl: string): Promise<GitSkillCandidate[]> {
    return invoke<GitSkillCandidate[]>("list_git_skills", { repoUrl });
  },

  async installGit(repoUrl: string, name?: string): Promise<ManagedSkill> {
    return invoke<ManagedSkill>("install_git", { repoUrl, name });
  },

  async installGitSelection(params: {
    repoUrl: string;
    subpath: string;
    name?: string;
  }): Promise<ManagedSkill> {
    return invoke<ManagedSkill>("install_git_selection", params);
  },

  async installLocalSelection(params: {
    basePath: string;
    subpath: string;
    name?: string;
  }): Promise<ManagedSkill> {
    return invoke<ManagedSkill>("install_local_selection", params);
  },

  async importExistingSkill(sourcePath: string, name: string): Promise<ManagedSkill> {
    return invoke<ManagedSkill>("import_existing_skill", { sourcePath, name });
  },

  async renameSkill(params: {
    skillId: string;
    newName: string;
    newSourceRef: string | null;
  }): Promise<void> {
    return invoke("rename_skill", params);
  },

  async searchOnline(query: string): Promise<OnlineSkillDto[]> {
    return invoke<OnlineSkillDto[]>("search_skills_online", { query });
  },
};
