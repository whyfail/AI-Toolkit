import { useCallback, useEffect, useId, useRef, useState, type ReactNode } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  ArrowUpRight, CheckCircle2, Coins, Copy, ExternalLink, KeyRound, Loader2,
  Info, Plus, Power, RefreshCw, Server, ShieldCheck, Trash2, X,
} from "lucide-react";
import { open } from "@tauri-apps/plugin-shell";
import { toast } from "sonner";
import { Modal } from "@/components/ui/Modal";
import { Pressable } from "@/components/ui/Pressable";
import { AgentIcon } from "@/components/ui/AgentIcon";
import { ModelInfoPopover } from "./ModelInfoPopover";
import { creditsQueryKey, useCreditsAction, useCreditsDashboard } from "@/hooks/useCredits";
import {
  creditsApi, type CreditAccount, type CreditDashboard,
  type CreditModel, type CreditPlatform, type OAuthStart, type RewardOutcome,
} from "@/lib/api/credits";

const platforms: Array<{ id: CreditPlatform; name: string }> = [
  { id: "workbuddy", name: "WorkBuddy" },
  { id: "traework", name: "TRAE CN" },
  { id: "qoder", name: "Qoder" },
];
const sections = [
  { id: "accounts", label: "账户与积分" },
  { id: "proxy", label: "本地代理" },
] as const;
type PanelTab = typeof sections[number]["id"];
const platformName = (platform: CreditPlatform) => platforms.find((item) => item.id === platform)?.name ?? platform;
const number = (value: number) => new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 2 }).format(value);
const time = (value?: number | null) => value ? new Date(value).toLocaleString("zh-CN", { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit" }) : "尚未同步";
const errorMessage = (error: unknown) => error instanceof Error ? error.message : String(error);
const isCredits = (kind: string) => ["credit", "credits", "points"].includes(kind);
const isClaimable = (account: CreditAccount) => account.enabled && account.snapshot?.rewards.some((reward) =>
  isCredits(reward.kind) && ["claimable", "available", "eligible"].includes(reward.status));
const accountModels = (account: CreditAccount) => {
  const seen = new Set<string>();
  return (account.snapshot?.models ?? []).filter((model) => {
    if (seen.has(model.id)) return false;
    seen.add(model.id);
    return true;
  });
};
const availableModels = (data: CreditDashboard) => data.accounts.filter((account) => account.enabled && account.status === "ready" && (account.snapshot?.balance == null || account.snapshot.balance > 0))
  .flatMap((account) => accountModels(account).map((model) => ({ account, model })));

const labels: Record<string, string> = {
  active: "已连接", ready: "已连接", connected: "已连接", authenticated: "已连接",
  expired: "登录已过期", auth_expired: "登录已过期", error: "连接异常", unknown: "待刷新",
  unsupported: "暂不支持", pending: "等待授权", claimed: "已领取", success: "成功",
  failed: "失败", skipped: "已跳过", claimable: "可领取", available: "可领取",
  unavailable: "暂不可领", already_claimed: "已领取", not_supported: "暂不支持",
  device_required: "需要已绑定的设备", eligible: "可领取",
  cancelled: "已取消", completed: "成功",
};

function Badge({ children, success = false }: { children: ReactNode; success?: boolean }) {
  return <span className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[11px] font-medium ${success ? "bg-emerald-500/10 text-emerald-700 dark:text-emerald-300" : "bg-slate-500/10 text-slate-600 dark:text-slate-300"}`}>{children}</span>;
}

async function copyValue(value: string, label: string) {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(value);
    } else {
      const field = document.createElement("textarea");
      field.value = value;
      field.style.position = "fixed";
      field.style.opacity = "0";
      document.body.appendChild(field);
      field.select();
      const copied = document.execCommand("copy");
      field.remove();
      if (!copied) throw new Error("复制失败");
    }
    toast.success(`已复制${label}`);
  } catch {
    toast.error("复制失败，请检查剪贴板权限");
  }
}

function CopyField({ label, value }: { label: string; value: string }) {
  return <div className="space-y-2">
    <span className="text-xs font-medium text-slate-500 dark:text-slate-400">{label}</span>
    <div className="glass-code flex items-center gap-3 rounded-xl px-3 py-2.5">
      <code className="min-w-0 flex-1 break-all text-xs">{value}</code>
      <Pressable type="button" onClick={() => void copyValue(value, label)} className="glass-icon-button shrink-0" aria-label={`复制${label}`}><Copy size={14} /></Pressable>
    </div>
  </div>;
}

type ModelEntry = { account: CreditAccount; model: CreditModel };
type ModelGroup = { title: string; entries: ModelEntry[] };

interface ModelRowProps extends ModelEntry {
  infoId?: string;
  onPreview: (entry: ModelEntry, anchor: HTMLElement, immediate?: boolean) => void;
  onToggle: (entry: ModelEntry, anchor: HTMLElement) => void;
  onLeave: () => void;
  onBlur: () => void;
}

function ModelRow({ account, model, infoId, onPreview, onToggle, onLeave, onBlur }: ModelRowProps) {
  const row = useRef<HTMLDivElement>(null);
  const id = `${account.platform}/${model.id}`;
  return <div ref={row} className={`rounded-xl px-3 py-2.5 transition-colors ${infoId ? "bg-blue-500/10" : "bg-slate-500/5 hover:bg-slate-500/10"}`}
    onPointerEnter={(event) => onPreview({ account, model }, event.currentTarget)} onPointerLeave={onLeave} onBlur={onBlur}>
    <div className="flex flex-wrap items-center justify-between gap-2">
      <button type="button" className="min-w-0 flex-1 rounded-lg text-left outline-none focus-visible:ring-2 focus-visible:ring-blue-500/60"
        aria-label={`查看 ${model.name || model.id} 模型信息`} aria-describedby={infoId} aria-expanded={!!infoId}
        onFocus={() => { if (row.current) onPreview({ account, model }, row.current, true); }}
        onClick={() => { if (row.current) onToggle({ account, model }, row.current); }}>
        <span className="flex items-center gap-2 text-xs font-medium"><span className="min-w-0 break-all">{model.name || model.id}</span><Info size={13} className="shrink-0 text-slate-400" aria-hidden="true" /></span>
        <span className="mt-1 block break-all text-[11px] text-slate-500 dark:text-slate-400">{id}</span>
      </button>
      <div className="flex shrink-0 items-center gap-1.5"><Pressable type="button" className="glass-icon-button" aria-label={`复制模型 ID ${id}`} onClick={() => void copyValue(id, "模型 ID")}><Copy size={13} /></Pressable></div>
    </div>
  </div>;
}

function ModelsModal({ title, groups, onClose }: { title: string; groups: ModelGroup[]; onClose: () => void }) {
  const infoId = useId();
  const [details, setDetails] = useState<{ entry: ModelEntry; anchor: HTMLElement } | null>(null);
  const delay = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pinned = useRef(false);
  const pointerInside = useRef(false);
  const clearDelay = useCallback(() => { if (delay.current) clearTimeout(delay.current); delay.current = null; }, []);
  const dismiss = useCallback(() => { clearDelay(); pinned.current = false; pointerInside.current = false; setDetails(null); }, [clearDelay]);
  useEffect(() => clearDelay, [clearDelay]);

  const preview = (entry: ModelEntry, anchor: HTMLElement, immediate = false) => {
    clearDelay();
    if (!immediate) pointerInside.current = true;
    const show = () => { pinned.current = false; setDetails({ entry, anchor }); };
    if (immediate) show();
    else if (details?.anchor !== anchor) delay.current = setTimeout(show, 180);
  };
  const scheduleClose = () => {
    clearDelay();
    if (pinned.current) return;
    delay.current = setTimeout(() => {
      if (!pointerInside.current && !details?.anchor.contains(document.activeElement)) setDetails(null);
    }, 160);
  };
  const leave = () => { pointerInside.current = false; scheduleClose(); };
  const blur = () => { pinned.current = false; scheduleClose(); };
  const toggle = (entry: ModelEntry, anchor: HTMLElement) => {
    if (details?.anchor === anchor && pinned.current) dismiss();
    else { clearDelay(); pinned.current = true; setDetails({ entry, anchor }); }
  };

  return <Modal open onClose={onClose} size="xl">
    <div className="flex items-center justify-between gap-3 border-b border-slate-500/10 px-6 py-4"><div><h2 className="text-base font-semibold">{title}</h2><p className="mt-1 text-[11px] text-slate-500 dark:text-slate-400">悬浮或点击模型，查看通道参数与能力。</p></div><Pressable type="button" className="glass-icon-button" onClick={onClose} aria-label="关闭模型列表"><X size={18} /></Pressable></div>
    <div className="max-h-[75vh] space-y-6 overflow-y-auto p-6">
      {groups.length ? groups.map((group) => <section key={group.title}><h3 className="text-xs font-semibold text-slate-500 dark:text-slate-400">{group.title}</h3><div className="mt-3 space-y-2.5">{group.entries.map(({ account, model }) => <ModelRow key={`${account.id}-${model.id}`} account={account} model={model}
        infoId={details?.entry.model === model && details.entry.account.id === account.id ? infoId : undefined}
        onPreview={preview} onToggle={toggle} onLeave={leave} onBlur={blur} />)}</div></section>) : <p className="text-xs text-slate-500 dark:text-slate-400">模型目录尚未就绪，刷新账户后查看。</p>}
    </div>
    {details ? <ModelInfoPopover id={infoId} anchor={details.anchor} account={details.entry.account} model={details.entry.model} platformName={platformName(details.entry.account.platform)} onDismiss={dismiss}
      onPointerEnter={() => { pointerInside.current = true; clearDelay(); }} onPointerLeave={leave} /> : null}
  </Modal>;
}

function AccountCard({ account, onDelete, refreshing = false }: { account: CreditAccount; onDelete: () => void; refreshing?: boolean }) {
  const refresh = useCreditsAction(() => creditsApi.refresh(account.id));
  const claim = useCreditsAction(() => creditsApi.claim(account.id));
  const toggle = useCreditsAction((enabled: boolean) => creditsApi.setAccountEnabled(account.id, enabled));
  const snapshot = account.snapshot;
  const models = accountModels(account);
  const credits = snapshot?.rewards.filter((reward) => isCredits(reward.kind)) ?? [];
  const activities = snapshot?.rewards.filter((reward) => reward.kind === "activity") ?? [];
  const otherRewards = snapshot?.rewards.filter((reward) => !isCredits(reward.kind) && reward.kind !== "activity") ?? [];
  const busy = refresh.isPending || claim.isPending || toggle.isPending;
  const [showingModels, setShowingModels] = useState(false);
  return <article className={`glass-card min-w-0 p-5 ${account.enabled ? "" : "opacity-70"}`}>
    <div className="flex items-start justify-between gap-3">
      <div className="min-w-0"><h3 className="truncate text-sm font-semibold">{account.label || platformName(account.platform)}</h3><p className="mt-1 truncate text-xs text-slate-500 dark:text-slate-400">{platformName(account.platform)} · {account.userId || "账户"}</p></div>
      <Pressable type="button" className="glass-icon-button shrink-0" aria-label={`移除账户 ${account.label}`} onClick={onDelete} disabled={busy}><Trash2 size={14} /></Pressable>
    </div>
    <div className="mt-5 flex items-end justify-between gap-2">
      <div><p className="text-[11px] font-medium text-slate-500 dark:text-slate-400">剩余额度</p><p className="mt-2 text-3xl font-semibold tracking-tight tabular-nums">{snapshot?.balance != null ? number(snapshot.balance) : "—"}<span className="ml-2 text-xs font-normal text-slate-500 dark:text-slate-400">{snapshot?.unit === "credits" ? "积分" : snapshot?.unit ?? "积分"}</span></p></div>
      <Badge success={account.enabled && ["active", "ready", "connected", "authenticated"].includes(account.status)}>{!account.enabled ? "已停用" : labels[account.status] ?? account.status}</Badge>
    </div>
    <p role="status" className="mt-2 text-[11px] text-slate-500 dark:text-slate-400">{refreshing || refresh.isPending ? "同步中 · " : ""}{snapshot?.fetchedAt ? `上次成功同步 ${time(snapshot.fetchedAt)}` : "余额待同步"}{snapshot?.stale ? account.lastError ? " · 同步失败，显示历史余额" : " · 余额待更新" : ""}{snapshot?.quota ? ` · ${snapshot.quota}` : ""}</p>
    {account.lastError || snapshot?.diagnostic ? <p className="mt-3 rounded-lg bg-amber-500/10 px-3 py-2 text-xs leading-relaxed text-amber-800 dark:text-amber-200">{account.lastError || snapshot?.diagnostic}</p> : null}
    <div className="mt-4 flex flex-wrap gap-2">
      <Pressable type="button" className="glass-primary-button min-h-8 px-3 py-2 text-xs" disabled={busy || !isClaimable(account)} onClick={() => claim.mutate(undefined, { onSuccess: reportClaims })}>{claim.isPending ? <Loader2 size={13} className="animate-spin" /> : <Coins size={13} />}领取积分</Pressable>
      <Pressable type="button" className="glass-secondary-button min-h-8 px-3 py-2 text-xs" disabled={busy} onClick={() => refresh.mutate(undefined)}><RefreshCw size={13} className={refresh.isPending || refreshing ? "animate-spin" : ""} />刷新</Pressable>
      <Pressable type="button" className="glass-secondary-button min-h-8 px-3 py-2 text-xs" disabled={busy} onClick={() => toggle.mutate(!account.enabled)}>{account.enabled ? "停用" : "启用"}</Pressable>
    </div>
    <div className="mt-4 border-t border-slate-500/10 pt-4">
      <p className="mb-2 text-xs font-medium">积分领取</p>
      {credits.length ? <div className="space-y-2">{credits.map((reward) => <div key={reward.id} className="flex items-start justify-between gap-3 text-xs"><span className="min-w-0 flex-1 break-words">{reward.name}{reward.amount != null ? ` · ${number(reward.amount)}` : ""}</span><span className="shrink-0 whitespace-nowrap text-slate-500 dark:text-slate-400">{labels[reward.status] ?? reward.status}</span></div>)}</div> : <p className="text-xs text-slate-500 dark:text-slate-400">{snapshot ? "当前没有可领取的积分任务。" : "刷新后查看领取资格。"}</p>}
      {account.platform !== "qoder" && activities.length ? <div className="mt-3 rounded-lg bg-sky-500/5 p-2.5"><p className="mb-1 text-[11px] font-medium text-slate-500 dark:text-slate-400">平台活动</p>{activities.map((reward) => <div key={reward.id} className="flex flex-wrap items-center justify-between gap-1 py-0.5"><span className="min-w-0 flex-1 text-[11px] leading-relaxed text-slate-500 dark:text-slate-400">{reward.name} · {labels[reward.status] ?? reward.status}</span>{reward.actionUrl ? <Pressable type="button" className="glass-secondary-button min-h-6 shrink-0 px-2 py-0.5 text-[11px]" onClick={() => void open(reward.actionUrl!).catch(() => toast.error("无法打开活动页"))}>活动详情<ExternalLink size={11} /></Pressable> : null}</div>)}</div> : null}
      {account.platform !== "qoder" && otherRewards.length ? <div className="mt-3 rounded-lg bg-slate-500/5 p-2.5"><p className="mb-1 text-[11px] font-medium text-slate-500 dark:text-slate-400">优惠券及其他奖励</p>{otherRewards.map((reward) => <div key={reward.id} className="flex flex-wrap items-center justify-between gap-1 py-0.5"><span className="min-w-0 flex-1 text-[11px] leading-relaxed text-slate-500 dark:text-slate-400">{reward.name} · {labels[reward.status] ?? reward.status}</span>{reward.actionUrl ? <Pressable type="button" className="glass-secondary-button min-h-6 shrink-0 px-2 py-0.5 text-[11px]" onClick={() => void open(reward.actionUrl!).catch(() => toast.error("无法打开活动页"))}>打开活动页<ExternalLink size={11} /></Pressable> : null}</div>)}</div> : null}
    </div>
    <div className="mt-4 border-t border-slate-500/10 pt-3"><Pressable type="button" className="glass-secondary-button min-h-8 px-3 py-2 text-xs" onClick={() => setShowingModels(true)}>可用模型 · {models.length}</Pressable></div>
    {showingModels ? <ModelsModal title="可用模型" groups={[{ title: account.label || platformName(account.platform), entries: models.map((model) => ({ account, model })) }]} onClose={() => setShowingModels(false)} /> : null}
  </article>;
}

function AccountStack({ accounts, onDelete, refreshingAccountIds = [] }: { accounts: CreditAccount[]; onDelete: (account: CreditAccount) => void; refreshingAccountIds?: string[] }) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const frontCard = useRef<HTMLDivElement>(null);
  const selectedIndex = Math.max(0, accounts.findIndex((account) => account.id === selectedId));
  return <div className="isolate grid" style={{ paddingTop: Math.max(0, accounts.length - 1) * 44 }}>
    {accounts.map((account, index) => {
      const active = index === selectedIndex;
      const depth = active ? 0 : index < selectedIndex ? index + 1 : index;
      const name = account.label || platformName(account.platform);
      const balance = account.snapshot?.balance;
      const unit = account.snapshot?.unit === "credits" ? "积分" : account.snapshot?.unit ?? "积分";
      return <div
        key={account.id}
        className="relative grid min-w-0 origin-top transition-transform duration-300 ease-[var(--motion-spring)] motion-reduce:transition-none [grid-area:1/1]"
        style={{ transform: `translateY(${-depth * 44}px) scale(${Math.max(0.9, 1 - depth * 0.025)})`, zIndex: accounts.length - depth }}
      >
        <div
          ref={active ? frontCard : undefined}
          role="group"
          aria-label={`账户 ${name}`}
          aria-hidden={!active}
          tabIndex={active ? -1 : undefined}
          className={`grid min-w-0 rounded-[var(--radius-card,12px)] ${active ? "bg-[hsl(var(--card))]" : "invisible pointer-events-none"}`}
        >
          <AccountCard account={account} onDelete={() => onDelete(account)} refreshing={refreshingAccountIds.includes(account.id)} />
        </div>
        {!active ? <Pressable
          type="button"
          noScale
          aria-label={`切换到账户 ${name} · ${account.userId || "账户"}`}
          title={`切换到账户 ${name}`}
          onClick={() => {
            setSelectedId(account.id);
            requestAnimationFrame(() => frontCard.current?.focus({ preventScroll: true }));
          }}
          className="absolute inset-x-0 top-0 flex h-14 items-start gap-2 rounded-[var(--radius-card,12px)] border border-slate-200/80 bg-slate-100 px-4 pt-3 text-left text-xs text-slate-600 transition-colors hover:bg-slate-200/80 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300 dark:hover:bg-slate-700"
        >
          <span className="min-w-0 flex-1 truncate font-medium">{name}</span>
          <span className="shrink-0 text-[11px] tabular-nums text-slate-500 dark:text-slate-400">{!account.enabled ? "已停用" : balance != null ? `${number(balance)} ${unit}` : "切换账户"}</span>
          <ArrowUpRight size={13} className="mt-0.5 shrink-0 text-slate-400" aria-hidden="true" />
        </Pressable> : null}
      </div>;
    })}
  </div>;
}

function reportClaims(results: RewardOutcome[]) {
  if (!results.length) { toast("当前没有可领取的积分"); return; }
  const successful = results.filter((result) => ["success", "claimed"].includes(result.status)).length;
  if (successful) toast.success(`已领取 ${successful} 项积分奖励`);
  const failed = results.filter((result) => ["failed", "error"].includes(result.status)).length;
  if (failed) toast.error(`${failed} 项领取失败，请查看领取记录`);
  if (!successful && !failed) toast("领取检查完成，请查看领取记录");
}

function AddAccountModal({ platform: initialPlatform, onClose }: { platform: CreditPlatform; onClose: () => void }) {
  const [platform, setPlatform] = useState<CreditPlatform>(initialPlatform);
  const [flow, setFlow] = useState<OAuthStart | null>(null);
  const [flowError, setFlowError] = useState("");
  const [importError, setImportError] = useState("");
  const [authorizing, setAuthorizing] = useState(false);
  const [importing, setImporting] = useState(false);
  const queryClient = useQueryClient();

  const importLocal = async () => {
    setImporting(true);
    setImportError("");
    try {
      const account = await creditsApi.importLocal(platform);
      await queryClient.invalidateQueries({ queryKey: creditsQueryKey });
      if (account.status !== "ready") {
        setImportError(`账户已保存，但积分同步失败：${account.lastError || account.snapshot?.diagnostic || "平台暂未返回可用积分信息"}`);
        return;
      }
      toast.success("已从本机应用导入账户");
      onClose();
    } catch (error) {
      setImportError(errorMessage(error));
    } finally {
      setImporting(false);
    }
  };

  useEffect(() => {
    if (!flow) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      if (Date.now() >= flow.expiresAt) { setFlow(null); setFlowError("授权已过期，请重新登录"); return; }
      try {
        const result = await creditsApi.oauthPoll(flow.flowId);
        if (cancelled) return;
        if (result.status === "completed") {
          await queryClient.invalidateQueries({ queryKey: creditsQueryKey });
          if (!cancelled) { toast.success("账户已连接"); onClose(); }
          return;
        }
        if (result.status === "failed") { setFlow(null); setFlowError(result.message); return; }
        timer = setTimeout(() => void poll(), 2000);
      } catch (error) {
        if (!cancelled) { setFlow(null); setFlowError(errorMessage(error)); }
      }
    };
    void poll();
    return () => { cancelled = true; clearTimeout(timer); };
  }, [flow, queryClient, onClose]);

  const login = async () => {
    setAuthorizing(true); setFlowError("");
    try {
      const result = await creditsApi.oauthStart(platform);
      await open(result.authorizationUrl);
      setFlow(result);
    } catch (error) { setFlowError(errorMessage(error)); }
    finally { setAuthorizing(false); }
  };
  return <Modal open onClose={onClose} size="xl" dismissOnBackdrop={!authorizing}>
    <div className="flex items-center justify-between border-b border-slate-500/10 px-6 py-4"><h2 className="text-base font-semibold">连接平台账户</h2><Pressable type="button" className="glass-icon-button" onClick={onClose} disabled={authorizing} aria-label="关闭账户表单"><X size={18} /></Pressable></div>
    <div className="max-h-[75vh] space-y-5 overflow-y-auto p-6">
      <fieldset>
        <legend className="mb-2 text-xs font-medium">平台</legend>
        <div className="flex flex-wrap gap-3">
          {platforms.map((item) => {
            const selected = platform === item.id;
            return <Pressable
              key={item.id}
              type="button"
              aria-label={item.name}
              aria-pressed={selected}
              title={item.name}
              disabled={Boolean(flow) || authorizing || importing}
              onClick={() => { setPlatform(item.id); setFlowError(""); setImportError(""); }}
              className={`relative inline-flex h-14 w-14 items-center justify-center rounded-xl border transition-colors disabled:cursor-not-allowed disabled:opacity-50 ${selected ? "border-blue-400 bg-blue-500/10 shadow-sm dark:border-sky-300/60 dark:bg-sky-300/10" : "border-white/55 bg-white/50 hover:bg-white/75 dark:border-white/10 dark:bg-white/5 dark:hover:bg-white/10"}`}
            >
              <AgentIcon id={item.id} name={item.name} className="h-8 w-8" />
              {selected ? <CheckCircle2 size={14} aria-hidden="true" className="absolute -right-1 -top-1 rounded-full bg-white text-blue-600 dark:bg-slate-900 dark:text-sky-300" /> : null}
            </Pressable>;
          })}
        </div>
      </fieldset>
      <div className="rounded-xl bg-blue-500/5 p-4"><p className="text-sm font-medium">在浏览器中完成授权</p><p className="mt-2 text-xs leading-relaxed text-slate-500 dark:text-slate-400">授权完成后自动连接账户。</p>{flow ? <div className="mt-4 flex flex-wrap items-center gap-3"><Loader2 size={16} className="animate-spin text-blue-500" /><span className="text-xs">等待授权 · {time(flow.expiresAt)} 到期</span><Pressable type="button" className="glass-secondary-button min-h-7 px-2 py-1 text-xs" onClick={() => { setFlow(null); setFlowError(""); }}>取消</Pressable></div> : <Pressable type="button" className="glass-primary-button mt-4 text-xs" onClick={() => void login()} disabled={authorizing}>{authorizing ? <Loader2 size={14} className="animate-spin" /> : <ExternalLink size={14} />}前往 {platformName(platform)} 登录</Pressable>}{flowError ? <p role="alert" className="mt-3 break-words text-xs text-amber-700 dark:text-amber-200">{flowError}</p> : null}</div>
      {platform === "traework" ? <div className="rounded-xl bg-blue-500/5 p-4"><p className="text-sm font-medium">从本机 TRAE CN 应用一键导入</p><p className="mt-2 text-xs leading-relaxed text-slate-500 dark:text-slate-400">优先读取本机 TRAE CN 桌面登录信息，兼容 CLI 凭据；无需复制 Token。</p><Pressable type="button" className="glass-secondary-button mt-4 text-xs" disabled={importing || authorizing} onClick={() => void importLocal()}>{importing ? <Loader2 size={13} className="animate-spin" /> : <ShieldCheck size={13} />}一键导入本机凭据</Pressable>{importError ? <p role="alert" className="mt-3 break-words text-xs text-amber-700 dark:text-amber-200">{importError}</p> : null}</div> : null}
    </div>
  </Modal>;
}

function AccountsView({ data, onAdd }: { data: CreditDashboard; onAdd: (platform: CreditPlatform) => void }) {
  const refresh = useCreditsAction(() => creditsApi.refresh());
  const claim = useCreditsAction(() => creditsApi.claim());
  const remove = useCreditsAction(creditsApi.deleteAccount);
  const [deleting, setDeleting] = useState<CreditAccount | null>(null);
  const [autoClaim, setAutoClaim] = useState(data.settings.autoClaim);
  const [autoRefresh, setAutoRefresh] = useState(data.settings.autoRefresh ?? true);
  const save = useCreditsAction(creditsApi.saveSettings);
  const changed = autoClaim !== data.settings.autoClaim || autoRefresh !== (data.settings.autoRefresh ?? true);
  useEffect(() => { setAutoClaim(data.settings.autoClaim); }, [data.settings.autoClaim]);
  useEffect(() => { setAutoRefresh(data.settings.autoRefresh ?? true); }, [data.settings.autoRefresh]);
  return <div className="space-y-6">
    <div className="flex flex-wrap items-center justify-between gap-3"><p className="text-xs text-slate-500 dark:text-slate-400">各平台分别展示原生额度，领取结果逐项记录。</p><div className="flex flex-wrap gap-2"><Pressable type="button" className="glass-secondary-button text-xs" disabled={refresh.isPending || !data.accounts.length} onClick={() => refresh.mutate(undefined)}><RefreshCw size={14} className={refresh.isPending ? "animate-spin" : ""} />刷新余额</Pressable><Pressable type="button" className="glass-primary-button text-xs" disabled={claim.isPending || !data.accounts.some(isClaimable)} onClick={() => claim.mutate(undefined, { onSuccess: reportClaims })}>{claim.isPending ? <Loader2 size={14} className="animate-spin" /> : <Coins size={14} />}一键领取积分</Pressable></div></div>
    <div className="grid grid-cols-1 items-start gap-4 lg:grid-cols-3">
      {platforms.map((platform) => {
        const accounts = data.accounts.filter((account) => account.platform === platform.id);
        return <section key={platform.id} className="min-w-0 space-y-3">
          <div className="flex items-center gap-2 px-1"><AgentIcon id={platform.id} name={platform.name} className="h-7 w-7" /><h2 className="text-sm font-semibold">{platform.name}</h2><span className="ml-auto text-xs text-slate-500 dark:text-slate-400">{accounts.length} 个账户</span></div>
          {accounts.length ? <AccountStack accounts={accounts} onDelete={setDeleting} refreshingAccountIds={data.refreshingAccountIds} /> : <div className="glass-card flex min-h-[260px] flex-col items-center justify-center p-6 text-center"><KeyRound size={25} className="text-slate-400" /><p className="mt-4 text-sm font-medium">连接 {platform.name}</p><p className="mt-2 text-xs leading-relaxed text-slate-500 dark:text-slate-400">登录后查看余额、领取资格与模型目录。</p><Pressable type="button" className="glass-secondary-button mt-5 text-xs" onClick={() => onAdd(platform.id)}><Plus size={14} />添加账户</Pressable></div>}
        </section>;
      })}
    </div>
    <section className="glass-card p-5"><h2 className="text-sm font-semibold">同步与领取设置</h2>
      <label aria-label="自动刷新余额" htmlFor="credits-auto-refresh" className="mt-4 flex items-start gap-3"><input id="credits-auto-refresh" type="checkbox" className="mt-0.5 h-4 w-4 accent-blue-500" checked={autoRefresh} onChange={(event) => setAutoRefresh(event.target.checked)} disabled={save.isPending} /><span><span className="block text-sm font-medium">自动刷新余额</span><span className="mt-1 block text-xs leading-relaxed text-slate-500 dark:text-slate-400">默认开启。查看本页时每 5 分钟同步，其他页面或托盘运行时每 15 分钟同步；代理调用后自动补刷。退出应用后停止。</span></span></label>
      <label aria-label="自动领取可用积分" htmlFor="credits-auto-claim" className="mt-4 flex items-start gap-3"><input id="credits-auto-claim" type="checkbox" className="mt-0.5 h-4 w-4 accent-blue-500" checked={autoClaim} onChange={(event) => setAutoClaim(event.target.checked)} disabled={save.isPending} /><span><span className="block text-sm font-medium">自动领取可用积分</span><span className="mt-1 block text-xs leading-relaxed text-slate-500 dark:text-slate-400">默认关闭。开启后仅在 AI Toolkit 运行期间，按北京时间每天 10:30 检查并领取；当天 10:30 后启动会补做检查，已领取任务会跳过。</span></span></label>
      <Pressable type="button" className="glass-secondary-button mt-4 text-xs" disabled={!changed || save.isPending} onClick={() => save.mutate({ port: data.settings.port, autoClaim, autoRefresh }, { onSuccess: () => toast.success("设置已保存") })}>{save.isPending ? <Loader2 size={13} className="animate-spin" /> : null}保存设置</Pressable>
    </section>
    <section className="glass-card p-5"><h2 className="text-sm font-semibold">最近领取</h2>{data.rewards.length ? <div className="mt-4 divide-y divide-slate-500/10">{data.rewards.slice(0, 12).map((outcome) => <div key={`${outcome.accountId}-${outcome.rewardId}-${outcome.createdAt}-${outcome.status}`} className="flex flex-wrap items-center justify-between gap-3 py-3 text-xs"><div className="min-w-0"><p className="font-medium">{outcome.name}{outcome.amount != null && isCredits(outcome.kind) ? ` · +${number(outcome.amount)}` : ""}</p><p className="mt-1 break-words text-slate-500 dark:text-slate-400">{data.accounts.find((account) => account.id === outcome.accountId)?.label || "账户"} · {outcome.message}</p></div><div className="flex shrink-0 items-center gap-3"><span className="text-[11px] text-slate-500 dark:text-slate-400">{time(outcome.createdAt)}</span><Badge success={["success", "claimed"].includes(outcome.status)}>{outcome.status === "pending" ? "待确认" : labels[outcome.status] ?? outcome.status}</Badge></div></div>)}</div> : <p className="mt-3 text-xs text-slate-500 dark:text-slate-400">还没有领取记录。</p>}</section>
    {deleting ? <Modal open onClose={() => !remove.isPending && setDeleting(null)} size="sm" dismissOnBackdrop={!remove.isPending}><div className="p-6"><h2 className="text-base font-semibold">移除账户</h2><p className="mt-3 break-words text-sm leading-relaxed text-slate-500 dark:text-slate-400">移除“{deleting.label || platformName(deleting.platform)}”及本地保存的凭据，后续代理请求将不再使用此账户。</p><div className="mt-6 flex justify-end gap-2"><Pressable type="button" className="glass-secondary-button text-xs" disabled={remove.isPending} onClick={() => setDeleting(null)}>取消</Pressable><Pressable type="button" className="glass-danger-button text-xs" disabled={remove.isPending} onClick={() => remove.mutate(deleting.id, { onSuccess: () => setDeleting(null) })}>{remove.isPending ? <Loader2 size={13} className="animate-spin" /> : <Trash2 size={13} />}移除</Pressable></div></div></Modal> : null}
  </div>;
}

function ProxyView({ data }: { data: CreditDashboard }) {
  const start = useCreditsAction(creditsApi.startProxy);
  const stop = useCreditsAction(creditsApi.stopProxy);
  const probe = useCreditsAction(creditsApi.probeProxy);
  const models = availableModels(data);
  const [showingCatalog, setShowingCatalog] = useState(false);
  const catalogGroups: ModelGroup[] = platforms
    .map((platform) => ({ title: platform.name, entries: models.filter(({ account }) => account.platform === platform.id) }))
    .filter((group) => group.entries.length);
  return <div className="space-y-5">
    <section className="glass-card p-6"><div className="flex flex-wrap items-center justify-between gap-4"><div className="flex items-center gap-3"><span className={`flex h-11 w-11 items-center justify-center rounded-xl ${data.gateway.running ? "bg-emerald-500/10 text-emerald-600 dark:text-emerald-300" : "bg-slate-500/10 text-slate-500"}`}><Server size={22} /></span><div><h2 className="text-base font-semibold">本地模型代理</h2><p className="mt-1 text-xs text-slate-500 dark:text-slate-400">{data.gateway.running ? "正在运行 · 关闭窗口后继续运行" : "已停止 · 手动开启后供其他工具使用"}</p></div></div><Pressable type="button" className={data.gateway.running ? "glass-secondary-button text-sm" : "glass-primary-button text-sm"} disabled={start.isPending || stop.isPending} onClick={() => data.gateway.running ? stop.mutate(undefined) : start.mutate(undefined)}>{start.isPending || stop.isPending ? <Loader2 size={15} className="animate-spin" /> : <Power size={15} />}{data.gateway.running ? "停止代理" : "一键开启代理"}</Pressable></div>
      <p className="mt-3 text-xs leading-5 text-slate-500 dark:text-slate-400">关闭窗口后应用留在系统托盘，可从托盘恢复主窗口；选择“退出应用（停止代理）”会停止代理并完全退出。</p>
      {data.gateway.lastError ? <p role="alert" className="mt-4 rounded-lg bg-amber-500/10 p-3 text-xs text-amber-800 dark:text-amber-200">{data.gateway.lastError}</p> : null}
      <p className="mt-5 text-xs leading-relaxed text-slate-500 dark:text-slate-400">在客户端的自定义提供商设置中填写下方连接参数，再添加一个目录模型 ID。保持 AI Toolkit 与代理运行即可使用。</p>
      <div className="mt-5 grid gap-5 md:grid-cols-2"><CopyField label="提供商名称" value="AI Toolkit" /><CopyField label="Base URL" value={data.gateway.baseUrl} /></div>
      <div className="mt-5 grid gap-5 md:grid-cols-2"><div className="space-y-2"><p className="text-xs font-medium text-slate-500 dark:text-slate-400">协议</p><div className="glass-code space-y-2 rounded-xl px-3 py-2.5 text-xs"><p>OpenAI Chat Completions <code className="block break-all text-[11px] text-slate-500 dark:text-slate-400">/v1/chat/completions</code></p><p>Anthropic Messages <code className="block break-all text-[11px] text-slate-500 dark:text-slate-400">/v1/messages</code></p></div><p className="text-[11px] leading-relaxed text-slate-500 dark:text-slate-400">两种协议使用同一 Base URL，在客户端选择对应协议。</p></div><div className="space-y-2"><p className="text-xs font-medium text-slate-500 dark:text-slate-400">API Key</p><div className="glass-code flex min-h-[44px] items-center justify-between gap-3 rounded-xl px-3 py-2.5"><span className="text-xs text-slate-500 dark:text-slate-400">使用本地代理密钥</span><CopyProxyKey /></div></div></div>
      <div className="mt-4 flex flex-wrap gap-2"><Pressable type="button" className="glass-secondary-button text-xs" disabled={!data.gateway.running || probe.isPending} onClick={() => probe.mutate(undefined, { onSuccess: (message) => toast.success(message) })}>{probe.isPending ? <Loader2 size={13} className="animate-spin" /> : <CheckCircle2 size={13} />}检测连接</Pressable></div>
    </section>
    <section className="glass-card p-6"><div className="flex flex-wrap items-start justify-between gap-4"><div className="min-w-0"><h2 className="text-sm font-semibold">目录模型 ID</h2><p className="mt-2 text-xs leading-relaxed text-slate-500 dark:text-slate-400">平台模型 ID 来自已连接账户的目录，可以指定对应平台调用；上游推理的可用性以实际请求结果为准。</p></div><Pressable type="button" className="glass-secondary-button shrink-0 text-xs" onClick={() => setShowingCatalog(true)}>查看模型 · {models.length}</Pressable></div><p className="mt-4 text-[11px] leading-relaxed text-slate-500 dark:text-slate-400">带平台前缀的 ID 指定对应平台，弹窗内按应用分组展示，可逐个复制。</p>{showingCatalog ? <ModelsModal title="目录模型 ID" groups={catalogGroups} onClose={() => setShowingCatalog(false)} /> : null}</section>
    <section className="glass-card p-6"><h2 className="text-sm font-semibold">最近请求</h2>{data.requests.length ? <div className="mt-3 overflow-x-auto"><table className="w-full text-left text-xs"><thead className="text-[11px] text-slate-500 dark:text-slate-400"><tr><th className="py-2 pr-4 font-medium">模型 / 平台</th><th className="py-2 pr-4 font-medium">耗时</th><th className="py-2 pr-4 font-medium">Tokens 输入 / 输出</th><th className="py-2 pr-4 font-medium">状态</th><th className="py-2 font-medium">时间</th></tr></thead><tbody>{data.requests.slice(0, 15).map((request) => <tr key={request.id} className="border-t border-slate-500/10"><td className="max-w-[260px] py-3 pr-4"><p className="break-all font-medium">{request.model}</p><p className="mt-1 text-[11px] text-slate-500 dark:text-slate-400">{platformName(request.platform)} · {data.accounts.find((account) => account.id === request.accountId)?.label || request.accountId}</p></td><td className="whitespace-nowrap py-3 pr-4 tabular-nums">{number(request.elapsedMs)} ms</td><td className="whitespace-nowrap py-3 pr-4 tabular-nums">{request.inputTokens == null ? "—" : number(request.inputTokens)} / {request.outputTokens == null ? "—" : number(request.outputTokens)}</td><td className="py-3 pr-4"><Badge success={["success", "completed"].includes(request.status)}>{labels[request.status] ?? request.status}</Badge></td><td className="whitespace-nowrap py-3 text-[11px] text-slate-500 dark:text-slate-400">{time(request.createdAt)}</td></tr>)}</tbody></table></div> : <p className="mt-3 text-xs text-slate-500 dark:text-slate-400">开启代理并从客户端调用后，显示平台、耗时和用量。</p>}</section>
  </div>;
}

function CopyProxyKey() {
  const [copying, setCopying] = useState(false);
  const copy = async () => {
    setCopying(true);
    try { await copyValue(await creditsApi.proxyKey(), "代理 API Key"); }
    catch (error) { toast.error(errorMessage(error)); }
    finally { setCopying(false); }
  };
  return <Pressable type="button" className="glass-secondary-button min-h-7 shrink-0 px-2 py-1 text-xs" disabled={copying} onClick={() => void copy()}>{copying ? <Loader2 size={12} className="animate-spin" /> : <Copy size={12} />}复制 API Key</Pressable>;
}

export default function CreditsPanel() {
  const [tab, setTab] = useState<PanelTab>("accounts");
  const [adding, setAdding] = useState<CreditPlatform | null>(null);
  const dashboard = useCreditsDashboard();
  const closeAdd = useCallback(() => setAdding(null), []);
  const moveTab = (key: string) => {
    const current = sections.findIndex((section) => section.id === tab);
    const index = key === "ArrowRight" ? (current + 1) % sections.length
      : key === "ArrowLeft" ? (current + sections.length - 1) % sections.length
      : key === "Home" ? 0 : key === "End" ? sections.length - 1 : -1;
    if (index < 0) return false;
    const next = sections[index].id;
    setTab(next);
    document.getElementById(`credits-tab-${next}`)?.focus();
    return true;
  };
  return <div className="glass-app flex h-full flex-col overflow-hidden">
    <div className="glass-header"><div className="flex flex-wrap items-start justify-between gap-4"><div><div className="glass-kicker"><Coins size={13} />Credits & Models</div><h1 className="mt-3 text-display">积分与代理</h1><p className="mt-2 text-caption text-slate-500 dark:text-slate-400">管理平台积分，将平台目录模型接入你的编程工具。</p></div><Pressable type="button" className="glass-primary-button mt-1 text-xs" onClick={() => setAdding("workbuddy")}><Plus size={15} />添加账户</Pressable></div><div className="mt-5 flex flex-wrap gap-2" role="tablist" aria-label="积分与代理功能">{sections.map((item) => <Pressable key={item.id} id={`credits-tab-${item.id}`} type="button" role="tab" aria-selected={tab === item.id} aria-controls="credits-tab-panel" tabIndex={tab === item.id ? 0 : -1} onKeyDown={(event) => { if (moveTab(event.key)) event.preventDefault(); }} className={tab === item.id ? "glass-primary-button min-h-8 px-4 py-2 text-xs" : "glass-secondary-button min-h-8 px-4 py-2 text-xs"} onClick={() => setTab(item.id)}>{item.label}</Pressable>)}</div></div>
    <div className="glass-content" role="tabpanel" id="credits-tab-panel" aria-labelledby={`credits-tab-${tab}` }>
      {dashboard.isLoading ? <div className="flex min-h-[280px] items-center justify-center gap-3 text-sm text-slate-500"><Loader2 size={20} className="animate-spin" />正在读取账户与代理状态…</div> : dashboard.isError ? <div className="glass-card p-6"><h2 className="text-sm font-semibold">读取失败</h2><p role="alert" className="mt-3 break-words text-xs text-slate-500 dark:text-slate-400">{errorMessage(dashboard.error)}</p><Pressable type="button" className="glass-secondary-button mt-4 text-xs" onClick={() => void dashboard.refetch()} disabled={dashboard.isFetching}><RefreshCw size={13} className={dashboard.isFetching ? "animate-spin" : ""} />重试</Pressable></div> : dashboard.data ? tab === "accounts" ? <AccountsView data={dashboard.data} onAdd={setAdding} /> : <ProxyView data={dashboard.data} /> : null}
    </div>
    {adding ? <AddAccountModal platform={adding} onClose={closeAdd} /> : null}
  </div>;
}
