import { useState, useCallback, useEffect } from 'react';
import { X, ChevronRight, Loader2, FolderSearch } from 'lucide-react';
import { toast } from 'sonner';
import type { OnboardingPlan, ToolOption } from '../types';
import { skillsApi } from '@/lib/api';

interface ImportModalProps {
  open: boolean;
  onClose: () => void;
  plan: OnboardingPlan | null;
  tools: ToolOption[];
  syncTargets: Record<string, boolean>;
  onSkillAdded: () => void;
}

function ImportModal({ open, onClose, plan, tools, syncTargets, onSkillAdded }: ImportModalProps) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<Record<string, boolean>>({});
  const [variantChoice, setVariantChoice] = useState<Record<string, string>>({});

  // Initialize default selections when plan changes
  useEffect(() => {
    if (plan) {
      const defaultSelected: Record<string, boolean> = {};
      const defaultChoice: Record<string, string> = {};
      plan.groups.forEach((group) => {
        defaultSelected[group.name] = true;
        const first = group.variants[0];
        if (first) {
          defaultChoice[group.name] = first.path;
        }
      });
      setSelected(defaultSelected);
      setVariantChoice(defaultChoice);
    }
  }, [plan]);

  const handleToggleGroup = useCallback((groupName: string, checked: boolean) => {
    setSelected((prev) => ({
      ...prev,
      [groupName]: checked
    }));
  }, []);

  const handleSelectVariant = useCallback((groupName: string, path: string) => {
    setVariantChoice((prev) => ({
      ...prev,
      [groupName]: path
    }));
  }, []);

  const handleImport = useCallback(async () => {
    if (!plan) return;
    setLoading(true);
    setError(null);
    try {
      const selectedGroups = plan.groups.filter((group) => selected[group.name]);
      const selectedTools = tools.filter(tool => syncTargets[tool.id]);

      const results = await Promise.all(selectedGroups.map(async (group) => {
        const chosenPath = variantChoice[group.name] ?? group.variants[0]?.path;
        if (!chosenPath) return null;

        // Import the skill
        const installResult = await skillsApi.importExistingSkill(chosenPath, group.name);

        // Sync to selected tools
        await Promise.all(selectedTools.map(tool => skillsApi.syncToTool({
            skillId: installResult.id,
            skillName: installResult.name,
            tool: tool.id,
            sourcePath: installResult.central_path,
          })));

        return installResult;
      }));

      const importedCount = results.filter(Boolean).length;

      if (importedCount > 0) {
        toast.success(`成功导入 ${importedCount} 个技能`);
      }
      onClose();
      onSkillAdded();
    } catch (err) {
      console.error('[DEBUG] import error:', err);
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [plan, selected, variantChoice, tools, syncTargets, onClose, onSkillAdded]);

  if (!open || !plan) return null;

  return (
    <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 p-2 sm:p-4 animate-in fade-in duration-200">
      <div className="glass-modal flex max-h-[90vh] w-full max-w-3xl flex-col overflow-hidden rounded-2xl sm:max-h-[85vh]">
        {/* 头部 */}
        <div className="flex flex-shrink-0 items-center justify-between border-b border-white/50 px-4 py-4 dark:border-white/10 sm:px-6 sm:py-5">
          <div className="min-w-0">
            <h2 className="text-base sm:text-lg font-semibold truncate">导入现有技能</h2>
            <p className="mt-0.5 text-xs text-slate-500 dark:text-slate-400">
              从已安装的工具中发现 {plan.total_skills_found} 个技能
            </p>
          </div>
          <button
            onClick={onClose}
            className="glass-icon-button flex-shrink-0"
            disabled={loading}
          >
            <X size={18} />
          </button>
        </div>

        {/* 内容 */}
        <div className="flex-1 overflow-y-auto px-4 sm:px-6 py-4 sm:py-5 space-y-4 min-h-0">
          {error && (
            <div className="p-3 rounded-lg bg-red-500/10 border border-red-500/20 text-red-500 text-sm">
              {error}
            </div>
          )}

          {plan.groups.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-64 text-center">
              <div className="glass-empty-icon mb-4">
                <FolderSearch size={28} />
              </div>
              <h3 className="text-base font-medium mb-1">未发现现有技能</h3>
              <p className="text-sm text-slate-500 dark:text-slate-400">
                在已安装的工具目录中未找到技能文件
              </p>
            </div>
          ) : (
            <div className="space-y-3">
              {plan.groups.map((group) => (
                <div key={group.name} className="glass-card overflow-hidden">
                  {/* 技能组头部 */}
                  <div className="flex items-center justify-between px-3 py-3 transition-colors hover:bg-white/35 dark:hover:bg-white/8 sm:px-4">
                    <label aria-label={`选择导入 ${group.name}`} className="flex items-center gap-3 cursor-pointer flex-1 min-w-0">
                      <input
                        type="checkbox"
                        checked={selected[group.name] || false}
                        onChange={(e) => handleToggleGroup(group.name, e.target.checked)}
                        className="w-4 h-4 rounded border-[hsl(var(--border))] text-[hsl(var(--primary))] focus:ring-[hsl(var(--primary))]"
                        disabled={loading}
                      />
                      <div className="min-w-0 flex-1">
                        <h4 className="font-medium text-sm truncate">{group.name}</h4>
                        <p className="text-xs text-slate-500 dark:text-slate-400">
                          {group.variants.length} 个来源
                        </p>
                      </div>
                    </label>
                  </div>

                  {/* 技能组变体 */}
                  {selected[group.name] && (
                    <div className="space-y-2 border-t border-white/50 bg-white/25 px-3 py-3 dark:border-white/10 dark:bg-white/5 sm:px-4">
                      {group.variants.map((variant) => (
                        <label key={variant.path} aria-label={`选择 ${variant.tool} 来源`} className="flex cursor-pointer items-center gap-3 rounded-xl p-2 transition-colors hover:bg-white/55 dark:hover:bg-white/10">
                          <input
                            type="radio"
                            name={`variant-${group.name}`}
                            checked={variantChoice[group.name] === variant.path}
                            onChange={() => handleSelectVariant(group.name, variant.path)}
                            className="w-4 h-4 text-[hsl(var(--primary))] focus:ring-[hsl(var(--primary))]"
                            disabled={loading}
                          />
                          <div className="flex-1 min-w-0">
                            <p className="text-sm font-medium">{variant.tool}</p>
                            <p className="truncate text-xs text-slate-500 dark:text-slate-400">{variant.path}</p>
                          </div>
                        </label>
                      ))}
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>

        {/* 底部 */}
        <div className="flex flex-shrink-0 flex-wrap justify-end gap-2 border-t border-white/50 bg-white/25 px-4 py-3 dark:border-white/10 dark:bg-white/5 sm:gap-3 sm:px-6 sm:py-4">
          <button
            onClick={onClose}
            className="glass-secondary-button"
            disabled={loading}
          >
            取消
          </button>
          <button
            onClick={handleImport}
            disabled={loading || !Object.values(selected).some(Boolean)}
            className="glass-primary-button"
          >
            {loading ? (
              <Loader2 size={16} className="animate-spin" />
            ) : (
              <>
                导入选中的技能
                <ChevronRight size={16} />
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
}

export default ImportModal;
