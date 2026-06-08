import { useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { GitBranch, Folder, Trash2, Sparkles, X, FileText, CheckSquare, Square, Github, RefreshCw, Pencil, AlertTriangle, Info, ExternalLink } from 'lucide-react';
import { toast } from 'sonner';
import type { ManagedSkill, ToolOption } from './types';
import type { SkillDeletePreview, SkillHealthItem } from '@/lib/api';
import { APP_COLORS } from '@/lib/tools';
import { skillsApi } from '@/lib/api';

type SkillFilter = 'all' | 'git' | 'local' | 'synced' | 'unsynced' | 'needsAttention';

interface SkillsListProps {
  skills: ManagedSkill[];
  tools: ToolOption[];
  selectedSkills: Set<string>;
  onSelectionChange: (skillId: string, selected: boolean) => void;
  onSelectAll: (selected: boolean) => void;
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
  selectedSkills,
  onSelectionChange,
  onSelectAll,
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

  const allSelected = filteredSkills.length > 0 && filteredSkills.every(s => selectedSkills.has(s.id));
  const someSelected = filteredSkills.some(s => selectedSkills.has(s.id)) && !allSelected;

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
    const confirmed = window.confirm(
      `刷新 "${skill.name}" 会覆盖中央仓库内容，并影响 ${skill.targets.length} 个已同步目标。是否继续？`
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
      <div className="space-y-3">
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
          <>
            {/* 全选栏 */}
            <div className="flex items-center gap-2 px-3 sm:px-5 py-2 mb-2">
              <button
                onClick={() => onSelectAll(!allSelected)}
                className="flex items-center gap-2 text-sm text-slate-500 transition-colors hover:text-slate-950 dark:text-slate-400 dark:hover:text-white"
              >
                {someSelected ? (
                  <CheckSquare size={16} className="text-[hsl(var(--primary))]" />
                ) : allSelected ? (
                  <CheckSquare size={16} className="text-[hsl(var(--primary))]" />
                ) : (
                  <Square size={16} />
                )}
                <span>全选</span>
              </button>
              {selectedSkills.size > 0 && (
                <span className="glass-pill">
                  已选择 {selectedSkills.size} 项
                </span>
              )}
            </div>
            {filteredSkills.map(skill => (
            <div
              key={skill.id}
              className={`glass-card group overflow-hidden ${
                selectedSkills.has(skill.id) ? 'ring-2 ring-blue-500/40' : ''
              }`}
            >
              {/* 技能头部 */}
              <div className="px-3 sm:px-5 py-3 sm:py-4 flex items-start justify-between gap-3">
                <div className="flex items-center gap-2 sm:gap-3 flex-1 min-w-0">
                  <button
                    onClick={() => onSelectionChange(skill.id, !selectedSkills.has(skill.id))}
                    className="glass-icon-button flex-shrink-0"
                  >
                    {selectedSkills.has(skill.id) ? (
                      <CheckSquare size={18} className="text-[hsl(var(--primary))]" />
                    ) : (
                      <Square size={18} className="text-[hsl(var(--muted-foreground))]" />
                    )}
                  </button>
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
                    <button
                      onClick={() => handleOpenDetail(skill)}
                      className="truncate text-left text-sm font-semibold transition-colors hover:text-blue-600 dark:hover:text-sky-300"
                    >
                      {skill.name}
                    </button>
                    <p className="mt-0.5 text-xs text-slate-500 dark:text-slate-400">
                      {sourceTypeLabel(skill.source_type)}
                      {skill.targets.length > 0 && ` · ${skill.targets.length} 个同步目标`}
                    </p>
                  </div>
                </div>

                <div className="flex items-center gap-1 sm:opacity-0 sm:group-hover:opacity-100 transition-opacity flex-shrink-0">
                  {attentionSkillIds.has(skill.id) && (
                    <span className="glass-pill text-amber-700" title="健康检查发现需处理项">
                      <AlertTriangle size={12} />
                      需处理
                    </span>
                  )}
                  {isGitHubUrl(skill.source_ref) && (
                    <button
                      onClick={() => handleRefreshGitSkill(skill)}
                      disabled={refreshingSkill === skill.id}
                      className="glass-icon-button"
                      title="从 GitHub 刷新"
                    >
                      <RefreshCw size={14} className={refreshingSkill === skill.id ? 'animate-spin' : ''} />
                    </button>
                  )}
                  <button
                    onClick={() => onEditSkill(skill)}
                    className="glass-icon-button"
                    title="编辑技能"
                  >
                    <Pencil size={14} />
                  </button>
                  <button
                    onClick={() => onDeleteSkill(skill)}
                    className="glass-icon-button hover:text-red-500"
                    title="删除技能"
                  >
                    <Trash2 size={14} className="text-red-500" />
                  </button>
                </div>
              </div>

              {/* 同步目标 */}
              <div className="border-t border-white/50 bg-white/25 px-3 py-2.5 dark:border-white/10 dark:bg-white/5 sm:px-5 sm:py-3">
                <div className="flex flex-wrap gap-1.5 sm:gap-2">
                  {tools.map(tool => {
                    const synced = isToolSynced(skill, tool.id);
                    const isSyncing = syncingTool === `${skill.id}-${tool.id}`;
                    return (
                      <button
                        key={tool.id}
                        onClick={() => !isSyncing && handleToggleSync(skill, tool.id, !synced)}
                        disabled={isSyncing}
                        className={`inline-flex items-center gap-1.5 rounded-full border px-2 py-1 text-xs font-semibold transition-all sm:px-2.5 sm:py-1.5 ${
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
                      </button>
                    );
                  })}
                </div>
              </div>
            </div>
          ))}
          </>
        )}
      </div>

      {/* 详情弹窗 */}
      {detailSkill && (
        <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 p-4 animate-in fade-in duration-200">
          <div className="glass-modal flex max-h-[90vh] w-full max-w-2xl flex-col overflow-hidden rounded-2xl">
            {/* 头部 */}
            <div className="flex flex-shrink-0 items-center justify-between border-b border-white/50 px-6 py-5 dark:border-white/10">
              <div className="flex items-center gap-3">
                <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-blue-600 to-sky-500 shadow-lg shadow-blue-500/15">
                  {isGitHubUrl(detailSkill.source_ref) ? (
                    <Github size={20} className="text-white" />
                  ) : detailSkill.source_type === 'git' ? (
                    <GitBranch size={20} className="text-white" />
                  ) : (
                    <Folder size={20} className="text-white" />
                  )}
                </div>
                <div>
                  <h3 className="text-lg font-semibold">{detailSkill.name}</h3>
                  <p className="text-xs text-slate-500 dark:text-slate-400">
                    {sourceTypeLabel(detailSkill.source_type)}
                  </p>
                </div>
              </div>
              <button
                onClick={() => setDetailSkill(null)}
                className="glass-icon-button"
              >
                <X size={18} />
              </button>
            </div>

            {/* 内容 */}
            <div className="flex-1 overflow-y-auto overflow-x-hidden px-6 py-5">
              <div className="mb-4 grid gap-2 rounded-xl border border-white/60 bg-white/45 p-3 text-xs dark:border-white/10 dark:bg-white/8 sm:grid-cols-2">
                <div>
                  <span className="text-slate-400">来源</span>
                  <p className="mt-0.5 break-all font-medium">{detailSkill.source_ref || detailSkill.central_path}</p>
                </div>
                <div>
                  <span className="text-slate-400">中央路径</span>
                  <p className="mt-0.5 break-all font-medium">{detailSkill.central_path}</p>
                  <button
                    type="button"
                    onClick={() => skillsApi.openSkillPath(detailSkill.central_path).catch((err) => toast.error(`打开目录失败: ${err}`))}
                    className="mt-2 inline-flex items-center gap-1 text-xs font-semibold text-blue-600 hover:underline"
                  >
                    <ExternalLink size={12} />
                    打开目录
                  </button>
                </div>
                <div>
                  <span className="text-slate-400">同步目标</span>
                  <p className="mt-0.5 font-medium">{detailSkill.targets.length} 个</p>
                </div>
                <div>
                  <span className="text-slate-400">最近同步</span>
                  <p className="mt-0.5 font-medium">
                    {detailSkill.last_sync_at ? new Date(detailSkill.last_sync_at * 1000).toLocaleString() : '暂无记录'}
                  </p>
                </div>
              </div>
              {detailHealthItems.length > 0 && (
                <div className="mb-4 rounded-xl border border-amber-200/70 bg-amber-500/10 p-3 text-xs text-amber-800">
                  <div className="mb-2 flex items-center gap-2 font-semibold">
                    <Info size={14} />
                    健康检查提示
                  </div>
                  <div className="space-y-1">
                    {detailHealthItems.map(item => (
                      <p key={`${item.scope}-${item.message}`}>{item.scope}: {item.message}</p>
                    ))}
                  </div>
                </div>
              )}
              {readmeLoading ? (
                <div className="flex items-center justify-center h-32">
                  <div className="glass-pill">加载中...</div>
                </div>
              ) : readmeContent ? (
                <div className="prose prose-sm dark:prose-invert max-w-none [&_h1]:text-lg [&_h1]:font-semibold [&_h2]:text-base [&_h2]:font-semibold [&_h3]:text-sm [&_h3]:font-semibold [&_p]:text-sm [&_ul]:text-sm [&_ol]:text-sm [&_li]:text-sm [&_code]:bg-[hsl(var(--muted))] [&_code]:px-1.5 [&_code]:py-0.5 [&_code]:rounded [&_code]:break-all [&_pre]:bg-[hsl(var(--muted))] [&_pre]:p-3 [&_pre]:rounded-lg [&_pre]:overflow-x-auto [&_a]:text-[hsl(var(--primary))] [&_a]:underline [&_table]:text-sm [&_th]:bg-[hsl(var(--muted))] [&_th]:px-3 [&_th]:py-2 [&_td]:px-3 [&_td]:py-2 [&_tr]:border [&_table]:block [&_table]:overflow-x-auto">
                  <ReactMarkdown remarkPlugins={[remarkGfm]}>{readmeContent}</ReactMarkdown>
                </div>
              ) : (
                <div className="flex flex-col items-center justify-center h-32 text-center">
                  <FileText size={32} className="mb-2 text-slate-400" />
                  <p className="text-sm text-slate-500 dark:text-slate-400">
                    技能目录下没有 SKILL.md 文件
                  </p>
                </div>
              )}
            </div>

            {/* 底部 */}
            <div className="flex flex-shrink-0 justify-end gap-3 border-t border-white/50 bg-white/25 px-6 py-4 dark:border-white/10 dark:bg-white/5">
              {isGitHubUrl(detailSkill.source_ref) && (
                <button
                  onClick={() => {
                    handleRefreshGitSkill(detailSkill);
                    setDetailSkill(null);
                  }}
                  className="glass-secondary-button"
                >
                  从 GitHub 刷新
                </button>
              )}
              <button
                onClick={() => {
                  setDetailSkill(null);
                  onDeleteSkill(detailSkill);
                }}
                className="glass-danger-button"
              >
                删除
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 删除确认弹窗 */}
      {onDeleteId && deleteSkill && (
        <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-[60] p-4 animate-in fade-in duration-200">
          <div className="glass-modal w-full max-w-lg overflow-hidden rounded-2xl">
            <div className="border-b border-white/50 px-6 py-5 dark:border-white/10">
              <h3 className="text-lg font-semibold">确认删除？</h3>
              <p className="mt-1 line-clamp-1 text-sm text-slate-500 dark:text-slate-400">
                技能: {deleteSkill.name}
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
              <button
                onClick={onCancelDelete}
                className="glass-secondary-button"
              >
                取消
              </button>
              <button
                onClick={onConfirmDelete}
                disabled={isDeleting}
                className="glass-danger-button"
              >
                {isDeleting ? '删除中...' : '删除'}
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}

export default SkillsList;
