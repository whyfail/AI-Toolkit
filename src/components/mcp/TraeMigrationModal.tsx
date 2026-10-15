import { useEffect, useState } from "react";
import { AlertCircle, CheckCircle2, Loader2, RefreshCw, Undo2, X } from "lucide-react";
import { toast } from "sonner";
import { mcpApi, type TraeMigrationOutcome, type TraeMigrationPreview, type TraeMigrationSelection } from "@/lib/api";
import { Modal } from "@/components/ui/Modal";
import { Pressable } from "@/components/ui/Pressable";

interface TraeMigrationModalProps {
  open: boolean;
  onClose: () => void;
  onChanged: () => void;
}

const errorMessage = (error: unknown) => error instanceof Error ? error.message : String(error);

export default function TraeMigrationModal({ open, onClose, onChanged }: TraeMigrationModalProps) {
  const [preview, setPreview] = useState<TraeMigrationPreview | null>(null);
  const [selected, setSelected] = useState<Set<string>>(() => new Set());
  const [renames, setRenames] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(false);
  const [applying, setApplying] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const [error, setError] = useState("");
  const [result, setResult] = useState<TraeMigrationOutcome | null>(null);
  const [scanVersion, setScanVersion] = useState(0);
  const busy = loading || applying || restoring;

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setLoading(true);
    setPreview(null);
    setError("");
    void mcpApi.previewTraeMigration().then((data) => {
      if (cancelled) return;
      setPreview(data);
      setSelected(new Set(data.entries.filter((entry) => !entry.conflict).map((entry) => entry.key)));
      setRenames({});
    }).catch((failure: unknown) => {
      if (!cancelled) setError(errorMessage(failure));
    }).finally(() => {
      if (!cancelled) setLoading(false);
    });
    return () => { cancelled = true; };
  }, [open, scanVersion]);

  const selectedEntries = preview?.entries.filter((entry) => selected.has(entry.key)) ?? [];
  const invalidRename = selectedEntries.some((entry) => entry.conflict &&
    (!(renames[entry.key] ?? "").trim() || renames[entry.key].trim() === entry.serverId));

  const toggleEntry = (key: string) => {
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const apply = async () => {
    if (!preview || busy || !selectedEntries.length) return;
    if (invalidRename) {
      toast.error("请为冲突配置填写不同的新服务器 ID");
      return;
    }
    const selections: TraeMigrationSelection[] = selectedEntries.map((entry) => ({
      key: entry.key,
      renameTo: entry.conflict ? renames[entry.key].trim() : null,
    }));
    setApplying(true);
    setError("");
    try {
      const migrated = await mcpApi.applyTraeMigration(preview.revision, selections);
      setResult(migrated);
      toast.success(`TRAE 配置迁移完成，导入 ${migrated.imported} 项`);
      onChanged();
      setScanVersion((version) => version + 1);
    } catch (failure) {
      setError(errorMessage(failure));
    } finally {
      setApplying(false);
    }
  };

  const restore = async () => {
    if (!result || busy) return;
    setRestoring(true);
    setError("");
    try {
      await mcpApi.restoreTraeMigration(result.backupId);
      setResult(null);
      toast.success("已撤销本次 TRAE 配置迁移");
      onChanged();
      setScanVersion((version) => version + 1);
    } catch (failure) {
      setError(errorMessage(failure));
    } finally {
      setRestoring(false);
    }
  };

  const close = () => { if (!busy) onClose(); };

  return <Modal open={open} onClose={close} size="full" dismissOnBackdrop={!busy}>
    <div className="flex max-h-[90vh] flex-col">
      <div className="flex items-center justify-between gap-3 border-b border-slate-500/10 px-4 py-4 sm:px-6">
        <h2 className="text-base font-semibold">迁移旧 TRAE MCP 配置</h2>
        <Pressable type="button" className="glass-icon-button" onClick={close} disabled={busy} aria-label="关闭 TRAE 迁移弹窗"><X size={18} /></Pressable>
      </div>
      <div className="space-y-4 overflow-y-auto px-4 py-4 sm:px-6">
        <p className="text-xs leading-relaxed text-slate-500 dark:text-slate-400">将本机旧 TRAE Work、TRAE Work CN 或 TRAE SOLO CN 的 MCP 配置迁移到同地区新版 TRAE。新版 Agent 与 IDE 共享本地配置，此操作不涉及云端 Agent。迁移前自动备份，旧来源文件保留。</p>
        {result ? <section className="rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-4" aria-live="polite">
          <p className="flex items-center gap-2 text-sm font-medium"><CheckCircle2 size={16} className="text-emerald-600" />迁移完成 · 已导入 {result.imported} 项</p>
          <p className="mt-2 break-all text-xs text-slate-500 dark:text-slate-400">备份位置：{result.backupPath}</p>
          <p className="mt-2 text-xs leading-relaxed text-slate-500 dark:text-slate-400">可撤销最近这次迁移。若迁移后配置已被编辑，将保留新工作并拒绝覆盖。</p>
          <Pressable type="button" className="glass-secondary-button mt-3 text-xs" onClick={() => void restore()} disabled={busy}>{restoring ? <Loader2 size={14} className="animate-spin" /> : <Undo2 size={14} />}撤销本次迁移</Pressable>
        </section> : null}
        {error ? <p role="alert" className="flex items-start gap-2 rounded-xl bg-amber-500/10 p-3 text-xs leading-relaxed text-amber-800 dark:text-amber-200"><AlertCircle size={15} className="mt-0.5 shrink-0" /><span className="break-words">{error}</span></p> : null}
        {loading ? <div role="status" className="flex min-h-40 items-center justify-center gap-2 text-sm text-slate-500"><Loader2 size={18} className="animate-spin" />正在预览本机旧配置…</div> : preview?.entries.length ? <div className="space-y-3">
          <p className="text-xs text-slate-500 dark:text-slate-400">选择要迁移的配置。同 ID 且配置相同的记录自动去重；配置不同的记录需使用新 ID。</p>
          {preview.entries.map((entry, index) => {
            const checked = selected.has(entry.key);
            const renameId = `trae-migration-rename-${index}`;
            return <article key={entry.key} className="glass-card min-w-0 p-4">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <label className="flex min-w-0 cursor-pointer items-center gap-2 text-sm font-medium">
                  <input type="checkbox" checked={checked} onChange={() => toggleEntry(entry.key)} disabled={busy} className="h-4 w-4 shrink-0 accent-blue-600" />
                  <span className="break-all">{entry.serverId}</span>
                </label>
                <span className={`rounded-full px-2 py-1 text-[11px] ${entry.conflict ? "bg-amber-500/10 text-amber-700 dark:text-amber-200" : "bg-blue-500/10 text-blue-700 dark:text-blue-300"}`}>{entry.appId === "trae-cn" ? "TRAE CN" : "TRAE"}{entry.conflict ? " · ID 冲突" : ""}</span>
              </div>
              <dl className="mt-3 space-y-2 break-all text-xs">
                <div><dt className="text-slate-500 dark:text-slate-400">来源</dt><dd className="mt-1 font-mono">{entry.sourcePath}</dd></div>
                <div><dt className="text-slate-500 dark:text-slate-400">目标</dt><dd className="mt-1 font-mono">{entry.targetPath}</dd></div>
                {entry.command ? <div><dt className="text-slate-500 dark:text-slate-400">命令</dt><dd className="mt-1 font-mono">{entry.command}</dd></div> : null}
                {entry.url ? <div><dt className="text-slate-500 dark:text-slate-400">URL</dt><dd className="mt-1 font-mono">{entry.url}</dd></div> : null}
              </dl>
              {entry.conflict ? <div className="mt-3 space-y-2">
                <label htmlFor={renameId} className="text-xs font-medium">新服务器 ID{checked ? "（必填）" : ""}</label>
                <input id={renameId} className="glass-input w-full px-3 py-2 text-xs" value={renames[entry.key] ?? ""} onChange={(event) => setRenames((previous) => ({ ...previous, [entry.key]: event.target.value }))} disabled={!checked || busy} aria-required={checked} aria-describedby={`${renameId}-help`} placeholder="例如 my-server-legacy" />
                <p id={`${renameId}-help`} className="text-[11px] leading-relaxed text-amber-700 dark:text-amber-200">已有不同配置，填写不同的新 ID 后导入，保留原配置。</p>
              </div> : null}
            </article>;
          })}
        </div> : preview ? <p className="py-10 text-center text-sm text-slate-500 dark:text-slate-400">未找到可迁移的旧 TRAE MCP 配置。</p> : null}
      </div>
      <div className="flex flex-wrap justify-end gap-2 border-t border-slate-500/10 px-4 py-3 sm:px-6">
        <Pressable type="button" className="glass-secondary-button" onClick={() => setScanVersion((version) => version + 1)} disabled={busy}><RefreshCw size={14} />重新扫描</Pressable>
        <Pressable type="button" className="glass-secondary-button" onClick={close} disabled={busy}>关闭</Pressable>
        <Pressable type="button" className="glass-primary-button" onClick={() => void apply()} disabled={busy || !selectedEntries.length || invalidRename}>{applying ? <Loader2 size={14} className="animate-spin" /> : null}{applying ? "迁移中…" : `迁移已选择 ${selectedEntries.length} 项`}</Pressable>
      </div>
    </div>
  </Modal>;
}
