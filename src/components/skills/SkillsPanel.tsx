import { useState, useCallback, useEffect } from 'react';
import { Plus, RefreshCw, Search, Upload, Sparkles } from 'lucide-react';
import { toast } from 'sonner';
import SkillsList from './SkillsList';
import AddSkillModal from './modals/AddSkillModal';
import BatchSyncModal from './modals/BatchSyncModal';
import EditSkillModal from './modals/EditSkillModal';
import { useInstalledTools } from '@/contexts/InstalledToolsContext';
import { skillsApi } from '@/lib/api';
import type {
  ManagedSkill,
  ToolOption
} from './types';
import { Pressable } from '@/components/ui/Pressable';
import { motion } from 'motion/react';

type SkillFilter = 'all' | 'git' | 'local' | 'synced' | 'unsynced';

function SkillsPanel() {
  const [managedSkills, setManagedSkills] = useState<ManagedSkill[]>([]);
  const [showAddModal, setShowAddModal] = useState(false);
  const [legacyImportPath, setLegacyImportPath] = useState<string | null>(null);
  const [showBatchSyncModal, setShowBatchSyncModal] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [syncTargets, setSyncTargets] = useState<Record<string, boolean>>({});
  const [deleteSkillId, setDeleteSkillId] = useState<string | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);
  const [editingSkill, setEditingSkill] = useState<ManagedSkill | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [filter, setFilter] = useState<SkillFilter>('all');
  const [toolFilter, setToolFilter] = useState('');

  // 使用共享的工具检测上下文
  const { toolStatuses, isLoading: toolsLoading, refresh: refreshInstalledTools } = useInstalledTools();

  const loadManagedSkills = useCallback(async () => {
    try {
      const result = await skillsApi.getManagedSkills();
      setManagedSkills(result);
    } catch (err) {
      console.warn('Failed to load managed skills:', err);
      toast.error(`加载技能失败: ${err}`);
    } finally {
      setIsLoading(false);
    }
  }, []);

  // 当工具状态加载完成后，设置 syncTargets
  useEffect(() => {
    const targets: Record<string, boolean> = {};
    for (const status of toolStatuses) {
      targets[status.tool.id] = status.installed;
    }
    setSyncTargets(targets);
    setToolFilter(current => current && !toolStatuses.some(status => status.tool.id === current) ? '' : current);
  }, [toolStatuses]);

  useEffect(() => {
    loadManagedSkills();
  }, [loadManagedSkills, toolStatuses]);

  const tools: ToolOption[] = toolStatuses
    ?.filter(status => status.installed)
    .map((status) => ({
      id: status.tool.id,
      label: status.tool.display_name
    })) || [];

  const legacySkills = Array.from(new Map(toolStatuses.flatMap(status =>
    (status.legacy_skills ?? []).map(skill => [skill.path, skill] as const)
  )).values());

  const handleSyncTargetChange = useCallback((toolId: string, checked: boolean) => {
    setSyncTargets((prev) => ({
      ...prev,
      [toolId]: checked
    }));
  }, []);

  const handleBatchSync = useCallback(() => {
    if (managedSkills.length === 0) {
      toast.warning('暂无可同步的技能');
      return;
    }
    if (tools.length === 0) {
      toast.warning('暂无可同步的目标工具');
      return;
    }
    setShowBatchSyncModal(true);
  }, [managedSkills.length, tools.length]);

  const handleRefresh = useCallback(async () => {
    // 刷新工具检测（这会更新 toolStatuses）
    await refreshInstalledTools();
    // 刷新技能列表
    await loadManagedSkills();
  }, [refreshInstalledTools, loadManagedSkills]);

  const handleDeleteSkill = useCallback((skill: ManagedSkill) => {
    setDeleteSkillId(skill.id);
  }, []);

  const handleEditSkill = useCallback((skill: ManagedSkill) => {
    setEditingSkill(skill);
  }, []);

  const confirmDelete = useCallback(async () => {
    if (!deleteSkillId) return;
    const skill = managedSkills.find(s => s.id === deleteSkillId);
    try {
      setIsDeleting(true);
      toast.info(`正在删除技能: ${skill?.name || deleteSkillId}`);
      await skillsApi.deleteManagedSkill(deleteSkillId, skill?.name || '');
      toast.success(`技能 "${skill?.name}" 已删除`);
      setDeleteSkillId(null);
      loadManagedSkills();
    } catch (err) {
      toast.error(`删除技能失败: ${err}`);
    } finally {
      setIsDeleting(false);
    }
  }, [deleteSkillId, managedSkills, loadManagedSkills]);

  const filterOptions: Array<{ id: SkillFilter; label: string }> = [
    { id: 'all', label: '全部' },
    { id: 'git', label: 'Git' },
    { id: 'local', label: '本地' },
    { id: 'synced', label: '已同步' },
    { id: 'unsynced', label: '未同步' },
  ];

  return (
    <div className="glass-app flex h-full flex-col overflow-hidden">
      {/* 头部 */}
      <div className="glass-header flex-shrink-0">
        <div className="flex flex-wrap items-center justify-between gap-3 mb-4 sm:mb-5">
          <div className="min-w-0">
            <div className="glass-kicker">
              <Sparkles size={13} />
              Skills
            </div>
            <h2 className="mt-3 truncate text-display">
              Skills 管理
            </h2>
            <p className="mt-2 text-caption text-slate-500 dark:text-slate-400 sm:text-body">
              管理技能，同步到本地已安装的 Agent
            </p>
          </div>
          <div className="flex gap-2 flex-shrink-0">
            <Pressable
              onClick={handleRefresh}
              disabled={isLoading || toolsLoading}
              className="glass-secondary-button"
            >
              <RefreshCw size={16} className={(isLoading || toolsLoading) ? "animate-spin" : ""} />
              <span className="hidden sm:inline">刷新</span>
            </Pressable>
            <Pressable
              onClick={() => setShowAddModal(true)}
              className="glass-primary-button"
            >
              <Plus size={16} />
              <span className="hidden sm:inline">添加技能</span>
            </Pressable>
          </div>
        </div>

        {/* 搜索栏 */}
        <div className="relative mb-3 sm:mb-4">
          <Search
            size={16}
            className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400"
          />
          <input
            type="text"
            placeholder="搜索技能..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="glass-input w-full px-4 py-2 pl-10 text-sm sm:py-2.5"
          />
        </div>

        <div className="mb-3 flex gap-2 overflow-x-auto pb-1">
          {filterOptions.map((option) => {
            const active = filter === option.id;
            return (
              <Pressable
                key={option.id}
                onClick={() => setFilter(option.id)}
                aria-pressed={active}
                className={`relative z-10 inline-flex min-h-8 flex-shrink-0 items-center rounded-xl px-3 text-xs font-medium transition-colors duration-200 ease-out ${
                  active
                    ? "text-white"
                    : "border border-white/60 bg-white/55 text-slate-600 hover:bg-white/80 dark:border-white/10 dark:bg-white/8 dark:text-slate-300 dark:hover:bg-white/12"
                }`}
              >
                {active && (
                  <motion.span
                    layoutId="skills-filter-pill"
                    className="absolute inset-0 -z-10 rounded-xl bg-gradient-to-r from-[#0A84FF] to-[#5AC8FA] shadow-[0_4px_12px_rgba(10,132,255,0.22)]"
                    transition={{ type: "spring", bounce: 0.15, duration: 0.4 }}
                  />
                )}
                {option.label}
              </Pressable>
            );
          })}
          {tools.length > 0 && (
            <select
              value={toolFilter}
              onChange={(event) => setToolFilter(event.target.value)}
              className="glass-select min-h-8 flex-shrink-0 px-3 text-xs font-semibold"
            >
              <option value="">全部工具</option>
              {tools.map((tool) => (
                <option key={tool.id} value={tool.id}>{tool.label}</option>
              ))}
            </select>
          )}
        </div>

        {/* 统计栏 */}
        <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs">
          <span className="glass-pill">
            总计: {managedSkills.length}
          </span>
          <Pressable
            onClick={handleBatchSync}
            disabled={managedSkills.length === 0 || tools.length === 0}
            className="glass-primary-button min-h-7 px-2 py-1 text-xs"
          >
            <Upload size={12} />
            <span>批量同步到工具</span>
          </Pressable>
          {tools.filter(t => syncTargets[t.id]).length > 0 && (
            <span className="glass-pill">
              已安装 Agent: {tools.filter(t => syncTargets[t.id]).length} 个
            </span>
          )}
        </div>
      </div>

      {/* 技能列表 */}
      <div className="glass-content px-3 sm:px-8">
        {legacySkills.length > 0 && (
          <details className="glass-card mb-4 p-4">
            <summary className="cursor-pointer text-sm font-medium">旧 TRAE 目录中还有 {legacySkills.length} 个技能</summary>
            <p className="mt-2 text-xs text-slate-500 dark:text-slate-400">可按需导入中央仓库，再同步到新版 TRAE。旧目录中的文件保留。</p>
            <div className="mt-3 space-y-3">
              {legacySkills.map(skill => (
                <div key={skill.path} className="flex items-start justify-between gap-3">
                  <div className="min-w-0"><p className="break-all text-sm">{skill.name}</p><p className="mt-1 break-all text-xs text-slate-500 dark:text-slate-400">{skill.path}</p></div>
                  <Pressable type="button" className="glass-secondary-button shrink-0 text-xs" aria-label={`导入旧技能 ${skill.name}`} onClick={() => { setLegacyImportPath(skill.path); setShowAddModal(true); }}>导入</Pressable>
                </div>
              ))}
            </div>
          </details>
        )}
        {isLoading || toolsLoading ? (
          <div className="flex items-center justify-center h-64">
            <div className="glass-pill">加载中...</div>
          </div>
        ) : (
          <SkillsList
            skills={managedSkills}
            tools={tools}
            searchQuery={searchQuery}
            onDeleteSkill={handleDeleteSkill}
            onEditSkill={handleEditSkill}
            filter={filter}
            toolFilter={toolFilter}
            onDeleteId={deleteSkillId}
            onConfirmDelete={confirmDelete}
            onCancelDelete={() => setDeleteSkillId(null)}
            isDeleting={isDeleting}
            onSkillSync={loadManagedSkills}
          />
        )}
      </div>

      {/* 模态框 */}
      <AddSkillModal
        open={showAddModal}
        onClose={() => { setShowAddModal(false); setLegacyImportPath(null); }}
        initialLocalPath={legacyImportPath}
        tools={tools}
        syncTargets={syncTargets}
        onSyncTargetChange={handleSyncTargetChange}
        onSkillAdded={loadManagedSkills}
      />
      <BatchSyncModal
        open={showBatchSyncModal}
        onClose={() => setShowBatchSyncModal(false)}
        skills={managedSkills}
        tools={tools}
        onSyncComplete={() => {
          loadManagedSkills();
        }}
      />
      <EditSkillModal
        open={editingSkill !== null}
        skill={editingSkill}
        onClose={() => setEditingSkill(null)}
        onSkillEdited={() => {
          setEditingSkill(null);
          loadManagedSkills();
        }}
      />
    </div>
  );
}

export default SkillsPanel;
