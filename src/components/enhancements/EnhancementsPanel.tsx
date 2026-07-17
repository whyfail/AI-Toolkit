import { useCallback, useEffect, useMemo, useState } from "react";
import { motion } from "motion/react";
import { Pressable } from "@/components/ui/Pressable";
import {
  Activity,
  AlertTriangle,
  Archive,
  CheckCircle2,
  ClipboardPaste,
  Download,
  FileJson,
  History,
  Import,
  Loader2,
  Play,
  RefreshCw,
  Rocket,
  Shield,
  Upload,
  Wrench,
  XCircle,
  type LucideIcon,
} from "lucide-react";
import { toast } from "sonner";
import * as dialog from "@tauri-apps/plugin-dialog";
import {
  enhancementApi,
  type BulkMcpServerInput,
  type ConflictItem,
  type EnhancementSnapshot,
  type HealthCheckItem,
  type LaunchPreset,
  type OnboardingChecklistItem,
  type SecurityFinding,
  type TaskLogEntry,
} from "@/lib/api";
import { agentApi, mcpApi } from "@/lib/api";
import { useAllMcpServers } from "@/hooks/useMcp";
import { useInstalledTools } from "@/contexts/InstalledToolsContext";

type PanelKey =
  | "onboarding"
  | "bulkImport"
  | "backup"
  | "health"
  | "skills"
  | "portable"
  | "logs"
  | "conflicts"
  | "security"
  | "presets";

const panelItems: Array<{ key: PanelKey; label: string; icon: LucideIcon }> = [
  { key: "onboarding", label: "首次引导", icon: Rocket },
  { key: "bulkImport", label: "MCP 批量导入", icon: ClipboardPaste },
  { key: "backup", label: "MCP 配置快照", icon: Archive },
  { key: "health", label: "健康检查", icon: Activity },
  { key: "skills", label: "Skills 更新", icon: RefreshCw },
  { key: "portable", label: "导入导出", icon: FileJson },
  { key: "logs", label: "任务日志", icon: History },
  { key: "conflicts", label: "冲突检测", icon: Wrench },
  { key: "security", label: "安全保护", icon: Shield },
  { key: "presets", label: "启动预设", icon: Play },
];

function formatTime(ms?: number) {
  if (!ms) return "-";
  return new Date(ms).toLocaleString();
}

function StatusBadge({ status }: { status: string }) {
  const className =
    status === "ok" || status === "success"
      ? "border-emerald-200 bg-emerald-500/10 text-emerald-700"
      : status === "error"
        ? "border-red-200 bg-red-500/10 text-red-600"
        : "border-amber-200 bg-amber-500/10 text-amber-700";
  return (
    <span className={`inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-semibold ${className}`}>
      {status}
    </span>
  );
}

function parseMcpServers(input: string): BulkMcpServerInput[] {
  const parsed = JSON.parse(input);
  const source = parsed.mcpServers || parsed;
  return Object.entries(source).map(([id, value]) => {
    const server = value as Record<string, unknown>;
    if (!server.command && !server.url && !server.httpUrl) {
      throw new Error(`${id} 缺少 command 或 url/httpUrl`);
    }
    return {
      id,
      name: typeof server.name === "string" ? server.name : id,
      server,
    };
  });
}

function SectionCard({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="glass-card p-4 sm:p-5">
      <h3 className="mb-3 text-sm font-semibold text-slate-950 dark:text-white">{title}</h3>
      {children}
    </div>
  );
}

