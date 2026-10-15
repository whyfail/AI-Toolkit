import { invoke } from "@tauri-apps/api/core";
import type { McpServer, McpServerSpec, AppConfigInfo } from "@/types";
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

export type SkillsInstallPreferences = {
  selected: string;
  options: Array<{
    id: string;
    label: string;
    path: string;
  }>;
};

export type OnboardingChecklistItem = {
  id: string;
  title: string;
  done: boolean;
  detail: string;
};

export type BulkMcpServerInput = {
  id: string;
  name: string;
  server: McpServerSpec;
};

export type SkillConflictItem = {
  name: string;
  scope: string;
  message: string;
  paths: string[];
};

export type SkillSyncTarget = {
  tool: string;
  mode: string;
  status: string;
  target_path: string;
  synced_at?: number | null;
};

export type TraeMigrationPreview = {
  revision: string;
  entries: Array<{
    key: string;
    appId: string;
    sourcePath: string;
    targetPath: string;
    serverId: string;
    command: string | null;
    url: string | null;
    conflict: boolean;
  }>;
};

export type TraeMigrationSelection = {
  key: string;
  renameTo: string | null;
};

export type TraeMigrationOutcome = {
  imported: number;
  backupId: string;
  backupPath: string;
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

  async previewTraeMigration(): Promise<TraeMigrationPreview> {
    return invoke("preview_trae_migration");
  },

  async applyTraeMigration(revision: string, selections: TraeMigrationSelection[]): Promise<TraeMigrationOutcome> {
    return invoke("apply_trae_migration", { revision, selections });
  },

  async restoreTraeMigration(backupId: string): Promise<void> {
    return invoke("restore_trae_migration", { backupId });
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

  async getSkillsInstallPreferences(): Promise<SkillsInstallPreferences> {
    return invoke<SkillsInstallPreferences>("get_skills_install_preferences");
  },

  async setSkillsInstallLocation(locationId: string): Promise<void> {
    return invoke("set_skills_install_location_preference", { locationId });
  },

  // 从指定应用导入
  async importFromApp(appId: string): Promise<number> {
    return invoke<number>("import_mcp_from_app", { appId });
  },
};

// 本地 Agent 扫描 API
export const toolApi = {
  // 获取已安装工具的缓存数据（启动时检测一次）
  async getInstalledTools(): Promise<InstalledToolsReport> {
    return invoke<InstalledToolsReport>("get_installed_tools");
  },

  // 重新扫描本地已安装的 Agent
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

  async launchAgent(
    agentId: string,
    workingDir?: string,
    launchKind?: "auto" | "cli" | "desktop"
  ): Promise<void> {
    return invoke("launch_agent", { agentId, workingDir, launchKind });
  },
};

export const skillsApi = {
  async getManagedSkills(): Promise<ManagedSkill[]> {
    return invokeWithTimeout<ManagedSkill[]>("get_managed_skills");
  },

  async openSkillPath(path: string): Promise<void> {
    return invoke("open_skill_path", { path });
  },

  async detectNameConflicts(): Promise<SkillConflictItem[]> {
    return invoke<SkillConflictItem[]>("detect_skill_name_conflicts");
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
  }): Promise<SkillSyncTarget> {
    return invoke<SkillSyncTarget>("sync_skill_to_tool", params);
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

// 新手引导与 MCP 批量导入 API
export const enhancementApi = {
  async getOnboardingChecklist(): Promise<OnboardingChecklistItem[]> {
    return invoke<OnboardingChecklistItem[]>("get_onboarding_checklist");
  },

  async bulkImportMcpServers(params: {
    servers: BulkMcpServerInput[];
    apps: Record<string, boolean>;
    overwrite: boolean;
  }): Promise<{ imported: number; overwritten: number }> {
    return invoke("bulk_import_mcp_servers", params);
  },
};
