import React, { useState, useEffect, useRef } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { invoke } from "@tauri-apps/api/core";
import { toolApi } from "@/lib/api";
import { useInstalledTools } from "@/contexts/InstalledToolsContext";
import { isLaunchable } from "@/lib/tools";
import { open } from "@tauri-apps/plugin-shell";
import { Loader2, Download, RefreshCw, ExternalLink, CheckCircle, AlertCircle, Play, Trash2 } from "lucide-react";

function compareVersions(current: string, latest: string): boolean {
  const parse = (v: string) => v.replace(/[^0-9.]/g, '').split('.').map(n => parseInt(n, 10) || 0);
  const currentParts = parse(current);
  const latestParts = parse(latest);
  const len = Math.max(currentParts.length, latestParts.length);
  for (let i = 0; i < len; i++) {
    const a = currentParts[i] || 0;
    const b = latestParts[i] || 0;
    if (a < b) return true;
    if (a > b) return false;
  }
  return false;
}

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  message: string;
  confirmText: string;
  onConfirm: () => void;
  onCancel: () => void;
}

const ConfirmDialog: React.FC<ConfirmDialogProps> = ({
  open,
  title,
  message,
  confirmText,
  onConfirm,
  onCancel,
}) => {
  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm"
      onClick={onCancel}
    >
      <div
        className="w-full max-w-sm mx-4 rounded-2xl border border-[hsl(var(--border))] bg-[hsl(var(--card))] shadow-2xl overflow-hidden p-6"
        onClick={(e) => e.stopPropagation()}
      >
        <h3 className="text-base font-semibold mb-2">{title}</h3>
        <p className="text-sm text-[hsl(var(--muted-foreground))] mb-6">
          {message}
        </p>
        <div className="flex gap-3">
          <button
            onClick={onCancel}
            className="flex-1 px-4 py-2.5 rounded-lg border border-[hsl(var(--border))] text-sm font-medium hover:bg-[hsl(var(--muted))] transition-colors"
          >
            取消
          </button>
          <button
            onClick={onConfirm}
            className="flex-1 px-4 py-2.5 bg-[hsl(var(--primary))] text-white rounded-lg hover:opacity-90 transition-opacity text-sm font-medium"
          >
            {confirmText}
          </button>
        </div>
      </div>
    </div>
  );
};

