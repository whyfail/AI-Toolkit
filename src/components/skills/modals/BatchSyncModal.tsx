import { useEffect, useState } from 'react';
import { X, CheckSquare, Square, Upload, Loader2, AlertTriangle, CheckCircle2 } from 'lucide-react';
import { toast } from 'sonner';
import type { ManagedSkill, ToolOption } from '../types';
import { APP_COLORS } from '@/lib/tools';
import { enhancementApi, skillsApi } from '@/lib/api';

interface BatchSyncModalProps {
  open: boolean;
  onClose: () => void;
  skills: ManagedSkill[];
  tools: ToolOption[];
  onSyncComplete: () => void;
}

function BatchSyncModal({
  open,
  onClose,
  skills,
  tools,
  onSyncComplete,
}: BatchSyncModalProps) {
  const [selectedTools, setSelectedTools] = useState<Set<string>>(new Set());
  const [selectedSkills, setSelectedSkills] = useState<Set<string>>(new Set());
  const [syncing, setSyncing] = useState(false);
  const [taskStates, setTaskStates] = useState<Record<string, {
    skillName: string;
    toolId: string;
    status: 'waiting' | 'syncing' | 'success' | 'error';
    message?: string;
  }>>({});

  useEffect(() => {
    if (open) {
      setSelectedSkills(new Set(skills.map(skill => skill.id)));
      setTaskStates({});
    }
  }, [open, skills]);

  if (!open) return null;

  const selectedSkillsList = skills.filter(skill => selectedSkills.has(skill.id));

  const toggleSkill = (skillId: string) => {
    if (syncing) return;
    setSelectedSkills(prev => {
      const next = new Set(prev);
      if (next.has(skillId)) {
        next.delete(skillId);
      } else {
        next.add(skillId);
      }
      return next;
    });
  };

  const toggleTool = (toolId: string) => {
    if (syncing) return;
    setSelectedTools(prev => {
      const next = new Set(prev);
      if (next.has(toolId)) {
        next.delete(toolId);
      } else {
        next.add(toolId);
      }
      return next;
    });
  };

  const toggleAllTools = () => {
    if (selectedTools.size === tools.length) {
      setSelectedTools(new Set());
    } else {
      setSelectedTools(new Set(tools.map(t => t.id)));
    }
  };

  const handleSync = async () => {
    if (selectedSkills.size === 0) {
      toast.warning('请选择至少一个 Skill');
      return;
    }
    if (selectedTools.size === 0) {
      toast.warning('请选择至少一个目标工具');
      return;
    }

    setSyncing(true);
    const taskList = selectedSkillsList.flatMap((skill) =>
      Array.from(selectedTools).map((toolId) => ({
        id: `${skill.id}-${toolId}`,
        skill,
        toolId,
      }))
    );
    setTaskStates(Object.fromEntries(taskList.map((task) => [
      task.id,
      {
        skillName: task.skill.name,
        toolId: task.toolId,
        status: 'waiting' as const,
      },
    ])));

    try {
      const results = await Promise.all(taskList.map(async (task) => {
        setTaskStates((prev) => ({
          ...prev,
          [task.id]: { ...prev[task.id], status: 'syncing' },
        }));
        try {
          const result = await skillsApi.syncToTool({
              skillId: task.skill.id,
              skillName: task.skill.name,
              tool: task.toolId,
              sourcePath: task.skill.central_path,
          });
          setTaskStates((prev) => ({
            ...prev,
            [task.id]: {
              ...prev[task.id],
              status: 'success',
              message: `${result.mode} · ${result.target_path}`,
            },
          }));
          return { ok: true };
        } catch (err) {
          const message = err instanceof Error ? err.message : String(err);
          console.error(`Failed to sync ${task.skill.name} to ${task.toolId}:`, err);
          setTaskStates((prev) => ({
            ...prev,
            [task.id]: {
              ...prev[task.id],
              status: 'error',
              message,
            },
          }));
          return { ok: false };
        }
      }));

      const successCount = results.filter(result => result.ok).length;
      const failCount = results.length - successCount;

      await enhancementApi.recordTaskLog({
        kind: 'skill-batch-sync',
        title: '批量同步 Skills 完成',
        detail: `${selectedSkillsList.length} 个 Skill，${successCount} 成功，${failCount} 失败`,
        status: failCount === 0 ? 'success' : 'warn',
      });

      if (failCount === 0) {
        toast.success(`成功同步 ${selectedSkillsList.length} 个 Skill 到 ${selectedTools.size} 个工具`);
      } else {
        toast.warning(`同步完成: ${successCount} 成功, ${failCount} 失败`);
      }

      onSyncComplete();
      if (failCount === 0) {
        onClose();
      }
    } catch (err) {
      toast.error(`同步失败: ${err}`);
    } finally {
      setSyncing(false);
    }
  };

  const allToolsSelected = selectedTools.size === tools.length;
  const someToolsSelected = selectedTools.size > 0 && !allToolsSelected;
  const allSkillsSelected = selectedSkills.size === skills.length;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-2 backdrop-blur-sm animate-in fade-in duration-200 sm:p-4">
      <div className="glass-modal flex max-h-[90vh] w-full max-w-3xl flex-col overflow-hidden rounded-2xl">
        {/* 头部 */}
        <div className="flex items-center justify-between border-b border-white/50 px-6 py-5 dark:border-white/10">
          <div>
            <h3 className="text-lg font-semibold">批量同步技能</h3>
            <p className="mt-0.5 text-xs text-slate-500 dark:text-slate-400">
              将 {selectedSkills.size}/{skills.length} 个 Skill 同步到目标工具
            </p>
          </div>
          <button
            onClick={onClose}
            disabled={syncing}
            className="glass-icon-button"
          >
            <X size={18} />
          </button>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto overflow-x-hidden">
          {/* 已选技能 */}
          <div className="border-b border-white/50 px-4 py-4 dark:border-white/10 sm:px-6">
            <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
              <p className="text-sm font-medium text-slate-500 dark:text-slate-400">
                选择 Skill ({selectedSkills.size}/{skills.length})
              </p>
              <button
                onClick={() => setSelectedSkills(allSkillsSelected ? new Set() : new Set(skills.map(skill => skill.id)))}
                disabled={syncing}
                className="flex items-center gap-2 text-sm text-slate-500 transition-colors hover:text-slate-950 dark:text-slate-400 dark:hover:text-white"
              >
                {allSkillsSelected ? (
                  <CheckSquare size={16} className="text-[hsl(var(--primary))]" />
                ) : (
                  <Square size={16} />
                )}
                <span className="font-medium">{allSkillsSelected ? '取消全选' : '全选 Skill'}</span>
              </button>
            </div>
            <div className="flex max-h-56 flex-wrap gap-2 overflow-y-auto pr-1 sm:max-h-64">
              {skills.map(skill => {
                const isSelected = selectedSkills.has(skill.id);
                return (
                  <button
                    key={skill.id}
                    onClick={() => toggleSkill(skill.id)}
                    disabled={syncing}
                    title={skill.name}
                    className={`inline-flex min-h-8 max-w-full items-center gap-1.5 rounded-full border px-3 py-1 text-xs font-semibold transition-all disabled:cursor-not-allowed ${
                      isSelected
                        ? 'border-blue-200/70 bg-blue-500/10 text-blue-700 shadow-sm shadow-blue-500/10 dark:border-sky-300/20 dark:text-sky-300'
                        : 'border-white/55 bg-white/45 text-slate-400 hover:text-slate-700 dark:border-white/10 dark:bg-white/8 dark:text-slate-500 dark:hover:text-slate-300'
                    }`}
                  >
                    {isSelected ? <CheckSquare size={13} className="shrink-0" /> : <Square size={13} className="shrink-0" />}
                    <span className="truncate">{skill.name}</span>
                  </button>
                );
              })}
            </div>
          </div>

          {/* 工具列表 */}
          <div className="px-4 py-4 sm:px-6">
            <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
              <p className="text-sm font-medium text-slate-500 dark:text-slate-400">
                选择目标工具 ({selectedTools.size}/{tools.length})
              </p>
              <button
                onClick={toggleAllTools}
                disabled={syncing}
                className="flex items-center gap-2 text-sm text-slate-500 transition-colors hover:text-slate-950 dark:text-slate-400 dark:hover:text-white"
              >
                {someToolsSelected || allToolsSelected ? (
                  <CheckSquare size={16} className="text-[hsl(var(--primary))]" />
                ) : (
                  <Square size={16} />
                )}
                <span className="font-medium">选择全部工具</span>
              </button>
            </div>
            <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
              {tools.map(tool => {
                const isSelected = selectedTools.has(tool.id);
                return (
                  <button
                    key={tool.id}
                    onClick={() => toggleTool(tool.id)}
                    className={`flex min-w-0 items-center gap-2 rounded-xl border px-3 py-2 text-sm font-semibold transition-all ${
                      isSelected
                        ? 'border-blue-200/70 bg-blue-500/10 text-blue-700 dark:border-sky-300/20 dark:text-sky-300'
                        : 'border-white/55 bg-white/50 text-slate-500 hover:text-slate-950 dark:border-white/10 dark:bg-white/8 dark:text-slate-400 dark:hover:text-white'
                    }`}
                  >
                    {isSelected ? (
                      <CheckSquare size={16} className="shrink-0" />
                    ) : (
                      <Square size={16} className="shrink-0" />
                    )}
                    <div
                      className={`h-2 w-2 shrink-0 rounded-full ${
                        isSelected
                          ? APP_COLORS[tool.id as keyof typeof APP_COLORS] || "bg-[hsl(var(--foreground))]"
                          : "bg-current opacity-40"
                      }`}
                    />
                    <span className="truncate">{tool.label}</span>
                  </button>
                );
              })}
            </div>
            {Object.keys(taskStates).length > 0 && (
              <div className="mt-4 space-y-2 rounded-xl border border-white/60 bg-white/35 p-3 dark:border-white/10 dark:bg-white/8">
                <p className="text-sm font-semibold">同步进度</p>
                <div className="max-h-40 space-y-1 overflow-y-auto">
                  {Object.entries(taskStates).map(([id, task]) => (
                    <div key={id} className="flex min-w-0 items-center gap-2 rounded-lg bg-white/45 px-2 py-1.5 text-xs dark:bg-white/8">
                      {task.status === 'syncing' ? (
                        <Loader2 size={13} className="shrink-0 animate-spin text-blue-600" />
                      ) : task.status === 'success' ? (
                        <CheckCircle2 size={13} className="shrink-0 text-emerald-600" />
                      ) : task.status === 'error' ? (
                        <AlertTriangle size={13} className="shrink-0 text-red-500" />
                      ) : (
                        <Square size={13} className="shrink-0 text-slate-400" />
                      )}
                      <span className="truncate font-medium">{task.skillName}</span>
                      <span className="shrink-0 text-slate-400">→</span>
                      <span className="shrink-0">{task.toolId}</span>
                      {task.message && <span className="truncate text-slate-500">{task.message}</span>}
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        </div>

        {/* 底部 */}
        <div className="flex justify-end gap-3 border-t border-white/50 bg-white/25 px-6 py-4 dark:border-white/10 dark:bg-white/5">
          <button
            onClick={onClose}
            className="glass-secondary-button"
          >
            取消
          </button>
          <button
            onClick={handleSync}
            disabled={syncing || selectedTools.size === 0 || selectedSkills.size === 0}
            className="glass-primary-button"
          >
            <Upload size={14} />
            {syncing ? '同步中...' : '开始同步'}
          </button>
        </div>
      </div>
    </div>
  );
}

export default BatchSyncModal;
