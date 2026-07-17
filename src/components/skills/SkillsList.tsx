import { useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import * as dialog from '@tauri-apps/plugin-dialog';
import { GitBranch, Folder, Trash2, Sparkles, X, FileText, Github, RefreshCw, Pencil, AlertTriangle, ExternalLink, CheckCircle } from 'lucide-react';
import { toast } from 'sonner';
import type { ManagedSkill, ToolOption } from './types';
import type { SkillDeletePreview, SkillHealthItem } from '@/lib/api';
import { APP_COLORS } from '@/lib/tools';
import { skillsApi } from '@/lib/api';
import { Pressable } from '@/components/ui/Pressable';
import { Modal } from '@/components/ui/Modal';
import { MotionList, MotionListItem } from '@/components/ui/MotionList';

type SkillFilter = 'all' | 'git' | 'local' | 'synced' | 'unsynced' | 'needsAttention';

interface SkillsListProps {
  skills: ManagedSkill[];
  tools: ToolOption[];
  searchQuery: string;
  onDeleteSkill: (skill: ManagedSkill) => void;
  onEditSkill: (skill: ManagedSkill) => void;
  filter: SkillFilter;
  toolFilter: string;
  attentionSkillIds: Set<string>;
  healthItems: SkillHealthItem[];
  onDeleteId: string | null;
  onConfirmDelete: () => void;
  onCancelDelete: () => void;
  deletePreview: SkillDeletePreview | null;
  onSkillSync?: () => void;
  isDeleting?: boolean;
}

const sourceTypeLabel = (type: string) => {
  switch (type) {
    case 'git': return 'Git';
    case 'link': return '软链接';
    case 'local': return '本地';
    default: return type;
  }
};

/**
 * Extract the human-friendly name and description from a SKILL.md body,
 * plus a cleaned version of the body that has those parts stripped out
 * (so they don't render twice in the body section).
 *
 * Tries YAML frontmatter first, then falls back to the first heading
 * (for name) and the first non-heading paragraph (for description).
 */
function parseSkillDoc(content: string | null): {
  name: string | null;
  description: string | null;
  body: string;
} {
  if (!content) return { name: null, description: null, body: "" };

  let name: string | null = null;
  let description: string | null = null;
  let body = content;

  // 1) YAML frontmatter
  const fmMatch = content.match(/^---\s*\r?\n([\s\S]*?)\r?\n---\s*(?:\r?\n|$)/);
  if (fmMatch) {
    const fm = fmMatch[1];
    const lines = fm.split(/\r?\n/);
    let i = 0;
    while (i < lines.length) {
      const line = lines[i];
      const kv = line.match(/^(name|description)\s*:\s*(.*)$/);
      if (kv) {
        const key = kv[1];
        let raw = kv[2].trim();
        // Strip surrounding quotes
        raw = raw.replace(/^["'](.*)["']$/, "$1");
        // Multi-line YAML scalar (| or >)
        if (raw === "|" || raw === ">") {
          const blockLines: string[] = [];
          i += 1;
          while (i < lines.length) {
            const next = lines[i];
            // Continuation lines: non-empty and not a new key
            if (!next || /^[A-Za-z_]/.test(next)) break;
            blockLines.push(next.replace(/^\s{2}/, ""));
            i += 1;
          }
          const joined = blockLines.join(raw === "|" ? "\n" : " ").trim();
          if (joined) {
            if (key === "name") name = joined;
            else description = joined;
          }
          continue;
        }
        if (raw) {
          if (key === "name") name = raw;
          else description = raw;
        }
      }
      i += 1;
    }
    // Drop the entire frontmatter block from the body.
    body = content.slice(fmMatch[0].length);
  }

  // 2) Fallback name: first H1 / H2 heading
  if (!name) {
    const headingMatch = body.match(/^#{1,2}\s+(.+?)\s*$/m);
    if (headingMatch) name = headingMatch[1].trim();
  }

  // If the first heading matches the extracted name, drop it from the body
  // so it isn't rendered twice (once in the Hero, once below).
  if (name) {
    const headingLineRegex = new RegExp(`^#{1,6}\\s+${escapeRegExp(name)}\\s*$`, "m");
    body = body.replace(headingLineRegex, "").replace(/^\s*\n+/, "");
  }

  // 3) Fallback description: first non-heading paragraph
  if (!description) {
    const stripped = body.replace(/^#{1,6}\s+.*$/m, "").replace(/^\s+/, "");
    const paraMatch = stripped.match(/^([^\n]+(?:\n(?!\s*$|#{1,6}\s)[^\n]+)*)/);
    if (paraMatch) {
      description = paraMatch[1].trim();
    }
  }

  // If the first paragraph matches the extracted description, drop it too.
  if (description) {
    const escaped = escapeRegExp(description);
    const paraLineRegex = new RegExp(`^${escaped}\\s*(?:\\n\\s*\\n)?`, "m");
    body = body.replace(paraLineRegex, "").replace(/^\s*\n+/, "");
  }

  return { name, description, body: body.trim() };
}

function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

interface MetadataCellProps {
  label: string;
  value?: string | null;
  fallback?: string | null;
  action?: React.ReactNode;
}

const MetadataCell: React.FC<MetadataCellProps> = ({ label, value, fallback, action }) => {
  const display = value ?? fallback ?? '—';
  const isLink = !!action;
  return (
    <div className="rounded-xl border border-white/55 bg-white/45 p-3.5 backdrop-blur-md dark:border-white/10 dark:bg-white/5">
      <div className="text-[11px] font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
        {label}
      </div>
      <div
        className={`mt-1 break-all text-[13px] font-medium text-slate-900 dark:text-slate-100 ${isLink ? "" : "min-h-[1.25rem]"}`}
        title={display}
      >
        {display}
      </div>
      {action}
    </div>
  );
};

const isGitHubUrl = (sourceRef?: string | null): boolean => {
  if (!sourceRef) return false;
  return sourceRef.startsWith('http://') || sourceRef.startsWith('https://');
};

const isToolSynced = (skill: ManagedSkill, toolId: string): boolean => {
  return skill.targets.some(t => t.tool === toolId);
};

function SkillsList({
  skills,
  tools,
  searchQuery,
  onDeleteSkill,
  onEditSkill,
  filter,
  toolFilter,
  attentionSkillIds,
  healthItems,
  onDeleteId,
  onConfirmDelete,
  onCancelDelete,
  deletePreview,
  onSkillSync,
  isDeleting,
}: SkillsListProps) {
  const [detailSkill, setDetailSkill] = useState<ManagedSkill | null>(null);
  const [readmeContent, setReadmeContent] = useState<string | null>(null);
  const [readmeLoading, setReadmeLoading] = useState(false);
  const [syncingTool, setSyncingTool] = useState<string | null>(null);
  const [refreshingSkill, setRefreshingSkill] = useState<string | null>(null);
  const filteredSkills = skills
    .filter(skill => {
      if (!searchQuery) return true;
      const query = searchQuery.toLowerCase();
      return (
        skill.name.toLowerCase().includes(query) ||
        skill.central_path.toLowerCase().includes(query) ||
        skill.source_type.toLowerCase().includes(query)
      );
    })
    .filter(skill => {
      if (filter === 'all') return true;
      if (filter === 'git') return isGitHubUrl(skill.source_ref) || skill.source_type === 'git';
      if (filter === 'local') return skill.source_type === 'local' || skill.source_type === 'link';
      if (filter === 'synced') return skill.targets.length > 0;
      if (filter === 'unsynced') return skill.targets.length === 0;
      if (filter === 'needsAttention') return attentionSkillIds.has(skill.id);
      return true;
    })
    .filter(skill => !toolFilter || skill.targets.some(target => target.tool === toolFilter))
    .sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));

  const deleteSkill = onDeleteId ? skills.find(s => s.id === onDeleteId) : null;
  const detailHealthItems = detailSkill
    ? healthItems.filter(item => item.skill_id === detailSkill.id && item.status !== 'ok')
    : [];

  const handleOpenDetail = async (skill: ManagedSkill) => {
    setDetailSkill(skill);
    setReadmeContent(null);
    setReadmeLoading(true);
    try {
      const content = await skillsApi.getReadme(skill.name);
      setReadmeContent(content);
    } catch (err) {
      console.error('Failed to load SKILL.md:', err);
      setReadmeContent(null);
    } finally {
      setReadmeLoading(false);
    }
  };

  // 切换技能的同步状态
  const handleToggleSync = async (skill: ManagedSkill, toolId: string, checked: boolean) => {
    setSyncingTool(`${skill.id}-${toolId}`);
    try {
      if (checked) {
        // 同步到工具
        await skillsApi.syncToTool({
          skillId: skill.id,
          skillName: skill.name,
          tool: toolId,
          sourcePath: skill.central_path,
        });
        toast.success(`已同步到 ${toolId}`);
      } else {
        // 取消同步 - 只从指定工具目录删除技能文件夹，不删除 central repo
        await skillsApi.unsyncFromTool(skill.name, toolId);
        toast.success(`已从 ${toolId} 移除`);
      }
      onSkillSync?.();
    } catch (err) {
      console.error('Sync failed:', err);
      toast.error(`操作失败: ${err}`);
    } finally {
      setSyncingTool(null);
    }
  };

  // 刷新 Git 技能（从 GitHub 重新拉取）
  const handleRefreshGitSkill = async (skill: ManagedSkill) => {
    if (!skill.source_ref) {
      toast.error('该技能没有 GitHub 地址');
      return;
    }
    const confirmed = await dialog.ask(
      `刷新 "${skill.name}" 会覆盖中央仓库内容，并影响 ${skill.targets.length} 个已同步目标。是否继续？`,
      {
        title: "刷新技能",
        kind: "warning",
        okLabel: "继续刷新",
        cancelLabel: "取消",
      }
    );
    if (!confirmed) return;
    setRefreshingSkill(skill.id);
    try {
      await skillsApi.updateSkill(skill.id);
      toast.success(`技能 "${skill.name}" 已刷新`);
      onSkillSync?.();
    } catch (err) {
      console.error('Refresh failed:', err);
      toast.error(`刷新失败: ${err}`);
    } finally {
      setRefreshingSkill(null);
    }
  };

  return (
    <>
      <MotionList className="space-y-2.5">
        {filteredSkills.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-64 text-center">
            <div className="glass-empty-icon mb-4">
              <Sparkles
                size={28}
              />
            </div>
            <h3 className="text-base font-medium mb-1">
              {searchQuery ? '未找到匹配的技能' : '暂无技能'}
            </h3>
            <p className="text-sm text-slate-500 dark:text-slate-400">
              {searchQuery ? '尝试其他关键词搜索' : '点击"添加技能"开始管理你的技能'}
            </p>
          </div>
        ) : (
          filteredSkills.map(skill => (
            <MotionListItem key={skill.id}>
            <div
              className="glass-card group overflow-hidden"
            >
              {/* 技能头部 */}
              <div className="px-3 sm:px-5 py-3 sm:py-4 flex items-start justify-between gap-3">
                <div className="flex items-center gap-2 sm:gap-3 flex-1 min-w-0">
                  <div className="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-xl bg-gradient-to-br from-blue-600 to-sky-500 shadow-lg shadow-blue-500/15">
                    {isGitHubUrl(skill.source_ref) ? (
                      <Github size={16} className="text-white" />
                    ) : skill.source_type === 'git' ? (
                      <GitBranch size={16} className="text-white" />
                    ) : (
                      <Folder size={16} className="text-white" />
                    )}
                  </div>
                  <div className="min-w-0 flex-1">
                    <Pressable
                      noScale
                      onClick={() => handleOpenDetail(skill)}
                      className="truncate text-left text-sm font-semibold transition-colors duration-200 ease-out hover:text-blue-600 dark:hover:text-sky-300 !bg-transparent !shadow-none !min-h-0 !p-0"
                    >
                      {skill.name}
                    </Pressable>
                    <p className="mt-0.5 text-xs text-slate-500 dark:text-slate-400">
                      {sourceTypeLabel(skill.source_type)}
                      {skill.targets.length > 0 && ` · ${skill.targets.length} 个同步目标`}
                    </p>
                  </div>
                </div>

                <div className="flex items-center gap-1 sm:opacity-0 sm:group-hover:opacity-100 transition-opacity flex-shrink-0">
                  {attentionSkillIds.has(skill.id) && (
                    <span className="glass-pill pill-warning" title="健康检查发现需处理项">
                      <AlertTriangle size={12} />
                      需处理
                    </span>
                  )}
                  {isGitHubUrl(skill.source_ref) && (
                    <Pressable
                      onClick={() => handleRefreshGitSkill(skill)}
                      disabled={refreshingSkill === skill.id}
                      variant="icon"
                      className="glass-icon-button"
                      title="从 GitHub 刷新"
                    >
                      <RefreshCw size={14} className={refreshingSkill === skill.id ? 'animate-spin' : ''} />
                    </Pressable>
                  )}
                  <Pressable
                    onClick={() => onEditSkill(skill)}
                    variant="icon"
                    className="glass-icon-button"
                    title="编辑技能"
                  >
                    <Pencil size={14} />
                  </Pressable>
                  <Pressable
                    onClick={() => onDeleteSkill(skill)}
                    variant="icon"
                    className="glass-icon-button hover:text-red-500"
                    title="删除技能"
                  >
                    <Trash2 size={14} className="text-red-500" />
                  </Pressable>
                </div>
              </div>

              {/* 同步目标 */}
              <div className="border-t border-white/50 bg-white/25 px-3 py-2.5 dark:border-white/10 dark:bg-white/5 sm:px-5 sm:py-3">
                <div className="flex flex-wrap gap-1.5 sm:gap-2">
                  {tools.map(tool => {
                    const synced = isToolSynced(skill, tool.id);
                    const isSyncing = syncingTool === `${skill.id}-${tool.id}`;
                    return (
                      <Pressable
                        key={tool.id}
                        onClick={() => !isSyncing && handleToggleSync(skill, tool.id, !synced)}
                        disabled={isSyncing}
                        aria-pressed={synced}
                        className={`inline-flex items-center gap-1.5 rounded-full border px-2 py-1 text-xs font-semibold transition-colors duration-200 ease-out sm:px-2.5 sm:py-1.5 ${
                          synced
                            ? "border-blue-200/70 bg-blue-500/10 text-blue-700 dark:border-sky-300/20 dark:text-sky-300"
                            : "border-white/55 bg-white/50 text-slate-500 hover:text-slate-950 dark:border-white/10 dark:bg-white/8 dark:text-slate-400 dark:hover:text-white"
                        } ${isSyncing ? 'opacity-50' : ''}`}
                      >
                        <div
                          className={`w-2 h-2 rounded-full flex-shrink-0 ${
                            synced
                              ? APP_COLORS[tool.id as keyof typeof APP_COLORS] || "bg-[hsl(var(--foreground))]"
                              : "bg-current opacity-40"
                          }`}
                        />
                        <span>{tool.label}</span>
                        {isSyncing && <span className="ml-1">...</span>}
                      </Pressable>
                    );
                  })}
                </div>
              </div>
            </div>
            </MotionListItem>
          ))
        )}
      </MotionList>

      {/* 详情弹窗 */}
      <Modal
        open={!!detailSkill}
        onClose={() => setDetailSkill(null)}
        size="2xl"
      >
        <div className="flex max-h-[90vh] w-full flex-col overflow-hidden">
          {/* Hero 头部 */}
          <div className="relative flex-shrink-0 px-7 pt-7 pb-5">
            <div className="flex items-start gap-4">
              <div className="relative flex h-14 w-14 flex-shrink-0 items-center justify-center rounded-2xl bg-gradient-to-br from-[#0A84FF] via-[#5E5CE6] to-[#5856D6] shadow-[0_8px_20px_rgba(10,132,255,0.32),inset_0_1px_0_rgba(255,255,255,0.25)]">
                {detailSkill && isGitHubUrl(detailSkill.source_ref) ? (
                  <Github size={24} className="text-white drop-shadow-sm" />
                ) : detailSkill?.source_type === 'git' ? (
                  <GitBranch size={24} className="text-white drop-shadow-sm" />
                ) : (
                  <Folder size={24} className="text-white drop-shadow-sm" />
                )}
              </div>
              <div className="min-w-0 flex-1">
                {(() => {
                  const parsed = parseSkillDoc(readmeContent);
                  const displayName = parsed.name ?? detailSkill?.name ?? "";
                  const displayDescription = parsed.description ?? detailSkill?.description;
                  return (
                    <>
                      <h3 className="truncate text-xl font-semibold tracking-tight text-slate-950 dark:text-white">
                        {displayName}
                      </h3>
                      {displayDescription && (
                        <p className="mt-1 line-clamp-3 text-[13px] leading-relaxed text-slate-600 dark:text-slate-300">
                          {displayDescription}
                        </p>
                      )}
                    </>
                  );
                })()}
                <div className="mt-2 flex flex-wrap items-center gap-1.5">
                  <span className="inline-flex items-center gap-1 rounded-full border border-white/60 bg-white/55 px-2 py-0.5 text-[11px] font-medium text-slate-600 backdrop-blur-md dark:border-white/10 dark:bg-white/8 dark:text-slate-300">
                    {detailSkill && sourceTypeLabel(detailSkill.source_type)}
                  </span>
                  {(detailSkill?.targets.length ?? 0) > 0 ? (
                    <span className="inline-flex items-center gap-1 rounded-full border border-blue-200/70 bg-blue-500/12 px-2 py-0.5 text-[11px] font-medium text-blue-700 dark:border-sky-300/25 dark:text-sky-300">
                      <CheckCircle size={11} className="text-blue-600 dark:text-sky-400" />
                      已同步 {(detailSkill?.targets.length ?? 0)} 个目标
                    </span>
                  ) : (
                    <span className="inline-flex items-center gap-1 rounded-full border border-slate-200/70 bg-slate-500/10 px-2 py-0.5 text-[11px] font-medium text-slate-600 dark:border-white/10 dark:bg-white/5 dark:text-slate-400">
                      未同步
                    </span>
                  )}
                  {detailHealthItems.length > 0 && (
                    <span className="inline-flex items-center gap-1 rounded-full border border-amber-200/70 bg-amber-500/12 px-2 py-0.5 text-[11px] font-medium text-amber-700 dark:border-amber-300/25 dark:text-amber-300">
                      <AlertTriangle size={11} />
                      {detailHealthItems.length} 项需处理
                    </span>
                  )}
                </div>
              </div>
              <Pressable
                onClick={() => setDetailSkill(null)}
                variant="icon"
                className="glass-icon-button flex-shrink-0"
                aria-label="关闭"
              >
                <X size={16} />
              </Pressable>
            </div>
          </div>
          <div className="hairline mx-7" />

          {/* 内容 */}
          <div className="flex-1 overflow-y-auto overflow-x-hidden px-7 py-5">
            {/* Metadata 网格 */}
            <div className="mb-5 grid grid-cols-1 gap-3 sm:grid-cols-2">
              <MetadataCell
                label="来源"
                value={detailSkill?.source_ref}
                fallback={detailSkill?.central_path}
              />
              <MetadataCell
                label="同步目标"
                value={`${detailSkill?.targets.length ?? 0} 个工具`}
              />
              <MetadataCell
                label="中央路径"
                value={detailSkill?.central_path}
                action={
                  <Pressable
                    noScale
                    onClick={() =>
                      detailSkill &&
                      skillsApi
                        .openSkillPath(detailSkill.central_path)
                        .catch((err) => toast.error(`打开目录失败: ${err}`))
                    }
                    className="mt-1.5 inline-flex items-center gap-1 text-xs font-medium text-[hsl(var(--primary))] hover:underline !bg-transparent !shadow-none !min-h-0 !p-0"
                  >
                    <ExternalLink size={11} />
                    在访达中打开
                  </Pressable>
                }
              />
              <MetadataCell
                label="最近同步"
                value={
                  detailSkill?.last_sync_at
                    ? new Date(detailSkill.last_sync_at * 1000).toLocaleString()
                    : '尚未同步'
                }
              />
            </div>

            {/* 健康检查 */}
            {detailHealthItems.length > 0 && (
              <div className="mb-5 overflow-hidden rounded-2xl border border-amber-200/70 bg-amber-500/8 backdrop-blur-md dark:border-amber-300/20 dark:bg-amber-500/8">
                <div className="flex items-center gap-2 border-b border-amber-200/60 bg-amber-500/8 px-4 py-2.5 text-amber-800 dark:border-amber-300/15 dark:text-amber-200">
                  <AlertTriangle size={14} />
                  <span className="text-sm font-semibold">健康检查</span>
                  <span className="ml-auto text-[11px] font-medium text-amber-700/80 dark:text-amber-300/70">
                    {detailHealthItems.length} 项
                  </span>
                </div>
                <ul className="divide-y divide-amber-200/40 px-4 py-2 text-xs text-amber-900 dark:divide-amber-300/10 dark:text-amber-100">
                  {detailHealthItems.map((item) => (
                    <li key={item.message} className="flex items-start gap-3 py-2">
                      <span className="mt-0.5 inline-flex h-1.5 w-1.5 flex-shrink-0 rounded-full bg-amber-500 dark:bg-amber-400" />
                      <span className="min-w-0 flex-1">
                        <span className="font-mono text-[11px] text-amber-700/80 dark:text-amber-300/70">
                          {item.scope}
                        </span>
                        <span className="mx-1.5 text-amber-400/60">·</span>
                        <span>{item.message}</span>
                      </span>
                    </li>
                  ))}
                </ul>
              </div>
            )}

            {/* README 区 */}
            <div className="overflow-hidden rounded-2xl border border-white/55 bg-white/40 backdrop-blur-md dark:border-white/10 dark:bg-white/5">
              <div className="flex items-center gap-2 border-b border-white/55 bg-white/30 px-4 py-2.5 dark:border-white/10 dark:bg-white/5">
                <FileText size={13} className="text-slate-500 dark:text-slate-400" />
                <span className="text-xs font-semibold uppercase tracking-wider text-slate-600 dark:text-slate-300">
                  SKILL.md
                </span>
              </div>
              <div className="px-6 py-5">
                {readmeLoading ? (
                  <div className="flex items-center justify-center py-10">
                    <div className="glass-pill">加载中...</div>
                  </div>
                ) : readmeContent ? (
                  (() => {
                    const parsed = parseSkillDoc(readmeContent);
                    if (!parsed.body) {
                      return (
                        <div className="flex flex-col items-center justify-center py-10 text-center">
                          <div className="mb-3 flex h-10 w-10 items-center justify-center rounded-2xl border border-white/55 bg-white/45 text-slate-400 dark:border-white/10 dark:bg-white/8">
                            <FileText size={18} />
                          </div>
                          <p className="text-sm font-medium text-slate-700 dark:text-slate-200">
                            SKILL.md 中没有额外内容
                          </p>
                          <p className="mt-1 text-xs text-slate-500 dark:text-slate-400">
                            名称与描述已在顶部展示
                          </p>
                        </div>
                      );
                    }
                    return (
                      <div className="prose prose-sm dark:prose-invert max-w-none
                        [&_h1]:text-lg [&_h1]:font-semibold [&_h1]:tracking-tight [&_h1]:mt-4 [&_h1]:mb-2
                        [&_h2]:text-base [&_h2]:font-semibold [&_h2]:tracking-tight [&_h2]:mt-3 [&_h2]:mb-2
                        [&_h3]:text-sm [&_h3]:font-semibold [&_h3]:mt-2.5 [&_h3]:mb-1.5
                        [&_p]:text-sm [&_p]:leading-relaxed [&_p]:my-2
                        [&_ul]:text-sm [&_ul]:my-2 [&_ul]:pl-5
                        [&_ol]:text-sm [&_ol]:my-2 [&_ol]:pl-5
                        [&_li]:my-1 [&_li]:leading-relaxed
                        [&_code]:bg-slate-900/5 [&_code]:px-1.5 [&_code]:py-0.5 [&_code]:rounded-md [&_code]:text-[12px] [&_code]:font-mono [&_code]:break-all [&_code]:before:content-none [&_code]:after:content-none
                        dark:[&_code]:bg-white/8
                        [&_pre]:bg-slate-900/5 [&_pre]:p-4 [&_pre]:rounded-xl [&_pre]:overflow-x-auto [&_pre]:my-3 [&_pre]:text-xs
                        dark:[&_pre]:bg-slate-950/60
                        [&_a]:text-[hsl(var(--primary))] [&_a]:underline [&_a]:underline-offset-2 [&_a]:font-medium
                        [&_blockquote]:border-l-2 [&_blockquote]:border-slate-300 [&_blockquote]:pl-3 [&_blockquote]:italic [&_blockquote]:text-slate-600
                        dark:[&_blockquote]:border-white/15 dark:[&_blockquote]:text-slate-400
                        [&_table]:text-sm [&_table]:my-3 [&_table]:w-full
                        [&_th]:bg-white/60 [&_th]:px-3 [&_th]:py-2 [&_th]:text-left [&_th]:font-semibold [&_th]:text-slate-700
                        dark:[&_th]:bg-white/8 dark:[&_th]:text-slate-200
                        [&_td]:px-3 [&_td]:py-2 [&_td]:border-t [&_td]:border-white/40
                        dark:[&_td]:border-white/8
                        [&_tr]:border [&_tr]:border-white/40 dark:[&_tr]:border-white/8
                        [&_table]:block [&_table]:overflow-x-auto">
                        <ReactMarkdown remarkPlugins={[remarkGfm]}>{parsed.body}</ReactMarkdown>
                      </div>
                    );
                  })()
                ) : (
                  <div className="flex flex-col items-center justify-center py-10 text-center">
                    <div className="mb-3 flex h-10 w-10 items-center justify-center rounded-2xl border border-white/55 bg-white/45 text-slate-400 dark:border-white/10 dark:bg-white/8">
                      <FileText size={18} />
                    </div>
                    <p className="text-sm font-medium text-slate-700 dark:text-slate-200">
                      技能目录下没有 SKILL.md
                    </p>
                    <p className="mt-1 text-xs text-slate-500 dark:text-slate-400">
                      在仓库中添加 README 来描述这个技能
                    </p>
                  </div>
                )}
              </div>
            </div>
          </div>

          {/* 底部 */}
          <div className="flex flex-shrink-0 items-center justify-between gap-3 border-t border-white/50 bg-white/30 px-7 py-4 dark:border-white/10 dark:bg-white/5">
            <span className="text-xs text-slate-500 dark:text-slate-400">
              共 {(detailSkill?.targets.length ?? 0)} 个同步目标
            </span>
            <div className="flex gap-2">
              {detailSkill && isGitHubUrl(detailSkill.source_ref) && (
                <Pressable
                  onClick={() => {
                    handleRefreshGitSkill(detailSkill);
                    setDetailSkill(null);
                  }}
                  className="glass-secondary-button"
                >
                  <RefreshCw size={14} />
                  从 GitHub 刷新
                </Pressable>
              )}
              <Pressable
                onClick={() => {
                  if (!detailSkill) return;
                  const s = detailSkill;
                  setDetailSkill(null);
                  onDeleteSkill(s);
                }}
                className="glass-danger-button"
              >
                <Trash2 size={14} />
                删除
              </Pressable>
            </div>
          </div>
        </div>
      </Modal>

      {/* 删除确认弹窗 */}
      <Modal
        open={!!onDeleteId && !!deleteSkill}
        onClose={onCancelDelete}
        size="lg"
        zIndex={60}
      >
        <div>
          <div className="border-b border-white/50 px-6 py-5 dark:border-white/10">
            <h3 className="text-lg font-semibold">确认删除？</h3>
            <p className="mt-1 line-clamp-1 text-sm text-slate-500 dark:text-slate-400">
              技能: {deleteSkill?.name}
            </p>
          </div>
          <div className="max-h-72 overflow-y-auto px-6 py-4 text-sm">
            {!deletePreview ? (
              <div className="glass-pill">正在加载影响范围...</div>
            ) : (
              <div className="space-y-3">
                <div className="rounded-xl border border-white/60 bg-white/45 p-3 dark:border-white/10 dark:bg-white/8">
                  <p className="text-xs text-slate-400">中央仓库</p>
                  <p className="mt-1 break-all text-xs">{deletePreview.central_path}</p>
                  <p className="mt-1 text-xs text-slate-500">
                    {deletePreview.central_exists ? '将删除' : '路径不存在'}
                  </p>
                </div>
                {deletePreview.affected_paths.map(path => (
                  <div key={path.path} className="rounded-xl border border-white/60 bg-white/45 p-3 dark:border-white/10 dark:bg-white/8">
                    <div className="flex items-center justify-between gap-2">
                      <span className="text-xs font-semibold">{path.tool}</span>
                      <span className="glass-pill">{path.is_link ? '链接' : '目录'}</span>
                    </div>
                    <p className="mt-1 break-all text-xs text-slate-500">{path.path}</p>
                  </div>
                ))}
                {deletePreview.warnings.map(warning => (
                  <p key={warning} className="text-xs text-amber-700">{warning}</p>
                ))}
              </div>
            )}
          </div>
          <div className="px-6 py-4 flex justify-end gap-3 border-t border-white/50 dark:border-white/10">
            <Pressable
              onClick={onCancelDelete}
              className="glass-secondary-button"
            >
              取消
            </Pressable>
            <Pressable
              onClick={onConfirmDelete}
              disabled={isDeleting}
              className="glass-danger-button"
            >
              {isDeleting ? '删除中...' : '删除'}
            </Pressable>
          </div>
        </div>
      </Modal>
    </>
  );
}

export default SkillsList;