const ToolCard: React.FC<{
  tool: {
    app_type: string;
    name: string;
    installed: boolean;
    version: string | null;
    latest_version: string | null;
    detected_method: string | null;
    methods: Array<{
      index: number;
      method_type: string;
      name: string;
      url?: string;
      command: string;
      needs_confirm: boolean;
    }>;
    homepage: string;
  };
  onInstall: (methodIndex: number, needsConfirm: boolean, command: string) => void;
  onUpdate: () => void;
  onScan: () => void;
  onLaunch: () => void;
  onDelete: () => void;
  installing: boolean;
  updating: boolean;
  scanning: boolean;
  deleting: boolean;
}> = ({ tool, onInstall, onUpdate, onScan, onLaunch, onDelete, installing, updating, scanning, deleting }) => {
  const [showMethods, setShowMethods] = useState(false);
  const hasUpdate = tool.installed && tool.version && tool.latest_version && compareVersions(tool.version, tool.latest_version);

  return (
    <div className="relative rounded-xl border border-[hsl(var(--border))] bg-[hsl(var(--card))] p-5 overflow-hidden">
      {updating && (
        <div className="absolute inset-0 bg-[hsl(var(--card))]/80 backdrop-blur-sm flex flex-col items-center justify-center gap-3 z-10">
          <Loader2 size={24} className="animate-spin text-[hsl(var(--primary))]" />
          <div className="text-center">
            <p className="text-sm font-medium">更新中...</p>
            <p className="text-xs text-[hsl(var(--muted-foreground))]">请稍候</p>
          </div>
        </div>
      )}
      {deleting && (
        <div className="absolute inset-0 bg-[hsl(var(--card))]/80 backdrop-blur-sm flex flex-col items-center justify-center gap-3 z-10">
          <Loader2 size={24} className="animate-spin text-red-500" />
          <div className="text-center">
            <p className="text-sm font-medium">卸载中...</p>
            <p className="text-xs text-[hsl(var(--muted-foreground))]">请稍候</p>
          </div>
        </div>
      )}
      <div className="flex items-start justify-between mb-3">
        <div className="flex items-center gap-3">
          <div
            className={`w-10 h-10 rounded-xl flex items-center justify-center ${
              tool.installed
                ? "bg-emerald-500/10"
                : "bg-[hsl(var(--muted))]"
            }`}
          >
            {tool.installed ? (
              <CheckCircle size={20} className="text-emerald-500" />
            ) : (
              <Download size={20} className="text-[hsl(var(--muted-foreground))]" />
            )}
          </div>
          <div>
            <h4 className="text-sm font-semibold">{tool.name}</h4>
            <p className="text-xs text-[hsl(var(--muted-foreground))]">
              {tool.installed ? (
                <span className="flex items-center gap-1.5 flex-wrap">
                  <span className="w-1.5 h-1.5 rounded-full bg-emerald-500" />
                  <span className="bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 px-1.5 py-0.5 rounded text-[10px] font-medium">
                    {tool.detected_method || tool.methods[0]?.name || "CLI"}
                  </span>
                  <span>{tool.version || ""}</span>
                  {hasUpdate && (
                    <span className="flex items-center gap-0.5 text-red-500" title={`有新版本 ${tool.latest_version}`}>
                      <AlertCircle size={12} />
                      <span>{tool.latest_version}</span>
                    </span>
                  )}
                  {!hasUpdate && tool.latest_version && (
                    <span className="text-[hsl(var(--muted-foreground))]">({tool.latest_version})</span>
                  )}
                </span>
              ) : (
                <span className="flex items-center gap-1">
                  <span className="w-1.5 h-1.5 rounded-full bg-[hsl(var(--muted-foreground))]" />
                  未安装
                </span>
              )}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-1">
          {tool.installed && tool.detected_method && tool.detected_method !== "下载安装" && (
            <button
              onClick={onDelete}
              className="p-1.5 rounded-lg hover:bg-red-500/10 transition-colors"
              title="卸载工具"
            >
              <Trash2
                size={14}
                className="text-[hsl(var(--muted-foreground))] hover:text-red-500"
              />
            </button>
          )}
          <button
            onClick={() => open(tool.homepage).catch(console.error)}
            className="p-1.5 rounded-lg hover:bg-[hsl(var(--muted))] transition-colors"
            title="访问官网"
          >
            <ExternalLink
              size={14}
              className="text-[hsl(var(--muted-foreground))]"
            />
          </button>
        </div>
      </div>

      <div className="flex gap-2">
        {tool.installed ? (
          <>
            {tool.methods.length > 0 &&
            tool.methods[0].method_type !== "download" ? (
              <>
                {isLaunchable(tool.app_type) && (
                  <button
                    onClick={onLaunch}
                    className="flex-1 flex items-center justify-center gap-2 px-3 py-2 bg-emerald-500 text-white rounded-lg hover:opacity-90 transition-all text-xs font-medium"
                    title="启动工具"
                  >
                    <Play size={14} />
                    启动
                  </button>
                )}
                <button
                  onClick={onScan}
                  disabled={scanning || updating}
                  className="flex-1 flex items-center justify-center gap-2 px-3 py-2 bg-[hsl(var(--primary))] text-white rounded-lg hover:opacity-90 disabled:opacity-50 transition-all text-xs font-medium"
                  title="扫描版本"
                >
                  <RefreshCw
                    size={14}
                    className={`text-white ${scanning ? 'animate-spin' : ''}`}
                  />
                  {scanning ? "扫描中..." : "扫描"}
                </button>
                <button
                  onClick={onUpdate}
                  disabled={updating}
                  className="flex-1 flex items-center justify-center gap-2 px-3 py-2 bg-[hsl(var(--primary))] text-white rounded-lg hover:opacity-90 disabled:opacity-50 transition-all text-xs font-medium"
                >
                  {updating ? (
                    <Loader2 size={14} className="animate-spin" />
                  ) : (
                    <RefreshCw size={14} />
                  )}
                  {updating ? "更新中..." : "更新"}
                </button>
              </>
            ) : (
              <button
                onClick={() => open(tool.homepage).catch(console.error)}
                className="flex-1 flex items-center justify-center gap-2 px-3 py-2 bg-[hsl(var(--primary))] text-white rounded-lg hover:opacity-90 transition-all text-xs font-medium"
              >
                <ExternalLink size={12} />
                访问官网
              </button>
            )}
          </>
        ) : (
          <>
            {(() => {
              const npmMethod = tool.methods.find(m => m.method_type === "npm");
              const downloadMethod = tool.methods.find(m => m.method_type === "download");
              const singleDownloadOnly = tool.methods.length === 1 && downloadMethod;

              if (singleDownloadOnly) {
                return (
                  <button
                    onClick={() => open(downloadMethod!.url || tool.homepage).catch(console.error)}
                    className="flex-1 flex items-center justify-center gap-2 px-3 py-2 bg-[hsl(var(--primary))] text-white rounded-lg hover:opacity-90 transition-all text-xs font-medium"
                  >
                    <ExternalLink size={12} />
                    下载安装
                  </button>
                );
              }

              if (npmMethod) {
                return (
                  <button
                    onClick={() => onInstall(npmMethod.index, npmMethod.needs_confirm, npmMethod.command)}
                    disabled={installing}
                    className="flex-1 flex items-center justify-center gap-2 px-3 py-2 bg-[hsl(var(--primary))] text-white rounded-lg hover:opacity-90 disabled:opacity-50 transition-all text-xs font-medium"
                  >
                    {installing ? (
                      <Loader2 size={12} className="animate-spin" />
                    ) : (
                      <Download size={12} />
                    )}
                    {installing ? "安装中..." : "安装"}
                  </button>
                );
              }

              return (
                <button
                  onClick={() => setShowMethods(!showMethods)}
                  disabled={installing}
                  className="flex-1 flex items-center justify-center gap-2 px-3 py-2 bg-[hsl(var(--primary))] text-white rounded-lg hover:opacity-90 disabled:opacity-50 transition-all text-xs font-medium"
                >
                  {installing ? (
                    <Loader2 size={12} className="animate-spin" />
                  ) : (
                    <Download size={12} />
                  )}
                  {installing ? "安装中..." : "安装"}
                </button>
              );
            })()}
          </>
        )}
      </div>

      {showMethods && !tool.installed && (
        <div className="mt-3 pt-3 border-t border-[hsl(var(--border))] space-y-2">
          <p className="text-xs text-[hsl(var(--muted-foreground))] mb-2">
            选择安装方式:
          </p>
          {tool.methods.map((method) => (
            <button
              key={method.index}
              onClick={() =>
                onInstall(method.index, method.needs_confirm, method.command)
              }
              disabled={installing}
              className="w-full flex items-center justify-between px-3 py-2 bg-[hsl(var(--muted))] rounded-lg hover:bg-[hsl(var(--muted))]/80 disabled:opacity-50 transition-colors text-xs"
            >
              <span className="font-medium">{method.name}</span>
              <code className="text-[hsl(var(--muted-foreground))] text-[10px] truncate max-w-[180px]">
                {method.command}
              </code>
            </button>
          ))}
        </div>
      )}
    </div>
  );
};

