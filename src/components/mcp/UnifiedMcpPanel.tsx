import { useMemo, useState, useEffect } from "react";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import {
  Server,
  Plus,
  Edit3,
  Trash2,
  Search,
  RefreshCw,
  ExternalLink,
  Loader2,
} from "lucide-react";
import {
  useAllMcpServers,
  useToggleMcpApp,
  useDeleteMcpServer,
} from "@/hooks/useMcp";
import { useInstalledTools, AgentInfo } from "@/contexts/InstalledToolsContext";
import type { McpServer } from "@/types";
import { APP_COLORS } from "@/lib/tools";
import { agentApi, toolApi } from "@/lib/api";
import { Pressable } from "@/components/ui/Pressable";
import { Modal } from "@/components/ui/Modal";
import { MotionList, MotionListItem } from "@/components/ui/MotionList";
import McpFormModal from "./McpFormModal";
import NewAgentModal from "./NewAgentModal";

const UnifiedMcpPanel: React.FC = () => {
  const [isFormOpen, setIsFormOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [newAgents, setNewAgents] = useState<AgentInfo[] | null>(null);
  const [isScanning, setIsScanning] = useState(false);
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [deleteName, setDeleteName] = useState<string>('');

  // 使用共享的工具检测上下文
  const { installedAgents } = useInstalledTools();

  const { data: serversMap, isLoading, refetch } = useAllMcpServers();
  const toggleAppMutation = useToggleMcpApp();
  const deleteServerMutation = useDeleteMcpServer();

  // 打开配置文件
  const handleOpenConfig = async (agentId: string) => {
    try {
      await agentApi.openConfigFile(agentId);
    } catch (e) {
      console.error(`Failed to open config for ${agentId}:`, e);
    }
  };

  // 监听新工具事件
  useEffect(() => {
    let unlisten: UnlistenFn;
    const setupListener = async () => {
      unlisten = await listen<AgentInfo[]>("agents-detected", (event) => {
        if (event.payload.length > 0) {
          setNewAgents(event.payload);
        }
      });
    };
    setupListener();
    return () => {
      unlisten?.();
    };
  }, []);

  // 手动扫描（使用全局刷新，刷新后所有模块共享）
  const handleScan = async () => {
    setIsScanning(true);
    try {
      // 调用全局刷新
      const report = await toolApi.refreshInstalledTools();
      const existing = report.agents.filter((a) => a.exists);
      if (existing.length > 0) {
        setNewAgents(existing);
      }
    } catch (e) {
      console.error("Failed to detect agents:", e);
    }
    setIsScanning(false);
  };

  const serverEntries = useMemo((): Array<[string, McpServer]> => {
    if (!serversMap) return [];
    let entries = Object.entries(serversMap);
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      entries = entries.filter(
        ([id, s]) =>
          id.toLowerCase().includes(q) ||
          s.name.toLowerCase().includes(q) ||
          s.description?.toLowerCase().includes(q) ||
          s.tags?.some((t) => t.toLowerCase().includes(q))
      );
    }
    return entries;
  }, [serversMap, searchQuery]);

  const enabledCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    installedAgents.forEach((a) => (counts[a.id] = 0));
    Object.values(serversMap || {}).forEach((server) => {
      Object.entries(server.apps).forEach(([appId, enabled]) => {
        if (enabled && counts[appId] !== undefined) {
          counts[appId]++;
        }
      });
    });
    return counts;
  }, [serversMap, installedAgents]);

  const handleToggleApp = async (
    serverId: string,
    app: string,
    enabled: boolean
  ) => {
    try {
      await toggleAppMutation.mutateAsync({ serverId, app, enabled });
    } catch (error) {
      console.error("Failed to toggle app:", error);
    }
  };

  const handleEdit = (id: string) => {
    setEditingId(id);
    setIsFormOpen(true);
  };

  const handleAdd = () => {
    setEditingId(null);
    setIsFormOpen(true);
  };

  const handleDelete = (id: string, name: string) => {
    setDeleteId(id);
    setDeleteName(name);
  };

  const confirmDelete = async () => {
    if (deleteId) {
      try {
        await deleteServerMutation.mutateAsync(deleteId);
        setDeleteId(null);
      } catch (error) {
        console.error("Failed to delete:", error);
      }
    }
  };

  return (
    <div className="glass-app flex h-full flex-col overflow-hidden">
      {/* 头部 */}
      <div className="glass-header flex-shrink-0">
        <div className="flex flex-wrap items-center justify-between gap-3 mb-4 sm:mb-5">
          <div className="min-w-0">
            <div className="glass-kicker">
              <Server size={13} />
              MCP
            </div>
            <h2 className="mt-3 truncate text-display">
              MCP 服务器
            </h2>
            <p className="mt-2 text-caption text-slate-500 dark:text-slate-400 sm:text-body">
              管理所有 AI CLI 工具的 MCP 配置
            </p>
          </div>
          <div className="flex gap-2 flex-shrink-0">
            <Pressable
              onClick={handleScan}
              disabled={isScanning}
              className="glass-secondary-button"
            >
              <RefreshCw size={16} className={isScanning ? "animate-spin" : ""} />
              <span className="hidden sm:inline">扫描工具</span>
            </Pressable>
            <Pressable
              onClick={handleAdd}
              className="glass-primary-button"
            >
              <Plus size={16} />
              <span className="hidden sm:inline">添加服务器</span>
            </Pressable>
          </div>
        </div>

        {/* 搜索栏 */}
        <div className="relative mb-3 sm:mb-4">
          <Search
            size={16}
            className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400"
          />
          <input
            type="text"
            placeholder="搜索服务器..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="glass-input w-full px-4 py-2 pl-10 text-sm sm:py-2.5"
          />
        </div>

        {/* 统计栏 */}
        <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs">
          <span className="glass-pill">
            总计: {Object.keys(serversMap || {}).length}
          </span>
          {installedAgents.length > 0 && (
            <div className="flex flex-wrap gap-x-3 gap-y-1">
              {installedAgents.map((agent) => (
                <button
                  type="button"
                  key={agent.id}
                  className="flex items-center gap-1.5 group cursor-pointer"
                  onClick={() => handleOpenConfig(agent.id)}
                  title="点击打开配置文件"
                >
                  <div
                    className={`h-2 w-2 flex-shrink-0 rounded-full ${APP_COLORS[agent.id as keyof typeof APP_COLORS]}`}
                  />
                  <span className="flex items-center gap-1 text-slate-500 transition-colors group-hover:text-slate-950 dark:text-slate-400 dark:group-hover:text-white">
                    {agent.name}:{" "}
                    <span className="font-semibold text-slate-950 dark:text-white">
                      {enabledCounts[agent.id] || 0}
                    </span>
                  </span>
                  <ExternalLink
                    size={10}
                    className="text-slate-400 opacity-0 transition-opacity group-hover:opacity-100"
                  />
                </button>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* 服务器列表 */}
      <div className="glass-content px-3 sm:px-8">
        {isLoading || isScanning ? (
          <div className="flex items-center justify-center h-64">
            <div className="glass-pill flex items-center gap-2">
              <Loader2 size={18} className="animate-spin" />
              <span>{isScanning ? "正在扫描工具..." : "加载中..."}</span>
            </div>
          </div>
        ) : serverEntries.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-64 text-center">
            <div className="glass-empty-icon mb-4">
              <Server
                size={28}
              />
            </div>
            <h3 className="text-base font-medium mb-1">暂无服务器</h3>
            <p className="text-sm text-slate-500 dark:text-slate-400">
              点击"添加服务器"或"导入"开始配置
            </p>
          </div>
        ) : (
          <MotionList className="space-y-2.5">
            {serverEntries.map(([id, server]) => (
              <MotionListItem key={id}>
                <McpServerRow
                  id={id}
                  server={server}
                  installedAgents={installedAgents}
                  onToggleApp={handleToggleApp}
                  onEdit={handleEdit}
                  onDelete={handleDelete}
                />
              </MotionListItem>
            ))}
          </MotionList>
        )}
      </div>

      {/* 表单弹窗 */}
      <McpFormModal
        open={isFormOpen}
        editingId={editingId || undefined}
        initialData={
          editingId && serversMap ? serversMap[editingId] : undefined
        }
        installedAgents={installedAgents}
        onClose={() => {
          setIsFormOpen(false);
          setEditingId(null);
        }}
      />

      {/* 新工具发现弹窗 */}
      <NewAgentModal
        open={!!newAgents}
        agents={newAgents ?? []}
        installedAgents={installedAgents}
        onClose={() => setNewAgents(null)}
        onSyncComplete={() => {
          setNewAgents(null);
          refetch();
        }}
      />

      {/* 删除确认弹窗 */}
      <Modal
        open={!!deleteId}
        onClose={() => setDeleteId(null)}
        size="sm"
      >
        <div className="border-b border-white/50 px-6 py-5 dark:border-white/10">
          <h3 className="text-lg font-semibold">确认删除？</h3>
          <p className="mt-1 line-clamp-1 text-sm text-slate-500 dark:text-slate-400">
            服务器: {deleteName || deleteId}
          </p>
        </div>
        <div className="px-6 py-4 flex justify-end gap-3">
          <Pressable
            onClick={() => setDeleteId(null)}
            className="glass-secondary-button"
          >
            取消
          </Pressable>
          <Pressable
            onClick={confirmDelete}
            disabled={deleteServerMutation.isPending}
            className="glass-danger-button"
          >
            {deleteServerMutation.isPending ? '删除中...' : '删除'}
          </Pressable>
        </div>
      </Modal>
    </div>
  );
}

// 服务器行组件
interface McpServerRowProps {
  id: string;
  server: McpServer;
  installedAgents: AgentInfo[];
  onToggleApp: (serverId: string, app: string, enabled: boolean) => void;
  onEdit: (id: string) => void;
  onDelete: (id: string, name: string) => void;
}

const McpServerRow: React.FC<McpServerRowProps> = ({
  id,
  server,
  installedAgents,
  onToggleApp,
  onEdit,
  onDelete,
}) => {
  const activeCount = installedAgents.filter(
    (a) => server.apps[a.id]
  ).length;

  return (
    <div className="glass-card group overflow-hidden">
      {/* 头部 */}
      <div className="px-3 sm:px-5 py-3 sm:py-4 flex items-start justify-between gap-3">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-1.5 sm:gap-2 mb-1">
            <h3 className="text-sm font-semibold truncate">{server.name}</h3>
          </div>
          {server.description && (
            <p className="text-xs text-[hsl(var(--muted-foreground))] line-clamp-2">
              {server.description}
            </p>
          )}
        </div>

        <div className="flex items-center gap-1 sm:opacity-0 sm:group-hover:opacity-100 transition-opacity flex-shrink-0">
          <Pressable
            onClick={() => onEdit(id)}
            variant="icon"
            className="glass-icon-button"
            title="编辑"
          >
            <Edit3 size={14} />
          </Pressable>
          <Pressable
            onClick={() => onDelete(id, server.name)}
            variant="icon"
            className="glass-icon-button hover:text-red-500"
            title="删除"
          >
            <Trash2 size={14} className="text-red-500" />
          </Pressable>
        </div>
      </div>

      {/* 应用切换 */}
      <div className="border-t border-white/50 bg-white/25 px-3 py-2.5 dark:border-white/10 dark:bg-white/5 sm:px-5 sm:py-3">
        <div className="mb-2 flex items-center gap-2 text-xs text-slate-500 dark:text-slate-400">
          <span>已启用: {activeCount}/{installedAgents.length}</span>
        </div>
        <div className="flex flex-wrap gap-1.5 sm:gap-2">
          {installedAgents.map((agent) => {
            const enabled = !!server.apps[agent.id];
            return (
              <Pressable
                key={agent.id}
                onClick={() => onToggleApp(id, agent.id, !enabled)}
                aria-pressed={enabled}
                className={`inline-flex cursor-pointer items-center gap-1.5 rounded-full border px-2 py-1 text-xs font-semibold transition-colors duration-200 ease-out sm:px-2.5 sm:py-1.5 ${
                  enabled
                    ? "border-blue-200/70 bg-blue-500/10 text-blue-700 dark:border-sky-300/20 dark:text-sky-300"
                    : "border-white/55 bg-white/50 text-slate-500 hover:text-slate-950 dark:border-white/10 dark:bg-white/8 dark:text-slate-400 dark:hover:text-white"
                }`}
              >
                <span
                  className={`w-2 h-2 rounded-full flex-shrink-0 ${
                    enabled
                      ? APP_COLORS[agent.id as keyof typeof APP_COLORS]
                      : "bg-current opacity-40"
                  }`}
                />
                <span>{agent.name}</span>
              </Pressable>
            );
          })}
        </div>
      </div>

    </div>
  );
};

export default UnifiedMcpPanel;
