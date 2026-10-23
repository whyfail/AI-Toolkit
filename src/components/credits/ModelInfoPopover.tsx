import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Check, CircleHelp, FileText, Image, Minus, Type, Video } from "lucide-react";
import type { CreditAccount, CreditModel } from "@/lib/api/credits";

interface ModelInfoProps {
  account: CreditAccount;
  model: CreditModel;
  platformName: string;
}

const tokenNumber = new Intl.NumberFormat("en-US");
const compactTokens = new Intl.NumberFormat("en-US", { notation: "compact", maximumFractionDigits: 2 });
const effortLabels: Record<string, string> = {
  none: "关闭", off: "关闭", disabled: "关闭", minimal: "最低", light: "轻", low: "低",
  medium: "中", high: "高", extra_high: "极高", xhigh: "极高", max: "最高",
};
const effortOrder = ["none", "off", "disabled", "minimal", "light", "low", "medium", "high", "extra_high", "xhigh", "max"];
const effortLabel = (effort: string) => typeof effortLabels[effort] === "string" ? `${effortLabels[effort]}（${effort}）` : effort;

function TokenLimit({ label, value }: { label: string; value?: number | null }) {
  const known = value != null && Number.isSafeInteger(value) && value > 0;
  return <div className="rounded-xl bg-slate-500/5 px-3 py-3">
    <dt className="text-[11px] text-slate-500 dark:text-slate-400">{label}</dt>
    <dd className="mt-1.5 text-xl font-semibold tracking-tight tabular-nums">{known ? compactTokens.format(value) : "未知"}
      <span className="mt-1 block text-[10px] font-normal tracking-normal text-slate-500 dark:text-slate-400">{known ? `${tokenNumber.format(value)} Token` : "通道未提供"}</span>
    </dd>
  </div>;
}

function Capability({ label, value, note }: { label: string; value?: boolean | null; note?: string }) {
  const Icon = value === true ? Check : value === false ? Minus : CircleHelp;
  return <div className="flex items-center justify-between gap-3 rounded-lg border border-slate-500/10 px-2.5 py-2 text-[11px]">
    <span className="text-slate-600 dark:text-slate-300">{label}</span>
    <span className={`inline-flex shrink-0 items-center gap-1 ${value === true ? "text-emerald-700 dark:text-emerald-300" : "text-slate-500 dark:text-slate-400"}`}><Icon size={12} aria-hidden="true" />{note ?? (value === true ? "支持" : value === false ? "不支持" : "未知")}</span>
  </div>;
}

