import { GitBranch, X, Loader2, Check } from 'lucide-react';
import { Pressable } from '@/components/ui/Pressable';
import { Modal } from '@/components/ui/Modal';

export interface GitSkillCandidate {
  name: string;
  description: string | null;
  subpath: string;
}

interface GitPickModalProps {
  open: boolean;
  candidates: GitSkillCandidate[];
  selected: GitSkillCandidate[];
  loading: boolean;
  onToggle: (candidate: GitSkillCandidate) => void;
  onConfirm: () => void;
  onCancel: () => void;
}

function GitPickModal({ open, candidates, selected, loading, onToggle, onConfirm, onCancel }: GitPickModalProps) {
  const isSelected = (c: GitSkillCandidate) =>
    selected.some((s) => s.subpath === c.subpath);

  return (
    <Modal open={open} onClose={onCancel} size="2xl" zIndex={60}>
      <div className="flex max-h-[80vh] w-full flex-col overflow-hidden">
        {/* 头部 */}
        <div className="flex items-center justify-between border-b border-white/50 px-4 py-4 dark:border-white/10 sm:px-6">
          <div>
            <h2 className="text-base sm:text-lg font-semibold">选择技能</h2>
            <p className="mt-0.5 text-xs text-slate-500 dark:text-slate-400">
              已选择 {selected.length} 个技能
            </p>
          </div>
          <Pressable
            onClick={onCancel}
            variant="icon"
            className="glass-icon-button"
            aria-label="关闭"
          >
            <X size={18} />
          </Pressable>
        </div>

        {/* 内容 */}
        <div className="flex-1 overflow-y-auto p-4">
          {loading ? (
            <div className="flex items-center justify-center py-12">
              <Loader2 size={24} className="animate-spin text-slate-400" />
              <span className="ml-3 text-sm text-slate-500 dark:text-slate-400">正在扫描仓库...</span>
            </div>
          ) : candidates.length > 0 ? (
            <div className="space-y-2">
              {candidates.map((candidate) => {
                const checked = isSelected(candidate);
                return (
                  <Pressable
                    key={candidate.subpath}
                    onClick={() => onToggle(candidate)}
                    aria-pressed={checked}
                    className={`group flex w-full items-center gap-4 rounded-xl border p-4 text-left transition-colors duration-200 ease-out ${
                      checked
                        ? "border-blue-200/70 bg-blue-500/10 dark:border-sky-300/20"
                        : "border-white/55 bg-white/50 hover:bg-white/75 dark:border-white/10 dark:bg-white/8 dark:hover:bg-white/12"
                    }`}
                  >
                    <div
                      className={`w-5 h-5 rounded-md border-2 flex items-center justify-center flex-shrink-0 transition-colors ${
                        checked
                          ? "border-blue-600 bg-blue-600"
                          : "border-white/60 bg-white/60 dark:border-white/10 dark:bg-white/8"
                      }`}
                    >
                      {checked && <Check size={12} className="text-white" />}
                    </div>
                    <div className="flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-xl bg-gradient-to-br from-[#0A84FF] to-[#5856D6]">
                      <GitBranch size={18} className="text-white" />
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium truncate">{candidate.name}</div>
                      {candidate.description && (
                        <div className="mt-0.5 line-clamp-1 text-xs text-slate-500 dark:text-slate-400">
                          {candidate.description}
                        </div>
                      )}
                      <div className="mt-1 font-mono text-xs text-slate-500 dark:text-slate-400">
                        {candidate.subpath}
                      </div>
                    </div>
                  </Pressable>
                );
              })}
            </div>
          ) : (
            <div className="py-12 text-center text-sm text-slate-500 dark:text-slate-400">
              未在仓库中找到有效的技能
            </div>
          )}
        </div>

        {/* 底部 */}
        <div className="flex gap-3 border-t border-white/50 bg-white/25 px-4 py-4 dark:border-white/10 dark:bg-white/5 sm:px-6">
          <Pressable
            onClick={onCancel}
            className="glass-secondary-button flex-1"
          >
            取消
          </Pressable>
          <Pressable
            onClick={onConfirm}
            disabled={selected.length === 0}
            className="glass-primary-button flex-1"
          >
            确认选择 {selected.length > 0 ? `(${selected.length})` : ''}
          </Pressable>
        </div>
      </div>
    </Modal>
  );
}

export default GitPickModal;
