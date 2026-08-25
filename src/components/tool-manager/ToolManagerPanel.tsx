import React, { useState, useEffect, useRef } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { agentApi, enhancementApi, toolApi } from "@/lib/api";
import { useInstalledTools } from "@/contexts/InstalledToolsContext";
import { getToolMeta, isLaunchable } from "@/lib/tools";
import { open as openUrl } from "@tauri-apps/plugin-shell";
import { Loader2, Download, RefreshCw, ExternalLink, CheckCircle, AlertCircle, Play, Trash2, BookOpen, Package, Settings, CheckSquare, Square } from "lucide-react";
import { Pressable } from "@/components/ui/Pressable";
import { Modal } from "@/components/ui/Modal";
import { MotionList, MotionListItem } from "@/components/ui/MotionList";

const glassSurface =
  "border border-white/60 bg-white/70 shadow-[0_18px_50px_rgba(15,23,42,0.08)] backdrop-blur-2xl dark:border-white/10 dark:bg-slate-950/55 dark:shadow-[0_18px_60px_rgba(0,0,0,0.35)]";
const iconButton =
  "flex h-9 w-9 items-center justify-center rounded-xl border border-transparent text-slate-500 transition-all duration-200 hover:border-white/70 hover:bg-white/70 hover:text-slate-950 hover:shadow-sm focus:outline-none focus:ring-2 focus:ring-[hsl(var(--ring))]/40 dark:text-slate-400 dark:hover:border-white/10 dark:hover:bg-white/10 dark:hover:text-white";
const primaryButton =
  "flex min-h-10 flex-1 items-center justify-center gap-2 rounded-xl bg-gradient-to-r from-blue-600 to-sky-500 px-3 py-2 text-xs font-semibold text-white shadow-[0_10px_24px_rgba(37,99,235,0.22)] transition-all duration-200 hover:-translate-y-0.5 hover:shadow-[0_14px_30px_rgba(37,99,235,0.28)] disabled:translate-y-0 disabled:opacity-50 disabled:shadow-none";
const secondaryButton =
  "flex min-h-10 flex-1 items-center justify-center gap-2 rounded-xl border border-white/60 bg-white/65 px-3 py-2 text-xs font-semibold text-slate-700 shadow-sm backdrop-blur-xl transition-all duration-200 hover:-translate-y-0.5 hover:bg-white/85 hover:text-slate-950 disabled:translate-y-0 disabled:opacity-50 dark:border-white/10 dark:bg-white/8 dark:text-slate-200 dark:hover:bg-white/12";
const toolVisibilityStorageKey = "ai-toolkit-tool-manager-hidden-tools";

const parseVersion = (v: string) => v.replace(/[^0-9.]/g, '').split('.').map(n => parseInt(n, 10) || 0);

function compareVersions(current: string, latest: string): boolean {
  const currentParts = parseVersion(current);
  const latestParts = parseVersion(latest);
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
  return (
    <Modal open={open} onClose={onCancel} size="sm">
      <div role="dialog" aria-modal="true" className="p-6">
        <h3 className="text-base font-semibold mb-2">{title}</h3>
        <p className="text-sm text-[hsl(var(--muted-foreground))] mb-6 whitespace-pre-wrap">
          {message}
        </p>
        <div className="flex gap-3">
          <Pressable
            onClick={onCancel}
            className="glass-secondary-button flex-1"
          >
            取消
          </Pressable>
          <Pressable
            onClick={onConfirm}
            className="glass-primary-button flex-1"
          >
            {confirmText}
          </Pressable>
        </div>
      </div>
    </Modal>
  );
};