const ToolManagerPanel: React.FC = () => {
  const queryClient = useQueryClient();
  const [confirmDialog, setConfirmDialog] = useState<{
    open: boolean;
    title: string;
    message: string;
    confirmText: string;
    onConfirm: () => void;
  } | null>(null);
  const [updatingTool, setUpdatingTool] = useState<string | null>(null);
  const [installingTool, setInstallingTool] = useState<string | null>(null);
  const [scanningTool, setScanningTool] = useState<string | null>(null);
  const [deletingTool, setDeletingTool] = useState<string | null>(null);

  // 使用共享的工具检测上下文
  const { refresh: refreshInstalledTools, markAgentUninstalled, markAgentInstalled } = useInstalledTools();

  const { data: tools, isLoading, refetch, isFetching } = useQuery({
    queryKey: ["tool-infos"],
    queryFn: toolApi.getToolInfos,
    staleTime: 5 * 60 * 1000,
    gcTime: 30 * 60 * 1000,
  });

  // 首次加载完成后，自动后台扫描版本号
  const versionScanStarted = useRef(false);
  useEffect(() => {
    if (versionScanStarted.current || !tools) return;
    versionScanStarted.current = true;

    toolApi.scanAllToolVersions().then((scannedTools) => {
      queryClient.setQueryData(["tool-infos"], (old: any) => {
        if (!old) return old;
        const map = new Map(scannedTools.map((t: any) => [t.app_type, t]));
        return old.map((tool: any) => {
          const scanned = map.get(tool.app_type);
          if (!scanned) return tool;
          // 只更新版本号字段，保留其他字段不变
          return {
            ...tool,
            version: scanned.version ?? tool.version,
            latest_version: scanned.latest_version ?? tool.latest_version,
          };
        });
      });
    }).catch(() => {
      // 后台扫描失败静默处理，不影响用户使用
    });
  }, [tools, queryClient]);

  const installMutation = useMutation({
    mutationFn: async ({
      appType,
      methodIndex,
    }: {
      appType: string;
      methodIndex: number;
    }) => {
      await toolApi.installTool(appType, methodIndex);
      const updatedInfo = await toolApi.getToolInfo(appType);
      return updatedInfo;
    },
    onSuccess: (updatedInfo) => {
      toast.success("安装成功");
      setInstallingTool(null);
      queryClient.setQueryData(["tool-infos"], (old: any) => {
        if (!old) return old;
        return old.map((tool: any) =>
          tool.app_type === updatedInfo.app_type ? updatedInfo : tool
        );
      });
      markAgentInstalled(updatedInfo.app_type);
    },
    onError: (error: unknown) => {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(`安装失败: ${message}`);
      setInstallingTool(null);
    },
  });

  const updateMutation = useMutation({
    mutationFn: async (appType: string) => {
      setUpdatingTool(appType);
      await toolApi.updateTool(appType);
      await new Promise(resolve => setTimeout(resolve, 2000));
      const updatedInfo = await toolApi.getToolInfo(appType);
      return updatedInfo;
    },
    onSuccess: (updatedInfo) => {
      toast.success("更新成功");
      setUpdatingTool(null);
      queryClient.setQueryData(["tool-infos"], (old: any) => {
        if (!old) return old;
        return old.map((tool: any) =>
          tool.app_type === updatedInfo.app_type ? updatedInfo : tool
        );
      });
    },
    onError: (error: unknown) => {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(`更新失败: ${message}`);
      setUpdatingTool(null);
    },
  });

  const scanMutation = useMutation({
    mutationFn: async (appType: string) => {
      setScanningTool(appType);
      const scannedInfo = await toolApi.getToolInfo(appType);
      return scannedInfo;
    },
    onSuccess: (scannedInfo) => {
      setScanningTool(null);
      queryClient.setQueryData(["tool-infos"], (old: any) => {
        if (!old) return old;
        return old.map((tool: any) =>
          tool.app_type === scannedInfo.app_type ? scannedInfo : tool
        );
      });
    },
    onError: (error: unknown) => {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(`扫描失败: ${message}`);
      setScanningTool(null);
    },
  });

  const uninstallMutation = useMutation({
    mutationFn: async (appType: string) => {
      setDeletingTool(appType);
      await toolApi.uninstallTool(appType);
      const updatedInfo = await toolApi.getToolInfo(appType);
      return updatedInfo;
    },
    onSuccess: (updatedInfo) => {
      toast.success("卸载成功");
      setDeletingTool(null);
      queryClient.setQueryData(["tool-infos"], (old: any) => {
        if (!old) return old;
        return old.map((tool: any) =>
          tool.app_type === updatedInfo.app_type ? updatedInfo : tool
        );
      });
      markAgentUninstalled(updatedInfo.app_type);
    },
    onError: (error: unknown) => {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(`卸载失败: ${message}`);
      setDeletingTool(null);
    },
  });

  const handleDelete = (appType: string, toolName: string) => {
    setConfirmDialog({
      open: true,
      title: "确认卸载",
      message: `确定要卸载 ${toolName} 吗？此操作将从系统中移除该工具。`,
      confirmText: "确认卸载",
      onConfirm: () => {
        setConfirmDialog(null);
        uninstallMutation.mutate(appType);
      },
    });
  };

  const handleInstall = async (
    appType: string,
    methodIndex: number,
    needsConfirm: boolean,
    command: string
  ) => {
    if (needsConfirm) {
      setConfirmDialog({
        open: true,
        title: "确认安装",
        message: `即将执行以下命令:\n${command}\n\n这将运行一个来自互联网的安装脚本，请确保来源可靠。`,
        confirmText: "继续安装",
        onConfirm: () => {
          setConfirmDialog(null);
          setInstallingTool(appType);
          installMutation.mutate({ appType, methodIndex });
        },
      });
    } else {
      setInstallingTool(appType);
      installMutation.mutate({ appType, methodIndex });
    }
  };

  const handleUpdate = (appType: string) => {
    updateMutation.mutate(appType);
  };

  const handleLaunch = async (appType: string) => {
    try {
      await invoke("launch_agent", { agentId: appType });
    } catch (e) {
      toast.error(`启动失败: ${e}`);
    }
  };

  if (isLoading) {
    return (
      <div className="flex flex-col h-full overflow-hidden">
        <div className="px-8 pt-8 pb-6 border-b border-[hsl(var(--border))]">
          <div className="flex items-center justify-between">
            <div>
              <div className="h-9 w-40 bg-[hsl(var(--muted))] rounded-lg animate-pulse" />
              <div className="h-4 w-56 bg-[hsl(var(--muted))] rounded-md mt-2 animate-pulse flex items-center gap-2">
                <Loader2 size={14} className="animate-spin text-[hsl(var(--primary))]" />
                <span className="text-sm text-[hsl(var(--muted-foreground))]">正在扫描中...</span>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <div className="h-9 w-28 bg-[hsl(var(--muted))] rounded-lg animate-pulse" />
              <div className="h-9 w-9 bg-[hsl(var(--muted))] rounded-lg animate-pulse" />
            </div>
          </div>
        </div>
        <div className="flex-1 overflow-y-auto px-8 py-6">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {[...Array(6)].map((_, i) => (
              <div
                key={i}
                className="rounded-xl border border-[hsl(var(--border))] bg-[hsl(var(--card))] p-5 animate-pulse"
                style={{ animationDelay: `${i * 100}ms` }}
              >
                <div className="flex items-start justify-between mb-3">
                  <div className="flex items-center gap-3">
                    <div className="w-10 h-10 rounded-xl bg-[hsl(var(--muted))]" />
                    <div>
                      <div className="h-4 w-20 bg-[hsl(var(--muted))] rounded-md mb-1" />
                      <div className="h-3 w-28 bg-[hsl(var(--muted))] rounded-md" />
                    </div>
                  </div>
                  <div className="w-6 h-6 bg-[hsl(var(--muted))] rounded-md" />
                </div>
                <div className="flex gap-2">
                  <div className="flex-1 h-8 bg-[hsl(var(--muted))] rounded-lg" />
                  <div className="flex-1 h-8 bg-[hsl(var(--muted))] rounded-lg" />
                  <div className="flex-1 h-8 bg-[hsl(var(--muted))] rounded-lg" />
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full overflow-hidden">
      <div className="px-8 pt-8 pb-6 border-b border-[hsl(var(--border))]">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-2xl font-semibold tracking-tight">工具管理</h2>
            <p className="text-sm text-[hsl(var(--muted-foreground))] mt-1 flex items-center gap-2">
              {isFetching ? (
                <>
                  <Loader2 size={14} className="animate-spin" />
                  <span>重新扫描工具中...</span>
                </>
              ) : (
                "安装或更新 AI 编程工具"
              )}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={async () => {
                // 调用全局刷新，刷新后所有模块共享结果
                await refreshInstalledTools();
                // 同时刷新工具详情
                refetch();
              }}
              disabled={isFetching}
              className="p-2 rounded-lg hover:bg-[hsl(var(--muted))] transition-colors disabled:opacity-50"
              title="刷新"
            >
              <RefreshCw size={18} className={`text-[hsl(var(--muted-foreground))] ${isFetching ? 'animate-spin' : ''}`} />
            </button>
          </div>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-8 py-6">
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {tools?.slice().sort((a, b) => a.name.localeCompare(b.name)).map((tool) => (
            <ToolCard
              key={tool.app_type}
              tool={tool}
              onInstall={(methodIndex, needsConfirm, command) =>
                handleInstall(tool.app_type, methodIndex, needsConfirm, command)
              }
              onUpdate={() => handleUpdate(tool.app_type)}
              onScan={() => scanMutation.mutate(tool.app_type)}
              onLaunch={() => handleLaunch(tool.app_type)}
              onDelete={() => handleDelete(tool.app_type, tool.name)}
              installing={installingTool === tool.app_type}
              updating={updatingTool === tool.app_type}
              scanning={scanningTool === tool.app_type}
              deleting={deletingTool === tool.app_type}
            />
          ))}
        </div>
      </div>

      <ConfirmDialog
        open={confirmDialog?.open || false}
        title={confirmDialog?.title || ""}
        message={confirmDialog?.message || ""}
        confirmText={confirmDialog?.confirmText || "确认"}
        onConfirm={confirmDialog?.onConfirm || (() => {})}
        onCancel={() => setConfirmDialog(null)}
      />
    </div>
  );
};

export default ToolManagerPanel;