export function ModelInfoContent({ account, model, platformName }: ModelInfoProps) {
  const snapshot = account.snapshot;
  const efforts = [...(model.reasoningEffortOptions ?? [])].sort((a, b) => {
    const rank = (value: string) => effortOrder.includes(value) ? effortOrder.indexOf(value) : effortOrder.length;
    return rank(a) - rank(b);
  });
  const modelsFetchedAt = snapshot?.sync?.modelsFetchedAt ?? snapshot?.fetchedAt;
  const updated = modelsFetchedAt ? new Date(modelsFetchedAt).toLocaleString("zh-CN", { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit" }) : "尚未同步";
  const modelsStale = snapshot?.sync?.modelsFetchedAt != null ? !!snapshot.sync.modelsError : snapshot?.stale;
  return <>
    <div className="flex items-center justify-between gap-3 text-[10px] font-medium text-slate-500 dark:text-slate-400"><span>{platformName} · 通道参数</span>{modelsStale ? <span className="rounded-full bg-amber-500/10 px-2 py-0.5 text-amber-700 dark:text-amber-300">缓存数据</span> : null}</div>
    <h3 className="mt-2 break-words text-base font-semibold leading-snug">{model.name || model.id}</h3>
    <p className="mt-1 break-all font-mono text-[10px] leading-relaxed text-slate-500 dark:text-slate-400">{account.platform}/{model.id}</p>

    <dl className="mt-3 space-y-2 rounded-xl border border-slate-500/10 px-3 py-2.5 text-[10px]">
      <div>
        <dt className="text-slate-500 dark:text-slate-400">调用 ID</dt>
        <dd className="mt-1 select-text break-all font-mono leading-relaxed">{model.upstreamId || model.id}</dd>
      </div>
      <div className="border-t border-slate-500/10 pt-2">
        <dt className="flex items-center justify-between gap-2 text-slate-500 dark:text-slate-400"><span>底层模型 ID</span>{model.upstreamModelIds?.length ? <span>目录声明</span> : null}</dt>
        <dd className="mt-1 leading-relaxed">{model.upstreamModelIds?.length
          ? model.upstreamModelIds.map((id) => <code key={id} className="block select-text break-all font-mono">{id}</code>)
          : <span className="text-slate-500 dark:text-slate-400">通道未公开具体版本 ID</span>}</dd>
      </div>
    </dl>

    <dl className="mt-4 grid grid-cols-2 gap-2">
      <TokenLimit label="上下文窗口（默认最大）" value={model.contextWindow} />
      <TokenLimit label="最大输出" value={model.maxOutputTokens} />
    </dl>

    <div className="mt-4">
      <p className="text-[11px] font-medium">代理输入类型</p>
      <div className="mt-2 grid grid-cols-2 gap-2">
        {[
          { label: "文本", Icon: Type, value: true },
          { label: "图片", Icon: Image, value: model.supportsImages },
          { label: "视频", Icon: Video, value: false, note: "代理未开放" },
          { label: "PDF", Icon: FileText, value: false, note: "代理未开放" },
        ].map(({ label, Icon, value, note }) => <div key={label} className="flex items-center gap-1.5 rounded-lg bg-slate-500/5 px-2.5 py-2 text-[11px]">
          <Icon size={13} className="shrink-0 text-slate-400" aria-hidden="true" /><span>{label}</span>
          <span className={`ml-auto text-[10px] ${value === true ? "text-emerald-700 dark:text-emerald-300" : "text-slate-500 dark:text-slate-400"}`}>{note ?? (value === true ? "支持" : value === false ? "不支持" : "未知")}</span>
        </div>)}
      </div>
    </div>

    <div className="mt-3 grid grid-cols-2 gap-2">
      <Capability label="工具调用" value={model.supportsTools} />
      <Capability label="思考输出" value={model.supportsReasoning} />
    </div>
    <dl className="mt-3 rounded-xl border border-slate-500/10 px-3 py-2.5 text-[11px]">
      <dt className="font-medium">推理等级</dt>
      <dd className="mt-2 flex flex-wrap gap-1.5">
        {efforts.length ? efforts.map((effort) => <span key={effort} className="max-w-full break-all rounded-md bg-slate-500/5 px-2 py-1 text-slate-600 dark:text-slate-300">{effortLabel(effort)}</span>)
          : <span className="text-slate-500 dark:text-slate-400">未知 · 通道未提供</span>}
      </dd>
      {model.defaultReasoningEffort ? <dd className="mt-2 text-[10px] text-slate-500 dark:text-slate-400">默认：{effortLabel(model.defaultReasoningEffort)}</dd> : null}
    </dl>
    <div className="mt-4 border-t border-slate-500/10 pt-3 text-[10px] leading-relaxed text-slate-500 dark:text-slate-400">
      <p>参数与能力来自当前账号通道。未知表示目录未提供，刷新账户可更新；目录标注不等于已实测。</p>
      <p className="mt-1">更新于 {updated}</p>
    </div>
  </>;
}

export function modelInfoPosition(
  anchor: Pick<DOMRect, "left" | "right" | "top" | "bottom">,
  panel: { width: number; height: number },
  viewport: { width: number; height: number },
) {
  const margin = 12;
  const gap = 10;
  const maxHeight = Math.max(0, viewport.height - margin * 2);
  const height = Math.min(panel.height, maxHeight);
  const width = Math.min(panel.width, Math.max(0, viewport.width - margin * 2));
  let left = anchor.right + gap;
  let top = anchor.top;
  if (left + width > viewport.width - margin) {
    if (anchor.left - gap - width >= margin) {
      left = anchor.left - gap - width;
    } else {
      left = anchor.left;
      top = anchor.bottom + gap + height <= viewport.height - margin
        ? anchor.bottom + gap : anchor.top - gap - height;
    }
  }
  return {
    left: Math.max(margin, Math.min(left, viewport.width - width - margin)),
    top: Math.max(margin, Math.min(top, viewport.height - height - margin)),
    maxHeight,
  };
}

interface ModelInfoPopoverProps extends ModelInfoProps {
  id: string;
  anchor: HTMLElement;
  onDismiss: () => void;
  onPointerEnter: () => void;
  onPointerLeave: () => void;
}

export function ModelInfoPopover({ id, anchor, onDismiss, onPointerEnter, onPointerLeave, ...info }: ModelInfoPopoverProps) {
  const panel = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState<ReturnType<typeof modelInfoPosition> | null>(null);

  useLayoutEffect(() => {
    if (!panel.current) return;
    setPosition(modelInfoPosition(anchor.getBoundingClientRect(), panel.current.getBoundingClientRect(), { width: window.innerWidth, height: window.innerHeight }));
  }, [anchor, info.model, info.account.snapshot?.fetchedAt]);

  useEffect(() => {
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Node && !panel.current?.contains(event.target) && !anchor.contains(event.target)) onDismiss();
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape") { event.stopPropagation(); onDismiss(); }
    };
    const scroll = (event: Event) => {
      if (event.target instanceof Node && panel.current?.contains(event.target)) return;
      onDismiss();
    };
    window.addEventListener("pointerdown", outside, true);
    window.addEventListener("keydown", escape, true);
    window.addEventListener("scroll", scroll, true);
    window.addEventListener("resize", onDismiss);
    return () => {
      window.removeEventListener("pointerdown", outside, true);
      window.removeEventListener("keydown", escape, true);
      window.removeEventListener("scroll", scroll, true);
      window.removeEventListener("resize", onDismiss);
    };
  }, [anchor, onDismiss]);

  if (typeof document === "undefined") return null;
  return createPortal(<div
    ref={panel}
    id={id}
    role="tooltip"
    className="glass-modal fixed z-[80] w-80 max-w-[calc(100vw-24px)] overflow-y-auto rounded-2xl p-4 text-slate-900 dark:text-slate-100"
    style={position ?? { left: 0, top: 0, maxHeight: "calc(100vh - 24px)", visibility: "hidden" }}
    onPointerEnter={onPointerEnter}
    onPointerLeave={onPointerLeave}
  ><ModelInfoContent {...info} /></div>, document.body);
}
