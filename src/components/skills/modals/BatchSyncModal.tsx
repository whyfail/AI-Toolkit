import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { X, CheckSquare, Square, Upload } from 'lucide-react';
import { toast } from 'sonner';
import type { ManagedSkill, ToolOption } from '../types';
import { APP_COLORS } from '@/lib/tools';

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
    let successCount = 0;
    let failCount = 0;

    try {
      for (const skill of selectedSkillsList) {
        for (const toolId of selectedTools) {
          try {
            await invoke('sync_skill_to_tool', {
              skillId: skill.id,
              skillName: skill.name,
              tool: toolId,
              sourcePath: skill.central_path,
            });
            successCount++;
          } catch (err) {
            console.error(`Failed to sync ${skill.name} to ${toolId}:`, err);
            failCount++;
          }
        }
      }

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
      <div className="bg-[hsl(var(--card))] rounded-2xl w-full max-w-3xl shadow-2xl border border-[hsl(var(--border))] overflow-hidden">
        {/* 头部 */}
        <div className="flex items-center justify-between px-6 py-5 border-b border-[hsl(var(--border))]">
          <div>
            <h3 className="text-lg font-semibold">批量同步技能</h3>
            <p className="text-xs text-[hsl(var(--muted-foreground))] mt-0.5">
              将 {selectedSkills.size} 个技能同步到目标工具
            </p>
          </div>
          <button
            onClick={onClose}
            className="p-2 hover:bg-[hsl(var(--muted))] rounded-lg transition-colors"
          >
            <X size={18} className="text-[hsl(var(--muted-foreground))]" />
          </button>
        </div>

        {/* 已选技能 */}
        <div className="px-6 py-4 border-b border-[hsl(var(--border))]">
          <div className="flex items-center gap-2 mb-3">
            <button
              onClick={toggleAllTools}
              className="flex items-center gap-2 text-sm text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))] transition-colors"
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
                className="px-2 py-1 bg-[hsl(var(--primary))/10] text-[hsl(var(--primary))] rounded-md text-xs font-medium"
              >
                {skill.name}
              </span>
            ))}
          </div>
        </div>

        {/* 工具列表 */}
        <div className="px-6 py-4 max-h-64 overflow-y-auto">
          <p className="text-sm font-medium text-[hsl(var(--muted-foreground))] mb-3">
            选择目标工具 ({selectedTools.size}/{tools.length})
          </p>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
            {tools.map(tool => {
              const isSelected = selectedTools.has(tool.id);
              return (
                <button
                  key={tool.id}
                  onClick={() => toggleTool(tool.id)}
                  className={`flex items-center gap-2 px-3 py-2 rounded-lg text-sm font-medium transition-all ${
                    isSelected
                      ? 'bg-[hsl(var(--primary))/10] text-[hsl(var(--primary))] border border-[hsl(var(--primary))]/30'
                      : 'bg-[hsl(var(--muted))] text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))] border border-transparent'
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
        <div className="px-6 py-4 border-t border-[hsl(var(--border))] flex justify-end gap-3">
          <button
            onClick={onClose}
            className="px-4 py-2 rounded-lg text-sm font-medium bg-[hsl(var(--secondary))] hover:brightness-[0.95] text-[hsl(var(--secondary-foreground))] transition-all border border-[hsl(var(--border))]"
          >
            取消
          </button>
          <button
            onClick={handleSync}
            disabled={syncing || selectedTools.size === 0}
            className="px-4 py-2 rounded-lg text-sm font-medium bg-[hsl(var(--primary))] hover:brightness-[0.9] text-white transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
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