export default function EnhancementsPanel() {
  const [active, setActive] = useState<PanelKey>("onboarding");
  const [loading, setLoading] = useState(false);
  const [checklist, setChecklist] = useState<OnboardingChecklistItem[]>([]);
  const [snapshots, setSnapshots] = useState<EnhancementSnapshot[]>([]);
  const [healthItems, setHealthItems] = useState<HealthCheckItem[]>([]);
  const [conflicts, setConflicts] = useState<ConflictItem[]>([]);
  const [securityFindings, setSecurityFindings] = useState<SecurityFinding[]>([]);
  const [logs, setLogs] = useState<TaskLogEntry[]>([]);
  const [skillUpdates, setSkillUpdates] = useState<Array<Record<string, unknown>>>([]);
  const [bulkJson, setBulkJson] = useState("");
  const [bulkOverwrite, setBulkOverwrite] = useState(false);
  const [packagePath, setPackagePath] = useState("");
  const [presets, setPresets] = useState<LaunchPreset[]>([]);
  const [presetName, setPresetName] = useState("");
  const [presetAgent, setPresetAgent] = useState("");
  const [presetDir, setPresetDir] = useState("");
  const { installedAgents, refresh } = useInstalledTools();
  const { data: serversMap, refetch: refetchMcp } = useAllMcpServers();

  const serverEntries = useMemo(() => Object.entries(serversMap || {}), [serversMap]);
  const selectedApps = useMemo(() => {
    const apps: Record<string, boolean> = {};
    installedAgents.forEach((agent) => {
      apps[agent.id] = true;
    });
    return apps;
  }, [installedAgents]);

  const loadAll = useCallback(async () => {
    setLoading(true);
    try {
      const [
        nextChecklist,
        nextSnapshots,
        nextLogs,
        nextConflicts,
        nextSecurity,
        nextPresets,
      ] = await Promise.all([
        enhancementApi.getOnboardingChecklist(),
        enhancementApi.listSnapshots(),
        enhancementApi.listTaskLogs(),
        enhancementApi.detectConfigConflicts(),
        enhancementApi.scanSecurityFindings(),
        enhancementApi.listLaunchPresets(),
      ]);
      setChecklist(nextChecklist);
      setSnapshots(nextSnapshots);
      setLogs(nextLogs);
      setConflicts(nextConflicts);
      setSecurityFindings(nextSecurity);
      setPresets(nextPresets);
      if (!presetAgent && installedAgents[0]) {
        setPresetAgent(installedAgents[0].id);
      }
    } catch (err) {
      toast.error(`加载增强中心失败: ${err}`);
    } finally {
      setLoading(false);
    }
  }, [installedAgents, presetAgent]);

  useEffect(() => {
    loadAll();
  }, [loadAll]);

  const handleBulkImport = async () => {
    try {
      const servers = parseMcpServers(bulkJson);
      if (servers.length === 0) {
        toast.warning("没有可导入的 MCP Server");
        return;
      }
      setLoading(true);
      const result = await enhancementApi.bulkImportMcpServers({
        servers,
        apps: selectedApps,
        overwrite: bulkOverwrite,
      });
      toast.success(`已导入 ${result.imported} 个 MCP，覆盖 ${result.overwritten} 个`);
      await Promise.all([refetchMcp(), loadAll()]);
    } catch (err) {
      toast.error(`批量导入失败: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const handleCreateSnapshot = async () => {
    setLoading(true);
    try {
      await enhancementApi.createSnapshot("用户手动创建");
      toast.success("配置快照已创建");
      setSnapshots(await enhancementApi.listSnapshots());
    } catch (err) {
      toast.error(`创建快照失败: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const handleRestoreSnapshot = async (id: string) => {
    setLoading(true);
    try {
      await enhancementApi.restoreSnapshot(id);
      toast.success("快照已恢复");
      await Promise.all([refetchMcp(), loadAll()]);
    } catch (err) {
      toast.error(`恢复失败: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const handleHealthCheck = async () => {
    setLoading(true);
    try {
      setHealthItems(await enhancementApi.runHealthCheck());
      setLogs(await enhancementApi.listTaskLogs());
    } catch (err) {
      toast.error(`健康检查失败: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const handleSkillPreview = async () => {
    setLoading(true);
    try {
      setSkillUpdates(await enhancementApi.previewSkillUpdates());
    } catch (err) {
      toast.error(`获取 Skills 更新预览失败: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const handleExport = async () => {
    setLoading(true);
    try {
      const selected = await dialog.save({
        title: "导出配置压缩包",
        defaultPath: `ai-toolkit-config-${new Date().toISOString().slice(0, 10)}.zip`,
        filters: [{ name: "AI Toolkit 配置包", extensions: ["zip"] }],
      });
      if (!selected) return;
      const exported = await enhancementApi.exportToolkitPackage(selected);
      setPackagePath(selected);
      toast.success(`配置压缩包已导出: ${exported.mcp_servers ? Object.keys(exported.mcp_servers).length : 0} 个 MCP、${exported.skills.length} 个 Skill`);
      if (exported.warnings.length > 0) {
        toast.warning(`导出包含 ${exported.warnings.length} 条提醒`);
      }
    } catch (err) {
      toast.error(`导出失败: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const handlePickPackage = async () => {
    try {
      const selected = await dialog.open({
        multiple: false,
        title: "选择配置压缩包",
        filters: [{ name: "AI Toolkit 配置包", extensions: ["zip"] }],
      });
      if (!selected || Array.isArray(selected)) return;
      setPackagePath(selected);
    } catch (err) {
      toast.error(`选择压缩包失败: ${err}`);
    }
  };

  const handleSavePreset = async () => {
    if (!presetName.trim() || !presetAgent) {
      toast.warning("请填写预设名称并选择工具");
      return;
    }
    setLoading(true);
    try {
      const preset = await enhancementApi.saveLaunchPreset({
        name: presetName.trim(),
        agentId: presetAgent,
        workingDir: presetDir.trim() || "~",
        enabledMcpServers: serverEntries.map(([id]) => id),
      });
      toast.success(`已保存预设: ${preset.name}`);
      setPresetName("");
      setPresetDir("");
      setPresets(await enhancementApi.listLaunchPresets());
    } catch (err) {
      toast.error(`保存预设失败: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const handleLaunchPreset = async (preset: LaunchPreset) => {
    try {
      await enhancementApi.recordTaskLog({
        kind: "launch",
        title: `启动预设: ${preset.name}`,
        detail: `工具 ${preset.agent_id}，目录 ${preset.working_dir}`,
        status: "success",
      });
      const selected = new Set(preset.enabled_mcp_servers);
      await Promise.all(
        serverEntries.map(([id, server]) => {
          const enabled = selected.has(id);
          if ((server.apps as Record<string, boolean>)[preset.agent_id] === enabled) {
            return Promise.resolve();
          }
          return mcpApi.toggleApp(id, preset.agent_id, enabled);
        })
      );
      await refetchMcp();
      await agentApi.launchAgent(preset.agent_id, preset.working_dir);
    } catch (err) {
      toast.error(`启动失败: ${err}`);
    }
  };

  return (
    <div className="glass-app flex h-full flex-col overflow-hidden">
      <div className="glass-header flex-shrink-0">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <div className="glass-kicker">
              <Wrench size={13} />
              Enhancements
            </div>
            <h2 className="mt-3 text-2xl font-semibold tracking-tight sm:text-3xl">
              增强中心
            </h2>
            <p className="mt-2 text-xs text-slate-500 dark:text-slate-400 sm:text-sm">
              MCP 批量导入、MCP 配置快照、健康检查、安全分享和启动预设
            </p>
          </div>
          <Pressable onClick={loadAll} disabled={loading} className="glass-secondary-button">
            <RefreshCw size={16} className={loading ? "animate-spin" : ""} />
            刷新
          </Pressable>
        </div>
        <div className="mt-5 flex flex-wrap gap-2">
          {panelItems.map((item) => {
            const isActive = active === item.key;
            return (
              <Pressable
                key={item.key}
                onClick={() => setActive(item.key)}
                aria-pressed={isActive}
                className={`relative z-10 inline-flex min-h-9 items-center gap-2 whitespace-nowrap rounded-xl px-3 text-xs font-medium transition-colors duration-200 ease-out ${
                  isActive
                    ? "text-white"
                    : "border border-white/60 bg-white/55 text-slate-600 hover:bg-white/80 dark:border-white/10 dark:bg-white/8 dark:text-slate-300 dark:hover:bg-white/12"
                }`}
              >
                {isActive && (
                  <motion.span
                    layoutId="enhancement-pill"
                    className="absolute inset-0 -z-10 rounded-xl bg-gradient-to-r from-[#0A84FF] to-[#5AC8FA] shadow-[0_4px_12px_rgba(10,132,255,0.22)]"
                    transition={{ type: "spring", bounce: 0.15, duration: 0.4 }}
                  />
                )}
                <item.icon size={14} />
                {item.label}
              </Pressable>
            );
          })}
        </div>
      </div>

      <div className="glass-content px-3 sm:px-8">
        {loading && (
          <div className="mb-3 inline-flex items-center gap-2 text-xs text-slate-500">
            <Loader2 size={14} className="animate-spin" />
            正在处理...
          </div>
        )}

        {active === "onboarding" && (
          <SectionCard title="首次使用引导">
            <div className="grid gap-3 md:grid-cols-2">
              {checklist.map((item) => (
                <div key={item.id} className="rounded-xl border border-white/60 bg-white/45 p-3">
                  <div className="flex items-center gap-2">
                    {item.done ? (
                      <CheckCircle2 size={16} className="text-emerald-600" />
                    ) : (
                      <XCircle size={16} className="text-amber-600" />
                    )}
                    <span className="text-sm font-semibold">{item.title}</span>
                  </div>
                  <p className="mt-1 text-xs text-slate-500">{item.detail}</p>
                </div>
              ))}
            </div>
            <div className="mt-4 flex flex-wrap gap-2">
              <Pressable onClick={refresh} className="glass-secondary-button">扫描工具</Pressable>
              <Pressable onClick={handleCreateSnapshot} className="glass-secondary-button">创建 MCP 配置快照</Pressable>
            </div>
          </SectionCard>
        )}

        {active === "bulkImport" && (
          <SectionCard title="MCP Server 批量导入">
            <p className="mb-3 text-xs text-slate-500 dark:text-slate-400">
              仅导入 MCP Server 配置，并同步到当前已安装工具的配置文件。
            </p>
            <textarea
              value={bulkJson}
              onChange={(event) => setBulkJson(event.target.value)}
              className="glass-input min-h-52 w-full p-3 font-mono text-xs"
              placeholder='粘贴 { "mcpServers": { ... } }，支持一次导入多个 MCP Server'
            />
            <div className="mt-3 flex flex-wrap items-center gap-3">
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={bulkOverwrite}
                  onChange={(event) => setBulkOverwrite(event.target.checked)}
                />
                覆盖同 ID 配置
              </label>
              <Pressable onClick={handleBulkImport} className="glass-primary-button">
                <Import size={16} />
                导入到已安装工具
              </Pressable>
            </div>
          </SectionCard>
        )}

        {active === "backup" && (
          <SectionCard title="MCP 与工具配置文件快照">
            <p className="mb-3 text-xs text-slate-500 dark:text-slate-400">
              备份 MCP Server 数据和各工具 MCP 配置文件，可在配置改坏后恢复。
            </p>
            <Pressable onClick={handleCreateSnapshot} className="glass-primary-button">
              <Archive size={16} />
              创建 MCP 配置快照
            </Pressable>
            <div className="mt-4 space-y-2">
              {snapshots.map((snapshot) => (
                <div key={snapshot.id} className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-white/60 bg-white/45 p-3">
                  <div>
                    <p className="text-sm font-semibold">{snapshot.reason}</p>
                    <p className="text-xs text-slate-500">
                      {formatTime(snapshot.created_at)} · {snapshot.server_count} MCP · {snapshot.config_count} 配置文件
                    </p>
                  </div>
                  <Pressable onClick={() => handleRestoreSnapshot(snapshot.id)} className="glass-secondary-button">
                    恢复
                  </Pressable>
                </div>
              ))}
            </div>
          </SectionCard>
        )}

        {active === "health" && (
          <SectionCard title="MCP 健康检查">
            <Pressable onClick={handleHealthCheck} className="glass-primary-button">
              <Activity size={16} />
              开始检查
            </Pressable>
            <div className="mt-4 space-y-2">
              {healthItems.map((item) => (
                <div key={`${item.id}-${item.scope}-${item.message}`} className="rounded-xl border border-white/60 bg-white/45 p-3">
                  <div className="flex flex-wrap items-center gap-2">
                    <StatusBadge status={item.status} />
                    <span className="text-sm font-semibold">{item.name}</span>
                    <span className="text-xs text-slate-400">{item.scope}</span>
                  </div>
                  <p className="mt-1 text-xs text-slate-500">{item.message}</p>
                </div>
              ))}
            </div>
          </SectionCard>
        )}

        {active === "skills" && (
          <SectionCard title="Skills 更新预览">
            <Pressable onClick={handleSkillPreview} className="glass-primary-button">
              <RefreshCw size={16} />
              生成更新预览
            </Pressable>
            <div className="mt-4 space-y-2">
              {skillUpdates.map((item) => (
                <div key={String(item.id)} className="rounded-xl border border-white/60 bg-white/45 p-3">
                  <p className="text-sm font-semibold">{String(item.name)}</p>
                  <p className="mt-1 text-xs text-slate-500 break-all">{String(item.source)}</p>
                  <p className="mt-1 text-xs text-slate-500">{String(item.status)} · {String(item.impact)}</p>
                </div>
              ))}
            </div>
          </SectionCard>
        )}

        {active === "portable" && (
          <SectionCard title="配置压缩包导入 / 导出">
            <div className="flex flex-wrap gap-2">
              <Pressable onClick={handleExport} className="glass-primary-button">
                <Download size={16} />
                导出压缩包
              </Pressable>
              <Pressable onClick={handlePickPackage} className="glass-secondary-button">
                <Upload size={16} />
                选择压缩包
              </Pressable>
            </div>
            <div className="mt-3 rounded-xl border border-white/60 bg-white/45 p-3 text-sm text-slate-600 dark:border-white/10 dark:bg-white/8 dark:text-slate-300">
              {packagePath ? (
                <span className="break-all">{packagePath}</span>
              ) : (
                <span>请选择或导出一个 .zip 配置压缩包</span>
              )}
            </div>
          </SectionCard>
        )}

        {active === "logs" && (
          <SectionCard title="安装 / 同步 / 导入日志">
            <div className="space-y-2">
              {logs.map((log) => (
                <div key={log.id} className="rounded-xl border border-white/60 bg-white/45 p-3">
                  <div className="flex flex-wrap items-center gap-2">
                    <StatusBadge status={log.status} />
                    <span className="text-sm font-semibold">{log.title}</span>
                    <span className="text-xs text-slate-400">{formatTime(log.created_at)}</span>
                  </div>
                  <p className="mt-1 text-xs text-slate-500">{log.detail}</p>
                </div>
              ))}
            </div>
          </SectionCard>
        )}

        {active === "conflicts" && (
          <SectionCard title="冲突检测与智能合并提示">
            <Pressable
              onClick={async () => setConflicts(await enhancementApi.detectConfigConflicts())}
              className="glass-primary-button"
            >
              <Wrench size={16} />
              重新检测
            </Pressable>
            <div className="mt-4 space-y-2">
              {conflicts.length === 0 && <p className="text-sm text-slate-500">暂无冲突。</p>}
              {conflicts.map((item) => (
                <div key={`${item.scope}-${item.key}`} className="rounded-xl border border-white/60 bg-white/45 p-3">
                  <div className="flex items-center gap-2">
                    <AlertTriangle size={15} className="text-amber-600" />
                    <span className="text-sm font-semibold">{item.scope}</span>
                    <StatusBadge status={item.severity} />
                  </div>
                  <p className="mt-1 text-xs text-slate-500">{item.message}</p>
                </div>
              ))}
            </div>
          </SectionCard>
        )}

        {active === "security" && (
          <SectionCard title="敏感信息保护">
            <Pressable
              onClick={async () => setSecurityFindings(await enhancementApi.scanSecurityFindings())}
              className="glass-primary-button"
            >
              <Shield size={16} />
              扫描敏感字段
            </Pressable>
            <div className="mt-4 space-y-2">
              {securityFindings.length === 0 && <p className="text-sm text-slate-500">未发现明显敏感字段。</p>}
              {securityFindings.map((item) => (
                <div key={`${item.scope}-${item.key}-${item.message}`} className="rounded-xl border border-white/60 bg-white/45 p-3">
                  <div className="flex flex-wrap items-center gap-2">
                    <StatusBadge status={item.severity} />
                    <span className="text-sm font-semibold">{item.scope}</span>
                    <code className="text-xs text-slate-500">{item.key}</code>
                  </div>
                  <p className="mt-1 text-xs text-slate-500">{item.message}</p>
                </div>
              ))}
            </div>
          </SectionCard>
        )}

        {active === "presets" && (
          <SectionCard title="Agent 启动预设">
            <div className="grid gap-2 md:grid-cols-3">
              <input
                value={presetName}
                onChange={(event) => setPresetName(event.target.value)}
                className="glass-input px-3 py-2 text-sm"
                placeholder="预设名称"
              />
              <select
                value={presetAgent}
                onChange={(event) => setPresetAgent(event.target.value)}
                className="glass-select px-3 py-2 text-sm"
              >
                <option value="">选择工具</option>
                {installedAgents.map((agent) => (
                  <option key={agent.id} value={agent.id}>{agent.name}</option>
                ))}
              </select>
              <input
                value={presetDir}
                onChange={(event) => setPresetDir(event.target.value)}
                className="glass-input px-3 py-2 text-sm"
                placeholder="启动目录，例如 ~/Desktop/project"
              />
            </div>
            <Pressable onClick={handleSavePreset} className="glass-primary-button mt-3">
              保存预设
            </Pressable>
            <div className="mt-4 space-y-2">
              {presets.map((preset) => (
                <div key={preset.id} className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-white/60 bg-white/45 p-3">
                  <div>
                    <p className="text-sm font-semibold">{preset.name}</p>
                    <p className="text-xs text-slate-500">
                      {preset.agent_id} · {preset.working_dir} · {preset.enabled_mcp_servers.length} MCP
                    </p>
                  </div>
                  <Pressable onClick={() => handleLaunchPreset(preset)} className="glass-secondary-button">
                    <Play size={15} />
                    启动
                  </Pressable>
                </div>
              ))}
            </div>
          </SectionCard>
        )}
      </div>
    </div>
  );
}
