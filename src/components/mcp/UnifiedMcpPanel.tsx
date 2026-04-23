import { useMemo, useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
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
      await invoke("open_config_file", { agentId });
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
      const report = await invoke<{ agents: AgentInfo[] }>("refresh_installed_tools");
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
    <div className="flex flex-col h-full overflow-hidden">
      {/* 头部 */}
      <div className="px-4 sm:px-8 pt-6 sm:pt-8 pb-4 sm:pb-5 border-b border-[hsl(var(--border))] flex-shrink-0">
        <div className="flex flex-wrap items-center justify-between gap-3 mb-4 sm:mb-5">
          <div className="min-w-0">
            <h2 className="text-xl sm:text-2xl font-semibold tracking-tight truncate">
              MCP 服务器
            </h2>
            <p className="text-xs sm:text-sm text-[hsl(var(--muted-foreground))] mt-1">
              管理所有 AI CLI 工具的 MCP 配置
            </p>
          </div>
          <div className="flex gap-2 flex-shrink-0">
            <button
              onClick={handleScan}
              disabled={isScanning}
              className="inline-flex items-center gap-1.5 sm:gap-2 px-3 sm:px-4 py-2 sm:py-2.5 bg-[hsl(var(--secondary))] hover:brightness-[0.95] active:brightness-[0.9] text-[hsl(var(--secondary-foreground))] rounded-lg text-sm font-medium transition-all border border-[hsl(var(--border))] disabled:opacity-50"
            >
              <RefreshCw size={16} className={isScanning ? "animate-spin" : ""} />
              <span className="hidden sm:inline">扫描工具</span>
            </button>
            <button
              onClick={handleAdd}
              className="inline-flex items-center gap-1.5 sm:gap-2 px-3 sm:px-4 py-2 sm:py-2.5 bg-[hsl(var(--primary))] hover:brightness-[0.9] active:brightness-[0.85] text-white rounded-lg text-sm font-medium transition-all shadow-sm"
            >
              <Plus size={16} />
              <span className="hidden sm:inline">添加服务器</span>
            </button>
          </div>
        </div>

        {/* 搜索栏 */}
        <div className="relative mb-3 sm:mb-4">
          <Search
            size={16}
            className="absolute left-3 top-1/2 -translate-y-1/2 text-[hsl(var(--muted-foreground))]"
          />
          <input
            type="text"
            placeholder="搜索服务器..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full pl-10 pr-4 py-2 sm:py-2.5 bg-[hsl(var(--muted))] border border-[hsl(var(--border))] rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-[hsl(var(--ring))] focus:border-transparent transition-all"
          />
        </div>

        {/* 统计栏 */}
        <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs">
          <span className="font-medium text-[hsl(var(--muted-foreground))]">
            总计: {Object.keys(serversMap || {}).length}
          </span>
          {installedAgents.length > 0 && (
            <div className="flex flex-wrap gap-x-3 gap-y-1">
              {installedAgents.map((agent) => (
                <div
                  key={agent.id}
                  className="flex items-center gap-1.5 group cursor-pointer"
                  onClick={() => handleOpenConfig(agent.id)}
                  title="点击打开配置文件"
                >
                  <div
                    className={`w-2 h-2 rounded-full flex-shrink-0 ${APP_COLORS[agent.id as keyof typeof APP_COLORS]}`}
                  />
                  <span className="text-[hsl(var(--muted-foreground))] group-hover:text-[hsl(var(--foreground))] transition-colors flex items-center gap-1">
                    {agent.name}:{" "}
                    <span className="font-medium text-[hsl(var(--foreground))]">
                      {enabledCounts[agent.id] || 0}
                    </span>
                  </span>
                  <ExternalLink
                    size={10}
                    className="text-[hsl(var(--muted-foreground))] opacity-0 group-hover:opacity-100 transition-opacity"
                  />
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* 服务器列表 */}
      <div className="flex-1 overflow-y-auto px-3 sm:px-8 py-4 sm:py-5">
        {isLoading || isScanning ? (
          <div className="flex items-center justify-center h-64">
            <div className="text-[hsl(var(--muted-foreground))] flex items-center gap-2">
              <Loader2 size={18} className="animate-spin" />
              <span>{isScanning ? "正在扫描工具..." : "加载中..."}</span>
            </div>
          </div>
        ) : serverEntries.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-64 text-center">
            <div className="w-16 h-16 rounded-2xl bg-[hsl(var(--muted))] flex items-center justify-center mb-4">
              <Server
                size={28}
                className="text-[hsl(var(--muted-foreground))]"
              />
            </div>
            <h3 className="text-base font-medium mb-1">暂无服务器</h3>
            <p className="text-sm text-[hsl(var(--muted-foreground))]">
              点击"添加服务器"或"导入"开始配置
            </p>
          </div>
        ) : (
          <div className="space-y-2">
            {serverEntries.map(([id, server]) => (
              <McpServerRow
                key={id}
                id={id}
                server={server}
                installedAgents={installedAgents}
                onToggleApp={handleToggleApp}
                onEdit={handleEdit}
                onDelete={handleDelete}
              />
            ))}
          </div>
        )}
      </div>

      {/* 表单弹窗 */}
      {isFormOpen && (
        <McpFormModal
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
      )}

      {/* 新工具发现弹窗 */}
      {newAgents && (
        <NewAgentModal
          agents={newAgents}
          installedAgents={installedAgents}
          onClose={() => setNewAgents(null)}
          onSyncComplete={() => {
            setNewAgents(null);
            refetch();
          }}
        />
      )}

      {/* 删除确认弹窗 */}
      {deleteId && (
        <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 p-4 animate-in fade-in duration-200">
          <div className="bg-[hsl(var(--card))] rounded-xl w-full max-w-sm shadow-2xl border border-[hsl(var(--border))] overflow-hidden">
            <div className="px-6 py-5 border-b border-[hsl(var(--border))]">
              <h3 className="text-lg font-semibold">确认删除？</h3>
              <p className="text-sm text-[hsl(var(--muted-foreground))] mt-1 line-clamp-1">
                服务器: {deleteName || deleteId}
              </p>
            </div>
            <div className="px-6 py-4 flex justify-end gap-3">
              <button
                onClick={() => setDeleteId(null)}
                className="px-4 py-2 rounded-lg text-sm font-medium bg-[hsl(var(--secondary))] hover:brightness-[0.95] text-[hsl(var(--secondary-foreground))]"
              >
                取消
              </button>
              <button
                onClick={confirmDelete}
                disabled={deleteServerMutation.isPending}
                className="px-4 py-2 rounded-lg text-sm font-medium bg-red-500 hover:bg-red-600 text-white transition-colors disabled:opacity-50"
              >
                {deleteServerMutation.isPending ? '删除中...' : '删除'}
              </button>
            </div>
          </div>
        </div>
      )}
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
    <div className="group rounded-xl border border-[hsl(var(--border))] bg-[hsl(var(--card))] hover:border-[hsl(var(--ring))] transition-all duration-150 overflow-hidden">
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
          <button
            onClick={() => onEdit(id)}
            className="p-2 hover:bg-[hsl(var(--muted))] rounded-lg transition-colors"
            title="编辑"
          >
            <Edit3 size={14} className="text-[hsl(var(--muted-foreground))]" />
          </button>
          <button
            onClick={() => onDelete(id, server.name)}
            className="p-2 hover:bg-red-500/10 rounded-lg transition-colors"
            title="删除"
          >
            <Trash2 size={14} className="text-red-500" />
          </button>
        </div>
      </div>

      {/* 应用切换 */}
      <div className="px-3 sm:px-5 py-2.5 sm:py-3 bg-[hsl(var(--muted))/50] border-t border-[hsl(var(--border))]">
        <div className="flex items-center gap-2 text-xs text-[hsl(var(--muted-foreground))] mb-2">
          <span>已启用: {activeCount}/{installedAgents.length}</span>
        </div>
        <div className="flex flex-wrap gap-1.5 sm:gap-2">
          {installedAgents.map((agent) => (
            <label
              key={agent.id}
              className={`inline-flex items-center gap-1.5 px-2 sm:px-2.5 py-1 sm:py-1.5 rounded-lg cursor-pointer transition-all text-xs font-medium ${
                server.apps[agent.id]
                  ? "bg-[hsl(var(--primary))/10] text-[hsl(var(--primary))]"
                  : "bg-[hsl(var(--muted))] text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))]"
              }`}
            >
              <input
                type="checkbox"
                checked={!!server.apps[agent.id]}
                onChange={(e) => onToggleApp(id, agent.id, e.target.checked)}
                className="sr-only"
              />
              <div
                className={`w-2 h-2 rounded-full flex-shrink-0 ${
                  server.apps[agent.id]
                    ? APP_COLORS[agent.id as keyof typeof APP_COLORS]
                    : "bg-current opacity-40"
                }`}
              />
              <span>{agent.name}</span>
            </label>
          ))}
        </div>
      </div>

    </div>
  );
};

export default UnifiedMcpPanel;