const ToolCard: React.FC<{
  tool: {
    app_type: string;
    name: string;
    installed: boolean;
    has_cli: boolean;
    has_desktop_app: boolean;
    version: string | null;
    desktop_version: string | null;
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
    edition_unknown: boolean;
  };
  onInstall: (methodIndex: number, needsConfirm: boolean, command: string) => void;
  onUpdate: (updateKind: "cli" | "desktop") => void;
  onScan: () => void;
  onLaunch: (launchKind: "cli" | "desktop") => void;
  onDelete: () => void;
  onOpenDownload: () => void;
  switchingEdition: boolean;
  installing: boolean;
  updating: boolean;
  scanning: boolean;
  deleting: boolean;
}> = ({ tool, onInstall, onUpdate, onScan, onLaunch, onDelete, onOpenDownload, switchingEdition, installing, updating, scanning, deleting }) => {
  const [showMethods, setShowMethods] = useState(false);
  const installedVersion = tool.desktop_version || tool.version;
  const hasUpdate = Boolean(tool.installed && installedVersion && tool.latest_version && compareVersions(installedVersion, tool.latest_version));
  const isWorkBuddy = tool.app_type === "workbuddy" || tool.app_type === "workbuddy-cn";
  const docsUrl = getToolMeta(tool.app_type)?.docsUrl;
  const installBadgeLabel = tool.has_cli && tool.has_desktop_app
    ? "CLI + 桌面端"
    : tool.has_cli
      ? "CLI"
      : tool.has_desktop_app
        ? "桌面端"
        : "已安装";

  return (
    <div className={`group relative overflow-hidden rounded-2xl p-5 transition-all duration-300 hover:-translate-y-1 hover:border-white/80 hover:shadow-[0_24px_70px_rgba(15,23,42,0.13)] dark:hover:border-white/20 ${glassSurface}`}>
      <div className="pointer-events-none absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-white/90 to-transparent dark:via-white/25" />
      {updating && (
        <div className="absolute inset-0 z-10 flex flex-col items-center justify-center gap-3 bg-white/75 backdrop-blur-xl dark:bg-slate-950/70">
          <Loader2 size={24} className="animate-spin text-blue-600 dark:text-sky-400" />
          <div className="text-center">
            <p className="text-sm font-medium">更新中...</p>
            <p className="text-xs text-slate-500 dark:text-slate-400">请稍候</p>
          </div>
        </div>
      )}
      {deleting && (
        <div className="absolute inset-0 z-10 flex flex-col items-center justify-center gap-3 bg-white/75 backdrop-blur-xl dark:bg-slate-950/70">
          <Loader2 size={24} className="animate-spin text-red-500" />
          <div className="text-center">
            <p className="text-sm font-medium">卸载中...</p>
            <p className="text-xs text-slate-500 dark:text-slate-400">请稍候</p>
          </div>
        </div>
      )}
      <div className="flex items-start justify-between mb-3">
        <div className="flex items-center gap-3">
          <div
            className={`flex h-11 w-11 items-center justify-center rounded-2xl border shadow-sm transition-colors ${
              tool.installed
                ? "border-emerald-200/70 bg-emerald-400/15 text-emerald-600 dark:border-emerald-300/20 dark:bg-emerald-400/10 dark:text-emerald-300"
                : "border-white/70 bg-white/70 text-slate-500 dark:border-white/10 dark:bg-white/8 dark:text-slate-400"
            }`}
          >
            {tool.installed ? (
              <CheckCircle size={21} />
            ) : (
              <Download size={21} />
            )}
          </div>
          <div>
            <h4 className="text-sm font-semibold text-slate-950 dark:text-white">{tool.name}</h4>
            <p className="mt-1 text-xs text-slate-500 dark:text-slate-400">
              {tool.installed ? (
                <span className="flex items-center gap-1.5 flex-wrap">
                  <span className="w-1.5 h-1.5 rounded-full bg-emerald-500" />
                  <span className="rounded-full border border-emerald-200/70 bg-emerald-400/10 px-2 py-0.5 text-[10px] font-semibold text-emerald-700 dark:border-emerald-300/20 dark:text-emerald-300">
                    {installBadgeLabel}
                  </span>
                  {tool.version && (
                    <span className="rounded-full bg-slate-100 px-2 py-0.5 text-[10px] font-medium text-slate-600 dark:bg-white/8 dark:text-slate-300">
                      CLI {tool.version}
                    </span>
                  )}
                  {tool.desktop_version && (
                    <span className="rounded-full bg-slate-100 px-2 py-0.5 text-[10px] font-medium text-slate-600 dark:bg-white/8 dark:text-slate-300">
                      桌面端 {tool.desktop_version}
                    </span>
                  )}
                  {hasUpdate && (
                    <span className="flex items-center gap-0.5 text-red-500" title={`有新版本 ${tool.latest_version}`}>
                      <AlertCircle size={12} />
                      <span>{tool.latest_version}</span>
                    </span>
                  )}
                  {!hasUpdate && tool.latest_version && (
                    <span className="text-slate-400 dark:text-slate-500">({tool.latest_version})</span>
                  )}
                </span>
              ) : tool.edition_unknown ? (
                <span className="flex items-center gap-1 text-amber-600 dark:text-amber-300">
                  <AlertCircle size={12} />
                  版本类型未知
                </span>
              ) : switchingEdition ? (
                <span className="flex items-center gap-1 text-amber-600 dark:text-amber-300">
                  <AlertCircle size={12} />
                  {tool.app_type === "workbuddy" ? "已安装国内版" : "已安装国际版"}
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
          {tool.installed && !isWorkBuddy && tool.detected_method && tool.detected_method !== "下载安装" && (
            <Pressable
              onClick={onDelete}
              className={`${iconButton} hover:text-red-500`}
              title="卸载工具"
            >
              <Trash2
                size={14}
              />
            </Pressable>
          )}
          <Pressable
            onClick={() => openUrl(tool.homepage).catch(console.error)}
            className={iconButton}
            title="访问官网"
          >
            <ExternalLink
              size={14}
            />
          </Pressable>
          {docsUrl && (
            <Pressable
              onClick={() => openUrl(docsUrl).catch(console.error)}
              className={iconButton}
              title="使用文档"
            >
              <BookOpen
                size={14}
              />
            </Pressable>
          )}
        </div>
      </div>

      <div className="flex gap-2">
        {tool.installed ? (
          <>
            {isLaunchable(tool.app_type) && tool.has_cli && (
              <Pressable
                onClick={() => onLaunch("cli")}
                className="flex min-h-10 flex-1 items-center justify-center gap-2 rounded-xl bg-gradient-to-r from-emerald-500 to-teal-500 px-3 py-2 text-xs font-semibold text-white shadow-[0_10px_24px_rgba(16,185,129,0.22)] transition-all duration-200 hover:-translate-y-0.5 hover:shadow-[0_14px_30px_rgba(16,185,129,0.28)]"
                title="启动 CLI"
              >
                <Play size={14} />
                启动 CLI
              </Pressable>
            )}
            {isLaunchable(tool.app_type) && tool.has_desktop_app && (
              <Pressable
                onClick={() => onLaunch("desktop")}
                className="flex min-h-10 flex-1 items-center justify-center gap-2 rounded-xl bg-gradient-to-r from-indigo-500 to-blue-500 px-3 py-2 text-xs font-semibold text-white shadow-[0_10px_24px_rgba(59,130,246,0.22)] transition-all duration-200 hover:-translate-y-0.5 hover:shadow-[0_14px_30px_rgba(59,130,246,0.28)]"
                title="启动桌面端"
              >
                <Play size={14} />
                启动桌面端
              </Pressable>
            )}
            <Pressable
              onClick={onScan}
              disabled={scanning || updating}
              className={secondaryButton}
              title="扫描版本"
            >
              <RefreshCw
                size={14}
                className={scanning ? 'animate-spin' : ''}
              />
              {scanning ? "扫描中..." : "扫描"}
            </Pressable>
            {tool.has_cli && (
              <Pressable
                onClick={() => onUpdate("cli")}
                disabled={updating}
                className={primaryButton}
                title="更新 CLI"
              >
                {updating ? (
                  <Loader2 size={14} className="animate-spin" />
                ) : (
                  <RefreshCw size={14} />
                )}
                {updating ? "更新中..." : "更新 CLI"}
              </Pressable>
            )}
            {tool.has_desktop_app && (
              <Pressable
                onClick={() => onUpdate("desktop")}
                className={primaryButton}
                title="更新桌面端"
              >
                <RefreshCw size={14} />
                更新桌面端
              </Pressable>
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
                  <Pressable
                    onClick={onOpenDownload}
                    className={primaryButton}
                  >
                    <ExternalLink size={12} />
                    {switchingEdition ? "切换版本" : tool.edition_unknown ? "重新安装" : "下载安装"}
                  </Pressable>
                );
              }

              if (npmMethod) {
                return (
                  <Pressable
                    onClick={() => onInstall(npmMethod.index, npmMethod.needs_confirm, npmMethod.command)}
                    disabled={installing}
                    className={primaryButton}
                  >
                    {installing ? (
                      <Loader2 size={12} className="animate-spin" />
                    ) : (
                      <Download size={12} />
                    )}
                    {installing ? "安装中..." : "安装"}
                  </Pressable>
                );
              }

              return (
                <Pressable
                  onClick={() => setShowMethods(!showMethods)}
                  disabled={installing}
                  className={primaryButton}
                >
                  {installing ? (
                    <Loader2 size={12} className="animate-spin" />
                  ) : (
                    <Download size={12} />
                  )}
                  {installing ? "安装中..." : "安装"}
                </Pressable>
              );
            })()}
          </>
        )}
      </div>

      {showMethods && !tool.installed && (
        <div className="mt-4 space-y-2 border-t border-white/60 pt-4 dark:border-white/10">
          <p className="mb-2 text-xs text-slate-500 dark:text-slate-400">
            选择安装方式:
          </p>
          {tool.methods.map((method) => (
            <Pressable
              key={method.index}
              onClick={() =>
                onInstall(method.index, method.needs_confirm, method.command)
              }
              disabled={installing}
              className="flex min-h-10 w-full items-center justify-between rounded-xl border border-white/60 bg-white/60 px-3 py-2 text-xs transition-all hover:bg-white/85 disabled:opacity-50 dark:border-white/10 dark:bg-white/8 dark:hover:bg-white/12"
            >
              <span className="font-medium">{method.name}</span>
              <code className="max-w-[180px] truncate text-[10px] text-slate-500 dark:text-slate-400">
                {method.command}
              </code>
            </Pressable>
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
  const [updatingTools, setUpdatingTools] = useState<Set<string>>(() => new Set());
  const [installingTool, setInstallingTool] = useState<string | null>(null);
  const [scanningTool, setScanningTool] = useState<string | null>(null);
  const [deletingTool, setDeletingTool] = useState<string | null>(null);
  const [showToolSettings, setShowToolSettings] = useState(false);
  const [hiddenTools, setHiddenTools] = useState<Set<string>>(() => {
    try {
      const raw = localStorage.getItem(toolVisibilityStorageKey);
      const parsed = raw ? JSON.parse(raw) : [];
      return new Set(Array.isArray(parsed) ? parsed.filter((id) => typeof id === "string") : []);
    } catch {
      return new Set();
    }
  });

  // 使用共享的工具检测上下文
  const { refresh: refreshInstalledTools, markAgentUninstalled, markAgentInstalled } = useInstalledTools();

  const { data: tools, isLoading, refetch, isFetching } = useQuery({
    queryKey: ["tool-infos"],
    queryFn: toolApi.getToolInfos,
    staleTime: 5 * 60 * 1000,
    gcTime: 30 * 60 * 1000,
  });
  const sortedTools = tools?.slice().sort((a, b) => a.name.localeCompare(b.name)) || [];
  const visibleTools = sortedTools.filter((tool) => !hiddenTools.has(tool.app_type));

  useEffect(() => {
    localStorage.setItem(toolVisibilityStorageKey, JSON.stringify(Array.from(hiddenTools)));
  }, [hiddenTools]);

  const toggleToolVisibility = (appType: string) => {
    setHiddenTools((prev) => {
      const next = new Set(prev);
      if (next.has(appType)) {
        next.delete(appType);
      } else {
        next.add(appType);
      }
      return next;
    });
  };

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
            desktop_version: scanned.desktop_version ?? tool.desktop_version,
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
      enhancementApi.recordTaskLog({
        kind: "install",
        title: `安装成功: ${updatedInfo.name}`,
        detail: `当前版本: ${updatedInfo.version || "未知"}`,
        status: "success",
      }).catch(console.error);
      setInstallingTool(null);
      queryClient.setQueryData(["tool-infos"], (old: any) => {
        if (!old) return old;
        return old.map((tool: any) =>
          tool.app_type === updatedInfo.app_type ? updatedInfo : tool
        );
      });
      markAgentInstalled(updatedInfo.app_type);
    },
    onError: (error: unknown, variables) => {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(`安装失败: ${message}`);
      enhancementApi.recordTaskLog({
        kind: "install",
        title: `安装失败: ${variables.appType}`,
        detail: message,
        status: "error",
      }).catch(console.error);
      setInstallingTool(null);
    },
  });

  const updateMutation = useMutation({
    mutationFn: async ({
      appType,
      updateKind,
    }: {
      appType: string;
      updateKind: "cli" | "desktop";
    }) => {
      setUpdatingTools((prev) => new Set(prev).add(appType));
      await toolApi.updateTool(appType, updateKind);
      await new Promise(resolve => setTimeout(resolve, 2000));
      const updatedInfo = await toolApi.getToolInfo(appType);
      return { updatedInfo, updateKind };
    },
    onSuccess: ({ updatedInfo }) => {
      toast.success("CLI 更新成功");
      enhancementApi.recordTaskLog({
        kind: "update",
        title: `CLI 更新: ${updatedInfo.name}`,
        detail: `当前版本: ${updatedInfo.version || "未知"}`,
        status: "success",
      }).catch(console.error);
      setUpdatingTools((prev) => {
        const next = new Set(prev);
        next.delete(updatedInfo.app_type);
        return next;
      });
      queryClient.setQueryData(["tool-infos"], (old: any) => {
        if (!old) return old;
        return old.map((tool: any) =>
          tool.app_type === updatedInfo.app_type ? updatedInfo : tool
        );
      });
    },
    onError: (error: unknown, variables) => {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(`更新失败: ${message}`);
      enhancementApi.recordTaskLog({
        kind: "update",
        title: `更新失败: ${variables.appType}`,
        detail: message,
        status: "error",
      }).catch(console.error);
      setUpdatingTools((prev) => {
        const next = new Set(prev);
        next.delete(variables.appType);
        return next;
      });
    },
  });

  const scanMutation = useMutation({
    mutationFn: async (appType: string) => {
      setScanningTool(appType);
      const scannedInfo = await toolApi.getToolInfo(appType);
      return scannedInfo;
    },
    onSuccess: (scannedInfo) => {
      enhancementApi.recordTaskLog({
        kind: "scan",
        title: `扫描完成: ${scannedInfo.name}`,
        detail: `已安装: ${scannedInfo.installed ? "是" : "否"}，CLI 版本: ${scannedInfo.version || "未知"}，桌面端版本: ${scannedInfo.desktop_version || "未知"}，最新版本: ${scannedInfo.latest_version || "未知"}`,
        status: "success",
      }).catch(console.error);
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
      enhancementApi.recordTaskLog({
        kind: "scan",
        title: "扫描失败",
        detail: message,
        status: "error",
      }).catch(console.error);
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
      enhancementApi.recordTaskLog({
        kind: "uninstall",
        title: `卸载成功: ${updatedInfo.name}`,
        detail: "工具已从系统移除或不再可检测",
        status: "success",
      }).catch(console.error);
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
      enhancementApi.recordTaskLog({
        kind: "uninstall",
        title: "卸载失败",
        detail: message,
        status: "error",
      }).catch(console.error);
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

  const handleUpdate = (appType: string, updateKind: "cli" | "desktop") => {
    if (updateKind === "desktop") {
      toolApi.updateTool(appType, "desktop").catch((e) => {
        toast.error(`打开更新页面失败: ${e}`);
      });
      return;
    }
    if (updatingTools.has(appType)) return;
    updateMutation.mutate({ appType, updateKind });
  };

  const handleOpenDownload = (tool: { app_type: string; name: string; homepage: string }, switchingEdition: boolean) => {
    if (!switchingEdition) {
      openUrl(tool.homepage).catch((e) => toast.error(`打开下载页面失败: ${e}`));
      return;
    }
    setConfirmDialog({
      open: true,
      title: "确认切换 WorkBuddy 版本",
      message: `安装 ${tool.name} 会替换当前已安装的 WorkBuddy 版本，但不会删除共享的 ~/.workbuddy 配置、MCP 或 Skills 数据。`,
      confirmText: "打开下载页面",
      onConfirm: () => {
        setConfirmDialog(null);
        openUrl(tool.homepage).catch((e) => toast.error(`打开下载页面失败: ${e}`));
      },
    });
  };

  const handleLaunch = async (appType: string, launchKind: "cli" | "desktop") => {
    try {
      await agentApi.launchAgent(appType, undefined, launchKind);
    } catch (e) {
      toast.error(`启动失败: ${e}`);
    }
  };

  if (isLoading) {
    return (
      <div className="relative isolate flex h-full flex-col overflow-hidden bg-[linear-gradient(135deg,#f8fbff_0%,#edf4ff_45%,#f8fafc_100%)] dark:bg-[linear-gradient(135deg,#020617_0%,#0f172a_52%,#111827_100%)]">
        <div className="pointer-events-none absolute inset-0 bg-[linear-gradient(115deg,rgba(37,99,235,0.10),transparent_34%,rgba(16,185,129,0.10)_67%,transparent)] dark:bg-[linear-gradient(115deg,rgba(56,189,248,0.12),transparent_35%,rgba(16,185,129,0.09)_68%,transparent)]" />
        <div className="relative px-8 pt-8 pb-6">
          <div className="flex items-center justify-between">
            <div>
              <div className="h-9 w-40 animate-pulse rounded-xl bg-white/70 dark:bg-white/10" />
              <div className="mt-2 flex h-4 w-56 animate-pulse items-center gap-2 rounded-md bg-white/60 dark:bg-white/8">
                <Loader2 size={14} className="animate-spin text-[hsl(var(--primary))]" />
                <span className="text-sm text-[hsl(var(--muted-foreground))]">正在扫描中...</span>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <div className="h-9 w-28 animate-pulse rounded-xl bg-white/70 dark:bg-white/10" />
              <div className="h-9 w-9 animate-pulse rounded-xl bg-white/70 dark:bg-white/10" />
            </div>
          </div>
        </div>
        <div className="relative flex-1 overflow-y-auto px-8 py-6">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {Array.from({ length: 6 }, (_, i) => i).map((i) => (
              <div
                key={i}
                className={`animate-pulse rounded-2xl p-5 ${glassSurface}`}
                style={{ animationDelay: `${i * 100}ms` }}
              >
                <div className="flex items-start justify-between mb-3">
                  <div className="flex items-center gap-3">
                    <div className="h-11 w-11 rounded-2xl bg-white/70 dark:bg-white/10" />
                    <div>
                      <div className="mb-1 h-4 w-20 rounded-md bg-white/70 dark:bg-white/10" />
                      <div className="h-3 w-28 rounded-md bg-white/60 dark:bg-white/8" />
                    </div>
                  </div>
                  <div className="h-9 w-9 rounded-xl bg-white/70 dark:bg-white/10" />
                </div>
                <div className="flex gap-2">
                  <div className="h-10 flex-1 rounded-xl bg-white/70 dark:bg-white/10" />
                  <div className="h-10 flex-1 rounded-xl bg-white/70 dark:bg-white/10" />
                  <div className="h-10 flex-1 rounded-xl bg-white/70 dark:bg-white/10" />
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="relative isolate flex h-full flex-col overflow-hidden bg-[linear-gradient(135deg,#f8fbff_0%,#edf4ff_45%,#f8fafc_100%)] text-slate-950 dark:bg-[linear-gradient(135deg,#020617_0%,#0f172a_52%,#111827_100%)] dark:text-white">
      <div className="pointer-events-none absolute inset-0 bg-[linear-gradient(115deg,rgba(37,99,235,0.10),transparent_34%,rgba(16,185,129,0.10)_67%,transparent)] dark:bg-[linear-gradient(115deg,rgba(56,189,248,0.12),transparent_35%,rgba(16,185,129,0.09)_68%,transparent)]" />
      <div className="relative px-8 pt-8 pb-6">
        <div className="flex items-center justify-between">
          <div>
            <div className="inline-flex items-center gap-2 rounded-full border border-white/70 bg-white/55 px-3 py-1 text-xs font-semibold text-blue-700 shadow-sm backdrop-blur-xl dark:border-white/10 dark:bg-white/8 dark:text-sky-300">
              <Package size={13} />
              Agent Tools
            </div>
            <h2 className="mt-3 text-display">工具管理</h2>
            <p className="mt-2 flex items-center gap-2 text-sm text-slate-500 dark:text-slate-400">
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
            <Pressable
              onClick={() => setShowToolSettings((open) => !open)}
              className={iconButton}
              title="管理工具展示"
            >
              <Settings size={18} />
            </Pressable>
            <Pressable
              onClick={async () => {
                // 调用全局刷新，刷新后所有模块共享结果
                await refreshInstalledTools();
                // 同时刷新工具详情
                refetch();
              }}
              disabled={isFetching}
              className={iconButton}
              title="刷新"
            >
              <RefreshCw size={18} className={isFetching ? 'animate-spin' : ''} />
            </Pressable>
          </div>
        </div>
        {showToolSettings && (
          <div className={`mt-5 rounded-2xl p-4 ${glassSurface}`}>
            <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
              <div>
                <h3 className="text-sm font-semibold">工具列表展示</h3>
                <p className="mt-0.5 text-xs text-slate-500 dark:text-slate-400">
                  已展示 {visibleTools.length}/{sortedTools.length} 个工具
                </p>
              </div>
              <Pressable
                onClick={() => setHiddenTools(new Set())}
                className="glass-secondary-button min-h-8 flex-none px-3 py-1 text-xs"
              >
                全部展示
              </Pressable>
            </div>
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 xl:grid-cols-4">
              {sortedTools.map((tool) => {
                const visible = !hiddenTools.has(tool.app_type);
                return (
                  <Pressable
                    key={tool.app_type}
                    onClick={() => toggleToolVisibility(tool.app_type)}
                    className={`inline-flex min-h-9 items-center gap-2 rounded-xl border px-3 text-left text-xs font-semibold transition-all ${
                      visible
                        ? "border-blue-200/70 bg-blue-500/10 text-blue-700 dark:border-sky-300/20 dark:text-sky-300"
                        : "border-white/55 bg-white/45 text-slate-400 hover:text-slate-700 dark:border-white/10 dark:bg-white/8 dark:text-slate-500 dark:hover:text-slate-300"
                    }`}
                  >
                    {visible ? <CheckSquare size={14} /> : <Square size={14} />}
                    <span className="min-w-0 truncate">{tool.name}</span>
                  </Pressable>
                );
              })}
            </div>
          </div>
        )}
      </div>

      <div className="relative flex-1 overflow-y-auto px-8 pb-8 pt-2">
        {visibleTools.length === 0 ? (
          <div className={`flex min-h-64 flex-col items-center justify-center rounded-2xl p-6 text-center ${glassSurface}`}>
            <Package size={28} className="mb-3 text-slate-400" />
            <p className="text-sm font-semibold">暂无展示的工具</p>
            <p className="mt-1 text-xs text-slate-500 dark:text-slate-400">
              点击顶部设置图标选择要展示的工具
            </p>
          </div>
        ) : (
          <MotionList className="grid grid-cols-1 gap-4 xl:grid-cols-2">
            {visibleTools.map((tool) => {
              const otherWorkBuddyId = tool.app_type === "workbuddy"
                ? "workbuddy-cn"
                : tool.app_type === "workbuddy-cn"
                  ? "workbuddy"
                  : null;
              const switchingEdition = otherWorkBuddyId !== null
                && Boolean(tools?.find((candidate) => candidate.app_type === otherWorkBuddyId)?.installed);
              return (
                <MotionListItem key={tool.app_type}>
                  <ToolCard
                    tool={tool}
                    onInstall={(methodIndex, needsConfirm, command) =>
                      handleInstall(tool.app_type, methodIndex, needsConfirm, command)
                    }
                    onUpdate={(updateKind) => handleUpdate(tool.app_type, updateKind)}
                    onScan={() => scanMutation.mutate(tool.app_type)}
                    onLaunch={(launchKind) => handleLaunch(tool.app_type, launchKind)}
                    onDelete={() => handleDelete(tool.app_type, tool.name)}
                    onOpenDownload={() => handleOpenDownload(tool, switchingEdition)}
                    switchingEdition={switchingEdition}
                    installing={installingTool === tool.app_type}
                    updating={updatingTools.has(tool.app_type)}
                    scanning={scanningTool === tool.app_type}
                    deleting={deletingTool === tool.app_type}
                  />
                </MotionListItem>
              );
            })}
          </MotionList>
        )}
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
