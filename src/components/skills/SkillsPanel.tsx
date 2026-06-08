import { useState, useCallback, useEffect } from 'react';
import { Plus, RefreshCw, Search, Folder, Upload, Sparkles, Activity } from 'lucide-react';
import { toast } from 'sonner';
import SkillsList from './SkillsList';
import AddSkillModal from './modals/AddSkillModal';
import ImportModal from './modals/ImportModal';
import BatchSyncModal from './modals/BatchSyncModal';
import EditSkillModal from './modals/EditSkillModal';
import { useInstalledTools } from '@/contexts/InstalledToolsContext';
import { skillsApi } from '@/lib/api';
import type {
  ManagedSkill,
  OnboardingPlan,
  ToolOption
} from './types';
import type { SkillDeletePreview, SkillHealthItem } from '@/lib/api';

type SkillFilter = 'all' | 'git' | 'local' | 'synced' | 'unsynced' | 'needsAttention';

function SkillsPanel() {
  const [managedSkills, setManagedSkills] = useState<ManagedSkill[]>([]);
  const [plan, setPlan] = useState<OnboardingPlan | null>(null);
  const [showAddModal, setShowAddModal] = useState(false);
  const [showImportModal, setShowImportModal] = useState(false);
  const [showBatchSyncModal, setShowBatchSyncModal] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [syncTargets, setSyncTargets] = useState<Record<string, boolean>>({});
  const [deleteSkillId, setDeleteSkillId] = useState<string | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);
  const [editingSkill, setEditingSkill] = useState<ManagedSkill | null>(null);
  const [selectedSkills, setSelectedSkills] = useState<Set<string>>(new Set());
  const [isLoading, setIsLoading] = useState(true);
  const [filter, setFilter] = useState<SkillFilter>('all');
  const [toolFilter, setToolFilter] = useState('');
  const [deletePreview, setDeletePreview] = useState<SkillDeletePreview | null>(null);
  const [healthItems, setHealthItems] = useState<SkillHealthItem[]>([]);
  const [checkingHealth, setCheckingHealth] = useState(false);

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

  const loadPlan = useCallback(async () => {
    try {
      const result = await skillsApi.getOnboardingPlan();
      setPlan(result);
      return result;
    } catch (err) {
      console.warn('Failed to load onboarding plan:', err);
      return null;
    }
  }, []);

  // 当工具状态加载完成后，设置 syncTargets
  useEffect(() => {
    if (toolStatuses && toolStatuses.length > 0) {
      const targets: Record<string, boolean> = {};
      for (const t of toolStatuses) {
        // tool.id is already a string (kebab-case) from backend serialization
        const toolId = t.tool.id;
        targets[toolId] = t.installed;
      }
      setSyncTargets(targets);
    }
  }, [toolStatuses]);

  useEffect(() => {
    loadManagedSkills();
    loadPlan();
  }, [loadManagedSkills, loadPlan]);

  const tools: ToolOption[] = toolStatuses
    ?.filter(status => status.installed)
    .map((status) => ({
      id: status.tool.id,
      label: status.tool.display_name
    })) || [];

  const handleSyncTargetChange = useCallback((toolId: string, checked: boolean) => {
    setSyncTargets((prev) => ({
      ...prev,
      [toolId]: checked
    }));
  }, []);

  const handleSelectionChange = useCallback((skillId: string, selected: boolean) => {
    setSelectedSkills((prev) => {
      const next = new Set(prev);
      if (selected) {
        next.add(skillId);
      } else {
        next.delete(skillId);
      }
      return next;
    });
  }, []);

  const handleSelectAll = useCallback((selected: boolean) => {
    if (selected) {
      setSelectedSkills(new Set(managedSkills.map(s => s.id)));
    } else {
      setSelectedSkills(new Set());
    }
  }, [managedSkills]);

  const handleBatchSync = useCallback(() => {
    if (selectedSkills.size === 0) {
      toast.warning('请先选择要同步的技能');
      return;
    }
    setShowBatchSyncModal(true);
  }, [selectedSkills]);

  const handleRefresh = useCallback(async () => {
    // 刷新工具检测（这会更新 toolStatuses）
    await refreshInstalledTools();
    // 刷新技能列表
    await loadManagedSkills();
  }, [refreshInstalledTools, loadManagedSkills]);

  const handleReviewImport = useCallback(async () => {
    if (plan) {
      setShowImportModal(true);
      return;
    }
    const nextPlan = await loadPlan();
    if (nextPlan) {
      setShowImportModal(true);
    }
  }, [loadPlan, plan]);

  const handleDeleteSkill = useCallback(async (skill: ManagedSkill) => {
    setDeleteSkillId(skill.id);
    setDeletePreview(null);
    try {
      setDeletePreview(await skillsApi.previewDelete(skill.id, skill.name));
    } catch (err) {
      toast.error(`加载删除预览失败: ${err}`);
    }
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
      setDeletePreview(null);
      loadManagedSkills();
    } catch (err) {
      toast.error(`删除技能失败: ${err}`);
    } finally {
      setIsDeleting(false);
    }
  }, [deleteSkillId, managedSkills, loadManagedSkills]);

  const handleHealthCheck = useCallback(async () => {
    setCheckingHealth(true);
    try {
      setHealthItems(await skillsApi.runHealthCheck());
      toast.success('Skill 健康检查完成');
    } catch (err) {
      toast.error(`健康检查失败: ${err}`);
    } finally {
      setCheckingHealth(false);
    }
  }, []);

  const attentionSkillIds = new Set(
    healthItems
      .filter((item) => item.status !== 'ok')
      .map((item) => item.skill_id)
  );
  const syncedTargetCount = managedSkills.reduce((count, skill) => count + skill.targets.length, 0);
  const filterOptions: Array<{ id: SkillFilter; label: string }> = [
    { id: 'all', label: '全部' },
    { id: 'git', label: 'Git' },
    { id: 'local', label: '本地' },
    { id: 'synced', label: '已同步' },
    { id: 'unsynced', label: '未同步' },
    { id: 'needsAttention', label: '需处理' },
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
            <h2 className="mt-3 truncate text-2xl font-semibold tracking-tight sm:text-3xl">
              Skills 管理
            </h2>
            <p className="mt-2 text-xs text-slate-500 dark:text-slate-400 sm:text-sm">
              统一管理和同步技能到多个 AI 编程工具
            </p>
          </div>
          <div className="flex gap-2 flex-shrink-0">
            <button
              onClick={handleRefresh}
              disabled={isLoading || toolsLoading}
              className="glass-secondary-button"
            >
              <RefreshCw size={16} className={(isLoading || toolsLoading) ? "animate-spin" : ""} />
              <span className="hidden sm:inline">刷新</span>
            </button>
            <button
              onClick={handleHealthCheck}
              disabled={checkingHealth}
              className="glass-secondary-button"
            >
              <Activity size={16} className={checkingHealth ? "animate-spin" : ""} />
              <span className="hidden sm:inline">健康检查</span>
            </button>
            <button
              onClick={handleReviewImport}
              className="glass-secondary-button"
            >
              <Folder size={16} />
              <span className="hidden sm:inline">导入</span>
            </button>
            <button
              onClick={() => setShowAddModal(true)}
              className="glass-primary-button"
            >
              <Plus size={16} />
              <span className="hidden sm:inline">添加技能</span>
            </button>
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
          {filterOptions.map((option) => (
            <button
              key={option.id}
              onClick={() => setFilter(option.id)}
              className={`inline-flex min-h-8 flex-shrink-0 items-center rounded-xl px-3 text-xs font-semibold transition ${
                filter === option.id
                  ? 'bg-blue-600 text-white shadow-lg shadow-blue-500/20'
                  : 'border border-white/60 bg-white/55 text-slate-600 hover:bg-white/80'
              }`}
            >
              {option.label}
            </button>
          ))}
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
          {selectedSkills.size > 0 && (
            <button
              onClick={handleBatchSync}
              className="glass-primary-button min-h-7 px-2 py-1 text-xs"
            >
              <Upload size={12} />
              <span>批量同步到工具</span>
            </button>
          )}
          {tools.filter(t => syncTargets[t.id]).length > 0 && (
            <span className="glass-pill">
              可同步工具: {tools.filter(t => syncTargets[t.id]).length} 个
            </span>
          )}
          <span className="glass-pill">
            同步目标: {syncedTargetCount} 个
          </span>
          {attentionSkillIds.size > 0 && (
            <span className="glass-pill text-amber-700">
              需处理: {attentionSkillIds.size} 个
            </span>
          )}
        </div>
      </div>

      {/* 技能列表 */}
      <div className="glass-content px-3 sm:px-8">
        {isLoading || toolsLoading ? (
          <div className="flex items-center justify-center h-64">
            <div className="glass-pill">加载中...</div>
          </div>
        ) : (
          <SkillsList
            skills={managedSkills}
            tools={tools}
            selectedSkills={selectedSkills}
            onSelectionChange={handleSelectionChange}
            onSelectAll={handleSelectAll}
            searchQuery={searchQuery}
            onDeleteSkill={handleDeleteSkill}
            onEditSkill={handleEditSkill}
            filter={filter}
            toolFilter={toolFilter}
            attentionSkillIds={attentionSkillIds}
            healthItems={healthItems}
            onDeleteId={deleteSkillId}
            onConfirmDelete={confirmDelete}
            onCancelDelete={() => {
              setDeleteSkillId(null);
              setDeletePreview(null);
            }}
            deletePreview={deletePreview}
            isDeleting={isDeleting}
            onSkillSync={loadManagedSkills}
          />
        )}
      </div>

      {/* 模态框 */}
      <AddSkillModal
        open={showAddModal}
        onClose={() => setShowAddModal(false)}
        tools={tools}
        syncTargets={syncTargets}
        onSyncTargetChange={handleSyncTargetChange}
        onSkillAdded={loadManagedSkills}
      />
      <ImportModal
        open={showImportModal}
        onClose={() => setShowImportModal(false)}
        plan={plan}
        tools={tools}
        syncTargets={syncTargets}
        onSkillAdded={loadManagedSkills}
      />
      <BatchSyncModal
        open={showBatchSyncModal}
        onClose={() => setShowBatchSyncModal(false)}
        selectedSkills={selectedSkills}
        skills={managedSkills}
        tools={tools}
        onSyncComplete={() => {
          setSelectedSkills(new Set());
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
