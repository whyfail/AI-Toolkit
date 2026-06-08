import { useState, useCallback, useEffect } from 'react';
import { open as openUrl } from '@tauri-apps/plugin-shell';
import * as dialog from '@tauri-apps/plugin-dialog';
import { GitBranch, Folder, Search, X, ChevronRight, Loader2, Check, Globe, Star, ArrowLeft, ExternalLink, Eye, RotateCcw } from 'lucide-react';
import { toast } from 'sonner';
import type { ToolOption, OnlineSkillDto } from '../types';
import GitPickModal, { type GitSkillCandidate } from './GitPickModal';
import { APP_COLORS } from '@/lib/tools';
import { skillsApi } from '@/lib/api';

interface FeaturedSkillDto {
  slug: string;
  name: string;
  summary: string;
  downloads: number;
  stars: number;
  source_url: string;
}

interface AddSkillModalProps {
  open: boolean;
  onClose: () => void;
  tools: ToolOption[];
  syncTargets: Record<string, boolean>;
  onSyncTargetChange: (toolId: string, checked: boolean) => void;
  onSkillAdded: () => void;
}

type Tab = 'git' | 'local' | 'online';

/** Git 标签页的阶段：input=输入URL | previewed=已预览 */
type GitPhase = 'input' | 'previewed';

const formatCount = (n: number) => {
  if (n >= 1000000) return `${(n / 1000000).toFixed(1)}M`;
  if (n >= 1000) return `${(n / 1000).toFixed(1)}K`;
  return String(n);
};

const basenameFromPath = (path: string) => {
  const parts = path.trim().split(/[\\/]/);
  for (let index = parts.length - 1; index >= 0; index--) {
    if (parts[index]) return parts[index];
  }
  return '';
};

