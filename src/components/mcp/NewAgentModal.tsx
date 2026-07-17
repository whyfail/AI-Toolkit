import { useState } from "react";
import { agentApi } from "@/lib/api";
import {
  X,
  Check,
  Plus,
  Terminal,
  ArrowRight,
  Loader2,
} from "lucide-react";
import { Pressable } from "@/components/ui/Pressable";
import { Modal } from "@/components/ui/Modal";
import { MotionList, MotionListItem } from "@/components/ui/MotionList";

interface AgentInfo {
  id: string;
  name: string;
  config_path: string;
  exists: boolean;
  mcp_count: number;
}

interface NewAgentModalProps {
  open: boolean;
  agents: AgentInfo[];
  installedAgents: AgentInfo[];
  onClose: () => void;
  onSyncComplete: () => void;
}

const NewAgentModal: React.FC<NewAgentModalProps> = ({
  open,
  agents,
  installedAgents,
  onClose,
  onSyncComplete,
}) => {
  const [selectedAgents, setSelectedAgents] = useState<Record<string, boolean>>(
    () => {
      const initial: Record<string, boolean> = {};
      agents.forEach((a) => (initial[a.id] = true));
      return initial;
    }
  );
  const [syncing, setSyncing] = useState(false);
  const [syncedCount, setSyncedCount] = useState(0);
  const [selectedApps, setSelectedApps] = useState<Record<string, boolean>>(
    () => {
      const initial: Record<string, boolean> = {};
      installedAgents.forEach((a) => (initial[a.id] = true));
      return initial;
    }
  );

  const toggleAgent = (id: string) => {
    setSelectedAgents((prev) => ({ ...prev, [id]: !prev[id] }));
  };

  const toggleApp = (id: string) => {
    setSelectedApps((prev) => ({ ...prev, [id]: !prev[id] }));
  };

  const handleSync = async () => {
    setSyncing(true);
    setSyncedCount(0);

    const selectedAgentIds = Object.entries(selectedAgents)
      .filter(([_, v]) => v)
      .map(([id]) => id);

    const enabledApps = Object.entries(selectedApps)
      .filter(([_, v]) => v)
      .map(([id]) => id);

    const results = await Promise.allSettled(
      selectedAgentIds.map((agentId) =>
        agentApi.syncAgentMcp(agentId, enabledApps)
      )
    );

    const total = results.reduce((sum, result, index) => {
      if (result.status === "fulfilled") {
        return sum + result.value;
      }
      console.error(`Failed to sync ${selectedAgentIds[index]}:`, result.reason);
      return sum;
    }, 0);
    setSyncedCount(total);

    setSyncing(false);
    onSyncComplete();
  };

  return (
    <Modal open={open} onClose={onClose} size="md" zIndex={60}>
      <div className="flex max-h-[90vh] w-full flex-col overflow-hidden sm:max-h-[85vh]">
        {/* 头部 */}
        <div className="flex flex-shrink-0 items-start justify-between gap-3 border-b border-white/50 px-4 py-4 dark:border-white/10 sm:px-6 sm:py-5">
          <div className="flex items-center gap-2 sm:gap-3 min-w-0">
            <div className="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-xl bg-emerald-500/10 sm:h-10 sm:w-10">
              <Plus size={16} className="text-emerald-500 sm:hidden" />
              <Plus size={20} className="text-emerald-500 hidden sm:block" />
            </div>
            <div className="min-w-0">
              <h2 className="text-base sm:text-lg font-semibold truncate">发现新的 AI 工具</h2>
              <p className="mt-0.5 text-xs text-slate-500 dark:text-slate-400">
                检测到 {agents.length} 个新安装的工具，是否同步其 MCP 配置？
              </p>
            </div>
          </div>
          <Pressable
            onClick={onClose}
            variant="icon"
            className="glass-icon-button flex-shrink-0"
            aria-label="关闭"
          >
            <X size={18} />
          </Pressable>
        </div>

        {/* 检测到的工具列表 */}
        <div className="min-h-0 flex-1 overflow-y-auto border-b border-white/50 px-4 py-4 dark:border-white/10 sm:px-6">
          <h3 className="mb-3 text-xs font-semibold uppercase tracking-wider text-slate-500 dark:text-slate-400">
            检测到的工具
          </h3>
          <MotionList className="space-y-2">
            {agents.map((agent) => (
              <MotionListItem key={agent.id}>
              <Pressable
                onClick={() => toggleAgent(agent.id)}
                aria-pressed={selectedAgents[agent.id]}
                className={`flex w-full items-center gap-3 rounded-xl border px-3 py-3 text-left transition-colors duration-200 ease-out ${
                  selectedAgents[agent.id]
                    ? "border-blue-200/70 bg-blue-500/10 dark:border-sky-300/20"
                    : "border-white/55 bg-white/50 dark:border-white/10 dark:bg-white/8"
                }`}
              >
                <div
                  className={`w-5 h-5 rounded-md flex items-center justify-center flex-shrink-0 transition-colors ${
                    selectedAgents[agent.id]
                      ? "bg-blue-600"
                      : "border border-white/50 bg-white/50 dark:border-white/10 dark:bg-white/8"
                  }`}
                >
                  {selectedAgents[agent.id] && (
                    <Check size={12} className="text-white" />
                  )}
                </div>
                <div className="flex items-center gap-2 flex-1 min-w-0">
                  <Terminal size={16} className="flex-shrink-0 text-slate-400" />
                  <span className="text-sm font-medium truncate">
                    {agent.name}
                  </span>
                </div>
                <div className="flex items-center gap-2 flex-shrink-0">
                  <span className="text-xs text-slate-500 dark:text-slate-400">
                    {agent.mcp_count} 个 MCP
                  </span>
                  <ArrowRight size={14} className="text-slate-400" />
                </div>
              </Pressable>
              </MotionListItem>
            ))}
          </MotionList>
        </div>

        {/* 集成到的工具 */}
        <div className="flex-shrink-0 border-b border-white/50 px-4 py-4 dark:border-white/10 sm:px-6">
          <h3 className="mb-3 text-xs font-semibold uppercase tracking-wider text-slate-500 dark:text-slate-400">
            同步到以下工具
          </h3>
          <div className="flex flex-wrap gap-1.5 sm:gap-2">
            {installedAgents.map((agent) => (
              <Pressable
                key={agent.id}
                onClick={() => toggleApp(agent.id)}
                aria-pressed={selectedApps[agent.id]}
                className={`rounded-full border px-2.5 py-1.5 text-xs font-semibold transition-colors duration-200 ease-out sm:px-3 ${
                  selectedApps[agent.id]
                    ? "border-blue-500 bg-blue-600 text-white"
                    : "border-white/55 bg-white/50 text-slate-500 hover:text-slate-950 dark:border-white/10 dark:bg-white/8 dark:text-slate-400 dark:hover:text-white"
                }`}
              >
                {agent.name}
              </Pressable>
            ))}
          </div>
        </div>

        {/* 底部按钮 */}
        <div className="flex flex-shrink-0 flex-wrap items-center justify-between gap-2 bg-white/25 px-4 py-3 dark:bg-white/5 sm:gap-3 sm:px-6 sm:py-4">
          <div className="order-2 w-full text-xs text-slate-500 dark:text-slate-400 sm:order-1 sm:w-auto sm:text-sm">
            {syncing ? (
              <span className="flex items-center gap-2">
                <Loader2 size={14} className="animate-spin" />
                已同步 {syncedCount} 个服务器...
              </span>
            ) : (
              `将同步 ${
                Object.values(selectedAgents).filter(Boolean).length
              } 个工具的配置`
            )}
          </div>
          <div className="flex gap-2 order-1 sm:order-2 flex-shrink-0">
            <Pressable
              onClick={onClose}
              className="glass-secondary-button"
            >
              稍后同步
            </Pressable>
            <Pressable
              onClick={handleSync}
              disabled={
                syncing ||
                !Object.values(selectedAgents).some(Boolean) ||
                !Object.values(selectedApps).some(Boolean)
              }
              className="glass-primary-button"
            >
              {syncing ? "同步中..." : "同步 MCP 配置"}
            </Pressable>
          </div>
        </div>
      </div>
    </Modal>
  );
};

export default NewAgentModal;
