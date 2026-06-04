import { useState } from 'react';
import { X, CheckSquare, Square, Upload } from 'lucide-react';
import { toast } from 'sonner';
import type { ManagedSkill, ToolOption } from '../types';
import { APP_COLORS } from '@/lib/tools';
import { skillsApi } from '@/lib/api';

interface BatchSyncModalProps {
  open: boolean;
  onClose: () => void;
  selectedSkills: Set<string>;
  skills: ManagedSkill[];
  tools: ToolOption[];
  onSyncComplete: () => void;
}

function BatchSyncModal({
  open,
  onClose,
  selectedSkills,
  skills,
  tools,
  onSyncComplete,
}: BatchSyncModalProps) {
  const [selectedTools, setSelectedTools] = useState<Set<string>>(new Set());
  const [syncing, setSyncing] = useState(false);

  if (!open) return null;

  const selectedSkillsList = skills.filter(s => selectedSkills.has(s.id));

  const toggleTool = (toolId: string) => {
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
    if (selectedTools.size === 0) {
      toast.warning('请选择至少一个目标工具');
      return;
    }

    setSyncing(true);

    try {
      const tasks = selectedSkillsList.flatMap((skill) =>
        Array.from(selectedTools).map((toolId) => ({
          skill,
          toolId,
          promise: skillsApi.syncToTool({
              skillId: skill.id,
              skillName: skill.name,
              tool: toolId,
              sourcePath: skill.central_path,
          }),
        }))
      );

      const results = await Promise.allSettled(tasks.map((task) => task.promise));
      let successCount = 0;
      let failCount = 0;
      results.forEach((result, index) => {
        if (result.status === 'fulfilled') {
          successCount++;
        } else {
          const task = tasks[index];
          console.error(`Failed to sync ${task.skill.name} to ${task.toolId}:`, result.reason);
          failCount++;
        }
      });

      if (failCount === 0) {
        toast.success(`成功同步 ${successCount} 个技能到 ${selectedTools.size} 个工具`);
      } else {
        toast.warning(`同步完成: ${successCount} 成功, ${failCount} 失败`);
      }

      onSyncComplete();
      onClose();
    } catch (err) {
      toast.error(`同步失败: ${err}`);
    } finally {
      setSyncing(false);
    }
  };

  const allToolsSelected = selectedTools.size === tools.length;
  const someToolsSelected = selectedTools.size > 0 && !allToolsSelected;

  return (
    <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 p-4 animate-in fade-in duration-200">
      <div className="glass-modal w-full max-w-3xl overflow-hidden rounded-2xl">
        {/* 头部 */}
        <div className="flex items-center justify-between border-b border-white/50 px-6 py-5 dark:border-white/10">
          <div>
            <h3 className="text-lg font-semibold">批量同步技能</h3>
            <p className="mt-0.5 text-xs text-slate-500 dark:text-slate-400">
              将 {selectedSkills.size} 个技能同步到目标工具
            </p>
          </div>
          <button
            onClick={onClose}
            className="glass-icon-button"
          >
            <X size={18} />
          </button>
        </div>

        {/* 已选技能 */}
        <div className="border-b border-white/50 px-6 py-4 dark:border-white/10">
          <div className="flex items-center gap-2 mb-3">
            <button
              onClick={toggleAllTools}
              className="flex items-center gap-2 text-sm text-slate-500 transition-colors hover:text-slate-950 dark:text-slate-400 dark:hover:text-white"
            >
              {someToolsSelected ? (
                <CheckSquare size={16} className="text-[hsl(var(--primary))]" />
              ) : allToolsSelected ? (
                <CheckSquare size={16} className="text-[hsl(var(--primary))]" />
              ) : (
                <Square size={16} />
              )}
              <span className="font-medium">选择全部工具</span>
            </button>
          </div>
          <div className="flex flex-wrap gap-2">
            {selectedSkillsList.map(skill => (
              <span
                key={skill.id}
                className="glass-pill"
              >
                {skill.name}
              </span>
            ))}
          </div>
        </div>

        {/* 工具列表 */}
        <div className="px-6 py-4 max-h-64 overflow-y-auto">
          <p className="mb-3 text-sm font-medium text-slate-500 dark:text-slate-400">
            选择目标工具 ({selectedTools.size}/{tools.length})
          </p>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
            {tools.map(tool => {
              const isSelected = selectedTools.has(tool.id);
              return (
                <button
                  key={tool.id}
                  onClick={() => toggleTool(tool.id)}
                  className={`flex items-center gap-2 rounded-xl border px-3 py-2 text-sm font-semibold transition-all ${
                    isSelected
                      ? 'border-blue-200/70 bg-blue-500/10 text-blue-700 dark:border-sky-300/20 dark:text-sky-300'
                      : 'border-white/55 bg-white/50 text-slate-500 hover:text-slate-950 dark:border-white/10 dark:bg-white/8 dark:text-slate-400 dark:hover:text-white'
                  }`}
                >
                  {isSelected ? (
                    <CheckSquare size={16} />
                  ) : (
                    <Square size={16} />
                  )}
                  <div
                    className={`w-2 h-2 rounded-full ${
                      isSelected
                        ? APP_COLORS[tool.id as keyof typeof APP_COLORS] || "bg-[hsl(var(--foreground))]"
                        : "bg-current opacity-40"
                    }`}
                  />
                  <span>{tool.label}</span>
                </button>
              );
            })}
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
            disabled={syncing || selectedTools.size === 0}
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
