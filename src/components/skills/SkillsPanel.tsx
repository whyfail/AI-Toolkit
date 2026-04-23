import { useState, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Plus, RefreshCw, Search, Folder, Upload } from 'lucide-react';
import { toast } from 'sonner';
import SkillsList from './SkillsList';
import AddSkillModal from './modals/AddSkillModal';
import ImportModal from './modals/ImportModal';
import BatchSyncModal from './modals/BatchSyncModal';
import EditSkillModal from './modals/EditSkillModal';
import { useInstalledTools } from '@/contexts/InstalledToolsContext';
import type {
  ManagedSkill,
  OnboardingPlan,
  ToolOption
} from './types';

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

  // 使用共享的工具检测上下文
  const { toolStatuses, isLoading: toolsLoading, refresh: refreshInstalledTools } = useInstalledTools();

  const loadManagedSkills = useCallback(async () => {
    try {
      // 10秒超时保护，防止命令挂起导致页面一直loading
      const timeoutPromise = new Promise<never>((_, reject) => {
        setTimeout(() => reject(new Error('加载超时，请尝试重启应用')), 10000);
      });
      const result = await Promise.race([
        invoke<ManagedSkill[]>('get_managed_skills'),
        timeoutPromise
      ]);
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
      // 10秒超时保护
      const timeoutPromise = new Promise<never>((_, reject) => {
        setTimeout(() => reject(new Error('加载超时')), 10000);
      });
      await Promise.race([
        invoke<OnboardingPlan>('get_onboarding_plan'),
        timeoutPromise
      ]).then(result => setPlan(result));
    } catch (err) {
      console.warn('Failed to load onboarding plan:', err);
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
    await loadPlan();
    if (plan) {
      setShowImportModal(true);
    }
  }, [loadPlan, plan]);

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
      await invoke('delete_managed_skill', { skillId: deleteSkillId, skillName: skill?.name || '' });
      toast.success(`技能 "${skill?.name}" 已删除`);
      setDeleteSkillId(null);
      loadManagedSkills();
    } catch (err) {
      toast.error(`删除技能失败: ${err}`);
    } finally {
      setIsDeleting(false);
    }
  }, [deleteSkillId, managedSkills, loadManagedSkills]);

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* 头部 */}
      <div className="px-4 sm:px-8 pt-6 sm:pt-8 pb-4 sm:pb-5 border-b border-[hsl(var(--border))] flex-shrink-0">
        <div className="flex flex-wrap items-center justify-between gap-3 mb-4 sm:mb-5">
          <div className="min-w-0">
            <h2 className="text-xl sm:text-2xl font-semibold tracking-tight truncate">
              Skills 管理
            </h2>
            <p className="text-xs sm:text-sm text-[hsl(var(--muted-foreground))] mt-1">
              统一管理和同步技能到多个 AI 编程工具
            </p>
          </div>
          <div className="flex gap-2 flex-shrink-0">
            <button
              onClick={handleRefresh}
              disabled={isLoading || toolsLoading}
              className="inline-flex items-center gap-1.5 sm:gap-2 px-3 sm:px-4 py-2 sm:py-2.5 bg-[hsl(var(--secondary))] hover:brightness-[0.95] active:brightness-[0.9] text-[hsl(var(--secondary-foreground))] rounded-lg text-sm font-medium transition-all border border-[hsl(var(--border))] disabled:opacity-50"
            >
              <RefreshCw size={16} className={(isLoading || toolsLoading) ? "animate-spin" : ""} />
              <span className="hidden sm:inline">刷新</span>
            </button>
            <button
              onClick={handleReviewImport}
              className="inline-flex items-center gap-1.5 sm:gap-2 px-3 sm:px-4 py-2 sm:py-2.5 bg-[hsl(var(--secondary))] hover:brightness-[0.95] active:brightness-[0.9] text-[hsl(var(--secondary-foreground))] rounded-lg text-sm font-medium transition-all border border-[hsl(var(--border))]"
            >
              <Folder size={16} />
              <span className="hidden sm:inline">导入</span>
            </button>
            <button
              onClick={() => setShowAddModal(true)}
              className="inline-flex items-center gap-1.5 sm:gap-2 px-3 sm:px-4 py-2 sm:py-2.5 bg-[hsl(var(--primary))] hover:brightness-[0.9] active:brightness-[0.85] text-white rounded-lg text-sm font-medium transition-all shadow-sm"
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
            className="absolute left-3 top-1/2 -translate-y-1/2 text-[hsl(var(--muted-foreground))]"
          />
          <input
            type="text"
            placeholder="搜索技能..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full pl-10 pr-4 py-2 sm:py-2.5 bg-[hsl(var(--muted))] border border-[hsl(var(--border))] rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-[hsl(var(--ring))] focus:border-transparent transition-all"
          />
        </div>

        {/* 统计栏 */}
        <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs">
          <span className="font-medium text-[hsl(var(--muted-foreground))]">
            总计: {managedSkills.length}
          </span>
          {selectedSkills.size > 0 && (
            <button
              onClick={handleBatchSync}
              className="inline-flex items-center gap-1 px-2 py-1 bg-[hsl(var(--primary))] text-white rounded-md hover:brightness-[0.9] transition-all text-xs font-medium"
            >
              <Upload size={12} />
              <span>批量同步到工具</span>
            </button>
          )}
          {tools.filter(t => syncTargets[t.id]).length > 0 && (
            <span className="text-[hsl(var(--muted-foreground))]">
              已同步到: {tools.filter(t => syncTargets[t.id]).length} 个工具
            </span>
          )}
        </div>
      </div>

      {/* 技能列表 */}
      <div className="flex-1 overflow-y-auto px-3 sm:px-8 py-4 sm:py-5">
        {isLoading || toolsLoading ? (
          <div className="flex items-center justify-center h-64">
            <div className="text-[hsl(var(--muted-foreground))]">加载中...</div>
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