function AddSkillModal({ open, onClose, tools, syncTargets, onSyncTargetChange, onSkillAdded }: AddSkillModalProps) {
  const [activeTab, setActiveTab] = useState<Tab>('git');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [gitUrl, setGitUrl] = useState('');
  const [gitName, setGitName] = useState('');
  const [localPath, setLocalPath] = useState('');
  const [localName, setLocalName] = useState('');
  const [localValid, setLocalValid] = useState(false);
  const [localValidationError, setLocalValidationError] = useState<string | null>(null);
  const [existingSkillNames, setExistingSkillNames] = useState<Set<string>>(new Set());

  // 验证本地文件夹是否为合规的技能目录
  const validateLocalPath = useCallback(async (path: string) => {
    if (!path.trim()) {
      setLocalValid(false);
      setLocalValidationError(null);
      return;
    }
    try {
      const result = await skillsApi.validateLocalSkill(path.trim());
      setLocalValid(result.valid);
      setLocalValidationError(result.valid ? null : result.reason);
    } catch (err) {
      setLocalValid(false);
      setLocalValidationError(err instanceof Error ? err.message : String(err));
    }
  }, []);
  const [onlineQuery, setOnlineQuery] = useState('');
  const [searchResults, setSearchResults] = useState<OnlineSkillDto[]>([]);
  const [searchLoading, setSearchLoading] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);

  // Featured skills state
  const [featuredSkills, setFeaturedSkills] = useState<FeaturedSkillDto[]>([]);
  const [featuredLoading, setFeaturedLoading] = useState(false);

  // Detail modal state
  const [detailSkill, setDetailSkill] = useState<FeaturedSkillDto | OnlineSkillDto | null>(null);

  // Git scanning state
  const [gitScanLoading, setGitScanLoading] = useState(false);
  const [gitScanError, setGitScanError] = useState<string | null>(null);
  const [gitCandidates, setGitCandidates] = useState<GitSkillCandidate[]>([]);
  const [selectedGitCandidates, setSelectedGitCandidates] = useState<GitSkillCandidate[]>([]);
  const [showGitPickModal, setShowGitPickModal] = useState(false);
  const [gitPhase, setGitPhase] = useState<GitPhase>('input');

  // 每个选中候选技能的自定义名称映射：subpath -> 自定义名称
  const [gitSkillNames, setGitSkillNames] = useState<Record<string, string>>({});

  const loadFeaturedSkills = useCallback(async () => {
    setFeaturedLoading(true);
    try {
      const skills = await skillsApi.getFeaturedSkills();
      setFeaturedSkills(skills);
    } catch (err) {
      console.error('Failed to load featured skills:', err);
    } finally {
      setFeaturedLoading(false);
    }
  }, []);

  // Load featured skills when entering online tab
  useEffect(() => {
    if (activeTab === 'online' && featuredSkills.length === 0) {
      loadFeaturedSkills();
    }
  }, [activeTab, featuredSkills.length, loadFeaturedSkills]);

  useEffect(() => {
    if (!open) return;
    skillsApi.getManagedSkills()
      .then(skills => setExistingSkillNames(new Set(skills.map(skill => skill.name.toLowerCase()))))
      .catch(() => setExistingSkillNames(new Set()));
  }, [open]);

  const resetGitState = useCallback(() => {
    setGitCandidates([]);
    setSelectedGitCandidates([]);
    setGitScanError(null);
    setGitPhase('input');
    setGitName('');
    setGitSkillNames({});
  }, []);

  const handleScanGitRepo = useCallback(async () => {
    if (!gitUrl.trim()) {
      setGitScanError('请输入 Git 仓库 URL');
      return;
    }
    setGitScanLoading(true);
    setGitScanError(null);
    setGitCandidates([]);
    setSelectedGitCandidates([]);
    setGitSkillNames({});
    try {
      const candidates = await skillsApi.listGitSkills(gitUrl.trim());

      if (candidates.length === 0) {
        setGitScanError('未在仓库中找到有效的技能');
        return;
      }

      setGitCandidates(candidates);

      if (candidates.length === 1) {
        // 单个技能：直接进入预览状态
        setSelectedGitCandidates([candidates[0]]);
        setGitName(candidates[0].name);
        setGitPhase('previewed');
      } else {
        // 多个技能：弹出选择窗口
        setSelectedGitCandidates([candidates[0]]);
        setGitSkillNames({ [candidates[0].subpath]: candidates[0].name });
        setShowGitPickModal(true);
      }
    } catch (err) {
      console.error('[DEBUG] list_git_skills error:', err);
      setGitScanError(err instanceof Error ? err.message : String(err));
    } finally {
      setGitScanLoading(false);
    }
  }, [gitUrl]);

  const handleGitCandidateToggle = useCallback((candidate: GitSkillCandidate) => {
    setSelectedGitCandidates((prev) => {
      const exists = prev.some((c) => c.subpath === candidate.subpath);
      if (exists) {
        // 移除时也清理名称映射
        setGitSkillNames(names => {
          const next = { ...names };
          delete next[candidate.subpath];
          return next;
        });
        return prev.filter((c) => c.subpath !== candidate.subpath);
      } else {
        // 添加时初始化名称映射
        setGitSkillNames(names => ({
          ...names,
          [candidate.subpath]: candidate.name,
        }));
        return [...prev, candidate];
      }
    });
  }, []);

  // GitPickModal 确认：关闭弹窗，进入预览状态
  const handleGitCandidatesConfirm = useCallback(() => {
    if (selectedGitCandidates.length === 0) {
      toast.warning('请先选择要安装的技能');
      return;
    }
    setShowGitPickModal(false);
    setGitPhase('previewed');
  }, [selectedGitCandidates]);

  const handleTabChange = (tab: Tab) => {
    setActiveTab(tab);
    setDetailSkill(null);
    // Reset git scanning state when switching tabs
    if (tab !== 'git') {
      resetGitState();
    }
  };

  const handlePickLocalPath = useCallback(async () => {
    try {
      const selected = await dialog.open({
        directory: true,
        multiple: false,
        title: '选择本地文件夹'
      });
      if (!selected || Array.isArray(selected)) return;
      setLocalPath(selected);
      validateLocalPath(selected);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [validateLocalPath]);

  const resolveNameConflict = useCallback((name: string, allowOverwrite: boolean): string | null => {
    if (!existingSkillNames.has(name.toLowerCase())) {
      return name;
    }

    const choice = window.prompt(
      allowOverwrite
        ? `技能 "${name}" 已存在，请输入处理方式：\n1 覆盖\n2 自动重命名\n3 跳过`
        : `技能 "${name}" 已存在，请输入处理方式：\n2 自动重命名\n3 跳过`,
      '2'
    );

    if (allowOverwrite && choice === '1') {
      return name;
    }
    if (choice === '2') {
      let index = 2;
      let nextName = `${name}-${index}`;
      while (existingSkillNames.has(nextName.toLowerCase())) {
        index++;
        nextName = `${name}-${index}`;
      }
      return nextName;
    }
    return null;
  }, [existingSkillNames]);

  // 点击"添加技能"按钮
  const handleCreateGit = useCallback(async () => {
    if (!gitUrl.trim()) {
      setError('请输入Git仓库URL');
      return;
    }

    // 如果还在 input 阶段，先预览
    if (gitPhase === 'input') {
      await handleScanGitRepo();
      return;
    }

    // previewed 阶段，开始安装
    const selectedTools = tools.filter(tool => syncTargets[tool.id]);
    setLoading(true);
    setError(null);

    try {
      const installedNames = await Promise.all(selectedGitCandidates.map(async (candidate) => {
        // 单个技能用 gitName，多个技能用各自 gitSkillNames 中的值
        const customName = selectedGitCandidates.length === 1
          ? gitName.trim()
          : (gitSkillNames[candidate.subpath]?.trim() || candidate.name);
        const requestedName = customName || candidate.name;
        const skillName = resolveNameConflict(requestedName, true);
        if (!skillName) {
          return null;
        }

        const created = await skillsApi.installGitSelection({
          repoUrl: gitUrl.trim(),
          subpath: candidate.subpath,
          name: skillName,
        });

        await Promise.all(selectedTools.map(tool => skillsApi.syncToTool({
          skillId: created.id,
          skillName: created.name,
          tool: tool.id,
          sourcePath: created.central_path,
        })));

        return created.name;
      }));
      const createdNames = installedNames.filter((name): name is string => Boolean(name));

      setGitUrl('');
      resetGitState();
      toast.success(`${createdNames.length > 1 ? `技能 "${createdNames.join(', ')}"` : `技能 "${createdNames[0] || '已跳过'}"`} 添加成功`);
      onClose();
      onSkillAdded();
    } catch (err) {
      console.error('[DEBUG] install_git error:', err);
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [gitUrl, gitPhase, selectedGitCandidates, gitName, gitSkillNames, handleScanGitRepo, tools, syncTargets, onClose, onSkillAdded, resetGitState, resolveNameConflict]);

  const handleCreateLocal = useCallback(async () => {
    if (!localPath.trim()) {
      setError('请选择本地文件夹');
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const defaultName = localName.trim() || basenameFromPath(localPath) || 'local-skill';
      const resolvedName = resolveNameConflict(defaultName, false);
      if (!resolvedName) {
        setLoading(false);
        toast.info('已跳过同名技能');
        return;
      }
      const created = await skillsApi.installLocalSelection({
        basePath: localPath.trim(),
        subpath: '',
        name: resolvedName
      });

      const selectedTools = tools.filter(tool => syncTargets[tool.id]);
      await Promise.all(selectedTools.map(tool => skillsApi.syncToTool({
        skillId: created.id,
        skillName: created.name,
        tool: tool.id,
        sourcePath: created.central_path,
      })));

      setLocalPath('');
      setLocalName('');
      toast.success(`技能 "${created.name}" 添加成功`);
      onClose();
      onSkillAdded();
    } catch (err) {
      console.error('[DEBUG] install_local_selection error:', err);
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [localPath, localName, tools, syncTargets, onClose, onSkillAdded, resolveNameConflict]);

  const handleSearchOnline = useCallback(async () => {
    if (!onlineQuery.trim()) {
      setSearchError('请输入搜索关键词');
      return;
    }
    setSearchLoading(true);
    setSearchError(null);
    setSearchResults([]);
    try {
      const results = await skillsApi.searchOnline(onlineQuery.trim());
      setSearchResults(results);
      if (results.length === 0) {
        setSearchError('未找到相关技能');
      }
    } catch (err) {
      console.error('[DEBUG] search_skills_online error:', err);
      setSearchError(err instanceof Error ? err.message : String(err));
    } finally {
      setSearchLoading(false);
    }
  }, [onlineQuery]);

  const handleSelectFeatured = (skill: FeaturedSkillDto) => {
    setGitUrl(skill.source_url);
    setGitName(skill.name);
    setActiveTab('git');
    resetGitState();
  };

  const toggleTool = (toolId: string) => {
    onSyncTargetChange(toolId, !syncTargets[toolId]);
  };

  const toggleAllTools = () => {
    const allEnabled = tools.every(t => syncTargets[t.id]);
    tools.forEach(t => onSyncTargetChange(t.id, !allEnabled));
  };

  if (!open) return null;

  // Git 标签页是否处于预览状态
  const isGitPreviewed = activeTab === 'git' && gitPhase === 'previewed';

  return (
    <>
      <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 p-2 sm:p-4 animate-in fade-in duration-200">
        <div className="glass-modal flex max-h-[90vh] w-full max-w-3xl flex-col overflow-hidden rounded-2xl sm:max-h-[85vh]">
          {/* 头部 */}
          <div className="flex flex-shrink-0 items-center justify-between border-b border-white/50 px-4 py-4 dark:border-white/10 sm:px-6 sm:py-5">
            <div className="min-w-0 flex items-center gap-3">
              {activeTab === 'online' && detailSkill && (
                <button
                  onClick={() => setDetailSkill(null)}
                  className="glass-icon-button"
                >
                  <ArrowLeft size={18} />
                </button>
              )}
              {isGitPreviewed && (
                <button
                  onClick={() => {
                    resetGitState();
                    setError(null);
                  }}
                  className="glass-icon-button"
                >
                  <ArrowLeft size={18} />
                </button>
              )}
              <div className="min-w-0">
                <h2 className="text-base sm:text-lg font-semibold truncate">
                  {detailSkill ? '技能详情' : '添加技能'}
                </h2>
                <p className="mt-0.5 text-xs text-slate-500 dark:text-slate-400">
                  {activeTab === 'online' && !detailSkill
                    ? '浏览和搜索在线技能'
                    : activeTab === 'online' && detailSkill
                    ? detailSkill.name
                    : isGitPreviewed
                    ? '确认技能信息后添加'
                    : '从 Git 仓库、本地文件夹添加'}
                </p>
              </div>
            </div>
            <button
              onClick={onClose}
              className="glass-icon-button flex-shrink-0"
              disabled={loading}
            >
              <X size={18} />
            </button>
          </div>

          {/* 表单内容 */}
          <div className="flex-1 overflow-y-auto px-4 sm:px-6 py-4 sm:py-5 space-y-5 min-h-0">
            {error && (
              <div className="p-3 rounded-lg bg-red-500/10 border border-red-500/20 text-red-500 text-sm">
                {error}
              </div>
            )}

            {/* 在线搜索详情页 */}
            {activeTab === 'online' && detailSkill ? (
              <div className="space-y-4">
                <div className="flex items-start gap-4">
                  <div className="flex h-12 w-12 flex-shrink-0 items-center justify-center rounded-xl bg-gradient-to-br from-blue-600 to-sky-500 shadow-lg shadow-blue-500/15">
                    <GitBranch size={20} className="text-white" />
                  </div>
                  <div className="flex-1 min-w-0">
                    <h3 className="text-base font-semibold">{detailSkill.name}</h3>
                    <p className="mt-1 text-xs text-slate-500 dark:text-slate-400">
                      {('source' in detailSkill ? detailSkill.source : detailSkill.source_url.replace('https://github.com/', ''))}
                    </p>
                  </div>
                </div>

                {'summary' in detailSkill && detailSkill.summary && (
                  <div className="glass-code rounded-xl p-4">
                    <p className="text-sm leading-relaxed">
                      {detailSkill.summary}
                    </p>
                  </div>
                )}

                <div className="flex items-center gap-6">
                  {'stars' in detailSkill && (
                    <div className="flex items-center gap-1.5 text-sm">
                      <Star size={14} className="text-yellow-500" />
                      <span>{formatCount(detailSkill.stars)}</span>
                    </div>
                  )}
                  {'downloads' in detailSkill && (
                    <div className="flex items-center gap-1.5 text-sm">
                      <span className="text-slate-500 dark:text-slate-400">下载:</span>
                      <span>{formatCount(detailSkill.downloads)}</span>
                    </div>
                  )}
                  {'installs' in detailSkill && (
                    <div className="flex items-center gap-1.5 text-sm">
                      <span className="text-slate-500 dark:text-slate-400">安装:</span>
                      <span>{formatCount(detailSkill.installs)}</span>
                    </div>
                  )}
                </div>

                <div className="flex gap-3 pt-2">
                  <button
                    onClick={() => {
                      const url = 'source_url' in detailSkill ? detailSkill.source_url : '';
                      if (url) openUrl(url);
                    }}
                    className="glass-secondary-button flex-1"
                  >
                    <ExternalLink size={14} />
                    查看源码
                  </button>
                  <button
                    onClick={() => handleSelectFeatured(detailSkill as FeaturedSkillDto)}
                    className="glass-primary-button flex-1"
                    disabled={loading}
                  >
                    <GitBranch size={14} />
                    添加此技能
                  </button>
                </div>
              </div>
            ) : (
              <>
                {/* 标签页 - 预览状态时隐藏 */}
                {!isGitPreviewed && (
                  <div className="flex rounded-xl border border-white/60 bg-white/50 p-1 dark:border-white/10 dark:bg-white/8">
                    <button
                      onClick={() => handleTabChange('git')}
                      className={`flex-1 py-2.5 px-4 text-sm font-medium rounded-md transition-all flex items-center justify-center gap-2 ${
                        activeTab === 'git'
                          ? 'bg-white text-slate-950 shadow-sm dark:bg-white/14 dark:text-white'
                          : 'text-slate-500 hover:text-slate-950 dark:text-slate-400 dark:hover:text-white'
                      }`}
                    >
                      <GitBranch size={14} />
                      <span>Git 仓库</span>
                    </button>
                    <button
                      onClick={() => handleTabChange('local')}
                      className={`flex-1 py-2.5 px-4 text-sm font-medium rounded-md transition-all flex items-center justify-center gap-2 ${
                        activeTab === 'local'
                          ? 'bg-white text-slate-950 shadow-sm dark:bg-white/14 dark:text-white'
                          : 'text-slate-500 hover:text-slate-950 dark:text-slate-400 dark:hover:text-white'
                      }`}
                    >
                      <Folder size={14} />
                      <span>本地文件夹</span>
                    </button>
                    <button
                      onClick={() => handleTabChange('online')}
                      className={`flex-1 py-2.5 px-4 text-sm font-medium rounded-md transition-all flex items-center justify-center gap-2 ${
                        activeTab === 'online'
                          ? 'bg-white text-slate-950 shadow-sm dark:bg-white/14 dark:text-white'
                          : 'text-slate-500 hover:text-slate-950 dark:text-slate-400 dark:hover:text-white'
                      }`}
                    >
                      <Globe size={14} />
                      <span>在线搜索</span>
                    </button>
                  </div>
                )}

                {/* ===== Git 标签页 ===== */}
                {activeTab === 'git' && (
                  <div className="space-y-4">
                    {/* Git 仓库 URL */}
                    <div>
                      <label htmlFor="git-url-input" className="text-sm font-medium flex items-center gap-2 mb-2">
                        Git 仓库 URL
                      </label>
                      <div className="flex gap-2">
                        <input
                          id="git-url-input"
                          type="text"
                          value={gitUrl}
                          onChange={(e) => {
                            setGitUrl(e.target.value);
                            // URL 变化时重置预览状态
                            if (gitPhase === 'previewed') {
                              resetGitState();
                            }
                          }}
                          placeholder="例如: https://github.com/username/repo.git"
                          className="glass-input flex-1 px-3 py-3 text-sm disabled:cursor-not-allowed disabled:opacity-60 sm:px-4"
                          disabled={loading || gitScanLoading || gitPhase === 'previewed'}
                        />
                        {gitPhase === 'previewed' && (
                          <button
                            onClick={() => {
                              resetGitState();
                              setError(null);
                            }}
                            className="glass-secondary-button whitespace-nowrap"
                          >
                            <RotateCcw size={14} />
                            重选
                          </button>
                        )}
                      </div>
                    </div>

                    {/* 扫描错误 */}
                    {gitScanError && (
                      <div className="p-3 rounded-lg bg-red-500/10 border border-red-500/20 text-red-500 text-sm">
                        {gitScanError}
                      </div>
                    )}

                    {/* ===== 预览后：单个技能 ===== */}
                    {gitPhase === 'previewed' && selectedGitCandidates.length === 1 && (
                      <>
                        <div>
                          <label htmlFor="git-skill-name-input" className="text-sm font-medium mb-2 flex items-center gap-2">
                            技能名称 <span className="text-[hsl(var(--muted-foreground))] font-normal">(可选)</span>
                          </label>
                          <input
                            id="git-skill-name-input"
                            type="text"
                            value={gitName}
                            onChange={(e) => setGitName(e.target.value)}
                            placeholder="留空则使用仓库名称"
                            className="glass-input w-full px-3 py-3 text-sm sm:px-4"
                            disabled={loading}
                          />
                        </div>

                        {/* 同步到工具 */}
                        <div className="glass-card space-y-3 p-3 sm:p-5">
                          <div className="flex items-center justify-between">
                            <span className="text-sm font-medium">同步到工具</span>
                            {tools.length > 0 && (
                              <button
                                type="button"
                                onClick={toggleAllTools}
                                className="text-xs text-[hsl(var(--primary))] hover:underline flex-shrink-0"
                              >
                                {tools.every(t => syncTargets[t.id]) ? '取消全选' : '全选'}
                              </button>
                            )}
                          </div>
                          {tools.length > 0 ? (
                            <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                              {tools.map(tool => {
                                const enabled = syncTargets[tool.id] ?? false;
                                return (
                                  <button
                                    key={tool.id}
                                    type="button"
                                    onClick={() => toggleTool(tool.id)}
                                    className={`flex items-center gap-3 rounded-xl border px-3 py-2.5 text-left transition-all ${
                                      enabled
                                        ? "border-blue-200/70 bg-blue-500/10 dark:border-sky-300/20"
                                        : "border-white/55 bg-white/50 hover:bg-white/75 dark:border-white/10 dark:bg-white/8 dark:hover:bg-white/12"
                                    }`}
                                    disabled={loading}
                                  >
                                    <div
                                      className={`w-4 h-4 rounded flex items-center justify-center flex-shrink-0 transition-colors ${
                                        enabled
                                          ? APP_COLORS[tool.id as keyof typeof APP_COLORS] || "bg-[hsl(var(--foreground))]"
                                          : "border border-white/50 bg-white/50 dark:border-white/10 dark:bg-white/8"
                                      }`}
                                    >
                                      {enabled && <Check size={12} className="text-white" />}
                                    </div>
                                    <span className="text-sm">{tool.label}</span>
                                  </button>
                                );
                              })}
                            </div>
                          ) : (
                            <p className="text-sm text-slate-500 dark:text-slate-400">
                              未检测到已安装的 AI 工具。
                            </p>
                          )}
                        </div>
                      </>
                    )}

                    {/* ===== 预览后：多个技能 ===== */}
                    {gitPhase === 'previewed' && selectedGitCandidates.length > 1 && (
                      <>
                        <div className="space-y-3">
                          <div className="flex items-center justify-between">
                            <div className="flex items-center gap-2">
                              <Check size={14} className="text-[hsl(var(--primary))]" />
                              <span className="text-xs font-medium text-[hsl(var(--primary))]">
                                已选择 {selectedGitCandidates.length} 个技能
                              </span>
                            </div>
                            <button
                              onClick={() => setShowGitPickModal(true)}
                              className="text-xs text-[hsl(var(--primary))] hover:underline"
                            >
                              重新选择
                            </button>
                          </div>
                          {selectedGitCandidates.map((candidate) => (
                            <div
                              key={candidate.subpath}
                              className="space-y-2 rounded-xl border border-blue-200/70 bg-blue-500/10 p-3 dark:border-sky-300/20"
                            >
                              <div className="flex items-center justify-between">
                                <div className="flex items-center gap-2 min-w-0">
                                  <GitBranch size={12} className="text-[hsl(var(--primary))] flex-shrink-0" />
                                  <span className="text-sm font-medium truncate">{candidate.name}</span>
                                </div>
                                <span className="text-xs text-[hsl(var(--muted-foreground))] font-mono flex-shrink-0 ml-2">
                                  {candidate.subpath}
                                </span>
                              </div>
                              {candidate.description && (
                                <p className="text-xs text-[hsl(var(--muted-foreground))]">{candidate.description}</p>
                              )}
                              <div>
                                <label htmlFor={`git-skill-name-${candidate.subpath}`} className="text-sm font-medium mb-1.5 block">
                                  技能名称 <span className="text-[hsl(var(--muted-foreground))] font-normal text-xs">(可选，留空使用默认)</span>
                                </label>
                                <input
                                  id={`git-skill-name-${candidate.subpath}`}
                                  type="text"
                                  value={gitSkillNames[candidate.subpath] ?? candidate.name}
                                  onChange={(e) =>
                                    setGitSkillNames(prev => ({
                                      ...prev,
                                      [candidate.subpath]: e.target.value,
                                    }))
                                  }
                                  placeholder={candidate.name}
                                className="glass-input w-full px-3 py-2 text-sm"
                                  disabled={loading}
                                />
                              </div>
                            </div>
                          ))}
                        </div>

                        {/* 同步到工具 */}
                        <div className="glass-card space-y-3 p-3 sm:p-5">
                          <div className="flex items-center justify-between">
                            <span className="text-sm font-medium">同步到工具</span>
                            {tools.length > 0 && (
                              <button
                                type="button"
                                onClick={toggleAllTools}
                                className="text-xs text-[hsl(var(--primary))] hover:underline flex-shrink-0"
                              >
                                {tools.every(t => syncTargets[t.id]) ? '取消全选' : '全选'}
                              </button>
                            )}
                          </div>
                          {tools.length > 0 ? (
                            <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                              {tools.map(tool => {
                                const enabled = syncTargets[tool.id] ?? false;
                                return (
                                  <button
                                    key={tool.id}
                                    type="button"
                                    onClick={() => toggleTool(tool.id)}
                                    className={`flex items-center gap-3 rounded-xl border px-3 py-2.5 text-left transition-all ${
                                      enabled
                                        ? "border-blue-200/70 bg-blue-500/10 dark:border-sky-300/20"
                                        : "border-white/55 bg-white/50 hover:bg-white/75 dark:border-white/10 dark:bg-white/8 dark:hover:bg-white/12"
                                    }`}
                                    disabled={loading}
                                  >
                                    <div
                                      className={`w-4 h-4 rounded flex items-center justify-center flex-shrink-0 transition-colors ${
                                        enabled
                                          ? APP_COLORS[tool.id as keyof typeof APP_COLORS] || "bg-[hsl(var(--foreground))]"
                                          : "border border-white/50 bg-white/50 dark:border-white/10 dark:bg-white/8"
                                      }`}
                                    >
                                      {enabled && <Check size={12} className="text-white" />}
                                    </div>
                                    <span className="text-sm">{tool.label}</span>
                                  </button>
                                );
                              })}
                            </div>
                          ) : (
                            <p className="text-sm text-slate-500 dark:text-slate-400">
                              未检测到已安装的 AI 工具。
                            </p>
                          )}
                        </div>
                      </>
                    )}
                  </div>
                )}

                {/* ===== 本地文件夹标签页 ===== */}
                {activeTab === 'local' && (
                  <div className="space-y-4">
                    <div>
                      <label htmlFor="local-path-input" className="text-sm font-medium mb-2 flex items-center gap-2">
                        本地文件夹
                      </label>
                      <div className="flex gap-2">
                        <input
                          id="local-path-input"
                          type="text"
                          value={localPath}
                          onChange={(e) => {
                            setLocalPath(e.target.value);
                            validateLocalPath(e.target.value);
                          }}
                          placeholder="选择或输入文件夹路径"
                          className="glass-input flex-1 px-3 py-3 text-sm sm:px-4"
                          disabled={loading}
                        />
                        <button
                          onClick={handlePickLocalPath}
                          className="glass-secondary-button whitespace-nowrap"
                          disabled={loading}
                        >
                          浏览
                        </button>
                      </div>
                    </div>
                    {localValidationError && (
                      <div className="p-3 rounded-lg bg-red-500/10 border border-red-500/20 text-red-500 text-sm">
                        {localValidationError}
                      </div>
                    )}
                    <div>
                      <label htmlFor="local-skill-name-input" className="text-sm font-medium mb-2 flex items-center gap-2">
                        技能名称 <span className="text-[hsl(var(--muted-foreground))] font-normal">(可选)</span>
                      </label>
                      <input
                        id="local-skill-name-input"
                        type="text"
                        value={localName}
                        onChange={(e) => setLocalName(e.target.value)}
                        placeholder="留空则使用文件夹名称"
                        className="glass-input w-full px-3 py-3 text-sm sm:px-4"
                        disabled={loading}
                      />
                    </div>
                  </div>
                )}

                {/* ===== 在线搜索标签页 ===== */}
                {activeTab === 'online' && (
                  <div className="space-y-4">
                    <div>
                      <label htmlFor="online-skill-search-input" className="text-sm font-medium mb-2 flex items-center gap-2">
                        搜索技能
                      </label>
                      <div className="flex gap-2">
                        <input
                          id="online-skill-search-input"
                          type="text"
                          value={onlineQuery}
                          onChange={(e) => setOnlineQuery(e.target.value)}
                          onKeyDown={(e) => e.key === 'Enter' && handleSearchOnline()}
                          placeholder="输入技能名称或关键词搜索"
                          className="glass-input flex-1 px-3 py-3 text-sm sm:px-4"
                          disabled={searchLoading}
                        />
                        <button
                          onClick={handleSearchOnline}
                          className="glass-primary-button whitespace-nowrap"
                          disabled={searchLoading}
                        >
                          {searchLoading ? (
                            <Loader2 size={14} className="animate-spin" />
                          ) : (
                            <Search size={14} />
                          )}
                          搜索
                        </button>
                      </div>
                    </div>

                    {searchError && (
                      <div className="p-3 rounded-lg bg-red-500/10 border border-red-500/20 text-red-500 text-sm">
                        {searchError}
                      </div>
                    )}

                    {/* 热门技能 */}
                    {!onlineQuery.trim() && (
                      <div className="space-y-3">
                        <p className="text-xs text-[hsl(var(--muted-foreground))] font-medium">
                          热门技能
                        </p>
                        {featuredLoading ? (
                          <div className="flex items-center justify-center py-8">
                            <Loader2 size={20} className="animate-spin text-[hsl(var(--muted-foreground))]" />
                          </div>
                        ) : featuredSkills.length > 0 ? (
                          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                            {featuredSkills.map((skill) => (
                              <button
                                key={skill.slug}
                                onClick={() => setDetailSkill(skill)}
                                className="glass-card flex items-center gap-3 p-3 text-left"
                              >
                                <div className="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-xl bg-gradient-to-br from-blue-600 to-sky-500">
                                  <GitBranch size={14} className="text-white" />
                                </div>
                                <div className="flex-1 min-w-0">
                                  <div className="text-sm font-medium truncate">{skill.name}</div>
                                  <div className="flex items-center gap-2 mt-0.5">
                                    <Star size={10} className="text-yellow-500" />
                                    <span className="text-xs text-[hsl(var(--muted-foreground))]">
                                      {formatCount(skill.stars)}
                                    </span>
                                  </div>
                                </div>
                              </button>
                            ))}
                          </div>
                        ) : (
                          <div className="text-center py-8 text-sm text-[hsl(var(--muted-foreground))]">
                            加载热门技能失败
                          </div>
                        )}
                      </div>
                    )}

                    {/* 搜索结果 */}
                    {searchResults.length > 0 && (
                      <div className="space-y-2 max-h-72 overflow-y-auto">
                        <p className="text-xs text-[hsl(var(--muted-foreground))]">
                          找到 {searchResults.length} 个技能
                        </p>
                        {searchResults.map((result) => (
                          <button
                            key={`${result.source_url}-${result.name}`}
                            onClick={() => setDetailSkill(result)}
                            className="glass-card flex w-full items-center justify-between p-3 text-left"
                          >
                            <div className="min-w-0 flex-1">
                              <div className="text-sm font-medium truncate">{result.name}</div>
                              <div className="text-xs text-[hsl(var(--muted-foreground))] truncate">
                                {result.source}
                              </div>
                            </div>
                            <div className="flex items-center gap-2 flex-shrink-0 ml-2">
                              <div className="flex items-center gap-1 text-xs text-[hsl(var(--muted-foreground))]">
                                <Star size={12} />
                                {formatCount(result.installs)}
                              </div>
                              <ChevronRight size={14} className="text-[hsl(var(--muted-foreground))]" />
                            </div>
                          </button>
                        ))}
                      </div>
                    )}

                    {!searchLoading && searchResults.length === 0 && onlineQuery.trim() && !searchError && (
                      <div className="text-center py-8 text-sm text-[hsl(var(--muted-foreground))]">
                        未找到相关技能
                      </div>
                    )}
                  </div>
                )}

                {/* 同步目标 - 仅本地标签页显示（Git 标签页在预览状态中单独显示） */}
                {activeTab === 'local' && (
                  <div className="glass-card space-y-3 p-3 sm:p-5">
                    <div className="flex items-center justify-between">
                      <span className="text-sm font-medium">同步到工具</span>
                      {tools.length > 0 && (
                        <button
                          type="button"
                          onClick={toggleAllTools}
                          className="text-xs text-[hsl(var(--primary))] hover:underline flex-shrink-0"
                        >
                          {tools.every(t => syncTargets[t.id]) ? '取消全选' : '全选'}
                        </button>
                      )}
                    </div>
                    {tools.length > 0 ? (
                      <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                        {tools.map(tool => {
                          const enabled = syncTargets[tool.id] ?? false;
                          return (
                            <button
                              key={tool.id}
                              type="button"
                              onClick={() => toggleTool(tool.id)}
                              className={`flex items-center gap-3 rounded-xl border px-3 py-2.5 text-left transition-all ${
                                enabled
                                  ? "border-blue-200/70 bg-blue-500/10 dark:border-sky-300/20"
                                  : "border-white/55 bg-white/50 hover:bg-white/75 dark:border-white/10 dark:bg-white/8 dark:hover:bg-white/12"
                              }`}
                              disabled={loading}
                            >
                              <div
                                className={`w-4 h-4 rounded flex items-center justify-center flex-shrink-0 transition-colors ${
                                  enabled
                                    ? APP_COLORS[tool.id as keyof typeof APP_COLORS] || "bg-[hsl(var(--foreground))]"
                                    : "border border-white/50 bg-white/50 dark:border-white/10 dark:bg-white/8"
                                }`}
                              >
                                {enabled && <Check size={12} className="text-white" />}
                              </div>
                              <span className="text-sm">{tool.label}</span>
                            </button>
                          );
                        })}
                      </div>
                    ) : (
                      <p className="text-sm text-slate-500 dark:text-slate-400">
                        未检测到已安装的 AI 工具。
                      </p>
                    )}
                  </div>
                )}
              </>
            )}
          </div>

          {/* 底部按钮 - 在线详情页隐藏 */}
          {!(activeTab === 'online' && detailSkill) && (
            <div className="flex flex-shrink-0 flex-wrap justify-end gap-2 border-t border-white/50 bg-white/25 px-4 py-3 dark:border-white/10 dark:bg-white/5 sm:gap-3 sm:px-6 sm:py-4">
              <button
                onClick={onClose}
                className="glass-secondary-button"
                disabled={loading}
              >
                取消
              </button>
              <button
                onClick={activeTab === 'online' ? () => {} : activeTab === 'git' ? handleCreateGit : handleCreateLocal}
                disabled={
                  loading || gitScanLoading ||
                  activeTab === 'online' ||
                  (activeTab === 'git' && !gitUrl.trim()) ||
                  (activeTab === 'local' && (!localPath.trim() || !localValid))
                }
                className="glass-primary-button"
              >
                {(loading || gitScanLoading) ? (
                  <Loader2 size={16} className="animate-spin" />
                ) : activeTab === 'git' && gitPhase === 'input' ? (
                  <>
                    <Eye size={14} />
                    预览仓库
                  </>
                ) : activeTab === 'online' ? (
                  <>
                    <Search size={14} />
                    选择技能后添加
                  </>
                ) : (
                  <>
                    添加技能
                    <ChevronRight size={16} />
                  </>
                )}
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Git 仓库多技能选择弹窗 */}
      <GitPickModal
        open={showGitPickModal}
        candidates={gitCandidates}
        selected={selectedGitCandidates}
        loading={gitScanLoading}
        onToggle={handleGitCandidateToggle}
        onConfirm={handleGitCandidatesConfirm}
        onCancel={() => setShowGitPickModal(false)}
      />
    </>
  );
}

export default AddSkillModal;
