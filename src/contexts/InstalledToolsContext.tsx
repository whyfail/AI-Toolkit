import { createContext, useContext, useState, useEffect, useCallback, ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { toast } from "sonner";
import type { ToolId } from "@/lib/tools";

// 类型定义（与后端 InstalledToolsReport 对齐）
export interface AgentInfo {
  id: string;
  name: string;
  config_path: string;
  exists: boolean;
  mcp_count: number;
}

export interface ToolAdapter {
  id: ToolId;
  display_name: string;
  relative_skills_dir: string;
  relative_detect_dir: string;
}

export interface DetectedSkill {
  tool: ToolId;
  name: string;
  path: string;
  is_link: boolean;
  link_target: string | null;
}

export interface ToolStatus {
  tool: ToolAdapter;
  installed: boolean;
  skills: DetectedSkill[];
}

export interface InstalledToolsReport {
  agents: AgentInfo[];
  tool_statuses: ToolStatus[];
  detected_at: number;
}

interface InstalledToolsContextValue {
  report: InstalledToolsReport | null;
  isLoading: boolean;
  refresh: () => Promise<void>;
  markAgentUninstalled: (agentId: string) => void;
  markAgentInstalled: (agentId: string) => void;
  // 便捷访问
  installedAgents: AgentInfo[];
  toolStatuses: ToolStatus[];
}

const InstalledToolsContext = createContext<InstalledToolsContextValue | null>(null);

export function InstalledToolsProvider({ children }: { children: ReactNode }) {
  const [report, setReport] = useState<InstalledToolsReport | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  const loadReport = useCallback(async () => {
    try {
      // 10秒超时保护
      const timeoutPromise = new Promise<never>((_, reject) => {
        setTimeout(() => reject(new Error('加载超时，请尝试重启应用')), 10000);
      });
      const data = await Promise.race([
        invoke<InstalledToolsReport>("get_installed_tools"),
        timeoutPromise
      ]);
      setReport(data);
    } catch (err) {
      console.error("Failed to load installed tools:", err);
      toast.error(`检测工具失败: ${err}`);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const refresh = useCallback(async () => {
    setIsLoading(true);
    try {
      const data = await invoke<InstalledToolsReport>("refresh_installed_tools");
      setReport(data);
      toast.success("工具检测已刷新");
    } catch (err) {
      console.error("Failed to refresh installed tools:", err);
      toast.error(`刷新失败: ${err}`);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const markAgentUninstalled = useCallback((agentId: string) => {
    setReport(prev => {
      if (!prev) return prev;
      return {
        ...prev,
        agents: prev.agents.map(a =>
          a.id === agentId ? { ...a, exists: false } : a
        ),
      };
    });
  }, []);

  const markAgentInstalled = useCallback((agentId: string) => {
    setReport(prev => {
      if (!prev) return prev;
      return {
        ...prev,
        agents: prev.agents.map(a =>
          a.id === agentId ? { ...a, exists: true } : a
        ),
      };
    });
  }, []);

  useEffect(() => {
    loadReport();
  }, [loadReport]);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    const setupListener = async () => {
      unlisten = await listen<InstalledToolsReport>("installed-tools-updated", (event) => {
        setReport(event.payload);
        setIsLoading(false);
      });
    };

    setupListener().catch((err) => {
      console.error("Failed to listen installed-tools-updated:", err);
    });

    return () => {
      unlisten?.();
    };
  }, []);

  // 便捷访问
  const installedAgents = report?.agents.filter((a) => a.exists) || [];
  const toolStatuses = report?.tool_statuses || [];

  return (
    <InstalledToolsContext.Provider
      value={{
        report,
        isLoading,
        refresh,
        markAgentUninstalled,
        markAgentInstalled,
        installedAgents,
        toolStatuses,
      }}
    >
      {children}
    </InstalledToolsContext.Provider>
  );
}

export function useInstalledTools() {
  const context = useContext(InstalledToolsContext);
  if (!context) {
    throw new Error("useInstalledTools must be used within InstalledToolsProvider");
  }
  return context;
}
