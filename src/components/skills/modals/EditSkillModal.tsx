import { useState, useEffect } from 'react';
import { X, Loader2, Github, Folder, GitBranch } from 'lucide-react';
import { toast } from 'sonner';
import type { ManagedSkill } from '../types';
import { skillsApi } from '@/lib/api';

interface GitSkillCandidate {
  name: string;
  description: string | null;
  subpath: string;
}

interface EditSkillModalProps {
  open: boolean;
  skill: ManagedSkill | null;
  onClose: () => void;
  onSkillEdited: () => void;
}

function EditSkillModal({ open, skill, onClose, onSkillEdited }: EditSkillModalProps) {
  const [name, setName] = useState('');
  const [sourceRef, setSourceRef] = useState('');
  const [saving, setSaving] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [showPickModal, setShowPickModal] = useState(false);
  const [gitCandidates, setGitCandidates] = useState<GitSkillCandidate[]>([]);
  const [selectedCandidate, setSelectedCandidate] = useState<GitSkillCandidate | null>(null);
  const [pendingSourceRef, setPendingSourceRef] = useState('');
  const [existingSkillNames, setExistingSkillNames] = useState<Set<string>>(new Set());

  // Sync state when skill changes
  useEffect(() => {
    if (skill) {
      setName(skill.name);
      setSourceRef(skill.source_ref || '');
      setShowPickModal(false);
      setGitCandidates([]);
      setSelectedCandidate(null);
      setPendingSourceRef('');
    }
  }, [skill]);

  useEffect(() => {
    if (!open) return;
    skillsApi.getManagedSkills()
      .then((skills) => setExistingSkillNames(new Set(skills.map((item) => item.name.toLowerCase()))))
      .catch(() => setExistingSkillNames(new Set()));
  }, [open]);

  if (!open || !skill) return null;

  const doSave = async () => {
    let finalName = name.trim();
    if (finalName.toLowerCase() !== skill.name.toLowerCase() && existingSkillNames.has(finalName.toLowerCase())) {
      const choice = window.prompt(
        `技能 "${finalName}" 已存在，请输入处理方式：\n2 自动重命名\n3 取消保存`,
        '2'
      );
      if (choice !== '2') {
        return;
      }
      let index = 2;
      let nextName = `${finalName}-${index}`;
      while (existingSkillNames.has(nextName.toLowerCase())) {
        index++;
        nextName = `${finalName}-${index}`;
      }
      finalName = nextName;
      setName(nextName);
    }

    setSaving(true);
    try {
      await skillsApi.renameSkill({
        skillId: skill.id,
        newName: finalName,
        newSourceRef: sourceRef.trim() || null,
      });
      toast.success(`技能 "${finalName}" 已更新`);
      onSkillEdited();
      onClose();
    } catch (err) {
      console.error('Failed to rename skill:', err);
      toast.error(`更新失败: ${err}`);
    } finally {
      setSaving(false);
    }
  };

  const handleScan = async () => {
    const trimmedSourceRef = sourceRef.trim();
    if (!trimmedSourceRef || (!trimmedSourceRef.startsWith('http://') && !trimmedSourceRef.startsWith('https://'))) {
      toast.error('请输入有效的 Git URL');
      return;
    }

    setScanning(true);
    try {
      const candidates = await skillsApi.listGitSkills(trimmedSourceRef);
      if (candidates.length === 0) {
        toast.error('未在仓库中找到有效的技能');
      } else if (candidates.length === 1) {
        // 单个技能：自动填入
        setSourceRef(`${trimmedSourceRef}/tree/HEAD/${candidates[0].subpath}`);
        setName(candidates[0].name);
        toast.success('已自动填入技能信息');
      } else {
        // 多个技能：弹出选择窗口
        setGitCandidates(candidates);
        setPendingSourceRef(trimmedSourceRef);
        setShowPickModal(true);
      }
    } catch (err) {
      toast.error(`扫描失败: ${err}`);
    } finally {
      setScanning(false);
    }
  };

  const handleSave = async () => {
    if (!name.trim()) {
      toast.error('技能名称不能为空');
      return;
    }
    await doSave();
  };

  const handleCandidateSelect = (candidate: GitSkillCandidate) => {
    setSelectedCandidate(candidate);
  };

  const handleCandidateConfirm = async () => {
    if (selectedCandidate) {
      // 回填完整仓库地址，使用 HEAD 指向默认分支
      setSourceRef(`${pendingSourceRef}/tree/HEAD/${selectedCandidate.subpath}`);
      // 自动填入选中的技能名
      setName(selectedCandidate.name);
    }
    setShowPickModal(false);
    setGitCandidates([]);
    setSelectedCandidate(null);
    setPendingSourceRef('');
  };

  const handlePickModalCancel = () => {
    setShowPickModal(false);
    setGitCandidates([]);
    setSelectedCandidate(null);
    setPendingSourceRef('');
  };

  const isGitHubUrl = sourceRef.startsWith('http://') || sourceRef.startsWith('https://');

  return (
    <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 p-4 animate-in fade-in duration-200">
      <div className="glass-modal w-full max-w-md overflow-hidden rounded-2xl">
        {/* 头部 */}
        <div className="flex items-center justify-between border-b border-white/50 px-6 py-5 dark:border-white/10">
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-blue-600 to-sky-500 shadow-lg shadow-blue-500/15">
              {isGitHubUrl ? (
                <Github size={20} className="text-white" />
              ) : (
                <Folder size={20} className="text-white" />
              )}
            </div>
            <div>
              <h3 className="text-lg font-semibold">编辑技能</h3>
              <p className="text-xs text-slate-500 dark:text-slate-400">
                修改技能名称和来源地址
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="glass-icon-button"
          >
            <X size={18} />
          </button>
        </div>

        {/* 内容 */}
        <div className="px-6 py-5 space-y-4">
          {/* 名称 */}
          <div>
            <label htmlFor="edit-skill-name" className="block text-sm font-medium mb-2">
              技能名称
            </label>
            <input
              id="edit-skill-name"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="glass-input w-full px-3 py-2.5 text-sm"
              placeholder="技能名称"
            />
            <p className="mt-1.5 text-xs text-slate-500 dark:text-slate-400">
              修改名称将同步重命名技能文件夹
            </p>
          </div>

          {/* 来源地址 */}
          <div>
            <label htmlFor="edit-skill-source-ref" className="block text-sm font-medium mb-2">
              来源地址 <span className="font-normal text-slate-500 dark:text-slate-400">(可选)</span>
            </label>
            <textarea
              id="edit-skill-source-ref"
              value={sourceRef}
              onChange={(e) => setSourceRef(e.target.value)}
              rows={3}
              className="glass-input min-h-[80px] w-full resize-y px-3 py-2.5 text-sm"
              placeholder="Git URL 或本地路径"
            />
            <p className="mt-1.5 text-xs text-slate-500 dark:text-slate-400">
              Git 技能的原始仓库地址
            </p>
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
            onClick={handleScan}
            disabled={saving || scanning}
            className="glass-secondary-button"
          >
            {scanning ? (
              <span className="flex items-center gap-2">
                <Loader2 size={14} className="animate-spin" />
                扫描中...
              </span>
            ) : (
              '扫描'
            )}
          </button>
          <button
            onClick={handleSave}
            disabled={saving || !name.trim()}
            className="glass-primary-button"
          >
            {saving ? (
              <span className="flex items-center gap-2">
                <Loader2 size={14} className="animate-spin" />
                保存中...
              </span>
            ) : (
              '保存'
            )}
          </button>
        </div>
      </div>

      {/* 多技能仓库选择弹窗 */}
      {showPickModal && (
        <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-[60] p-4 animate-in fade-in duration-200">
          <div className="glass-modal w-full max-w-md overflow-hidden rounded-2xl">
            <div className="flex items-center justify-between border-b border-white/50 px-6 py-5 dark:border-white/10">
              <div>
                <h3 className="text-lg font-semibold">选择技能</h3>
                <p className="mt-0.5 text-xs text-slate-500 dark:text-slate-400">
                  该仓库包含 {gitCandidates.length} 个技能
                </p>
              </div>
              <button
                onClick={handlePickModalCancel}
                className="glass-icon-button"
              >
                <X size={18} />
              </button>
            </div>

            <div className="max-h-64 overflow-y-auto p-4 space-y-2">
              {gitCandidates.map((candidate) => (
                <button
                  key={candidate.subpath}
                  onClick={() => handleCandidateSelect(candidate)}
                  className={`flex w-full items-center gap-3 rounded-xl border p-3 text-left transition-all ${
                    selectedCandidate?.subpath === candidate.subpath
                      ? "border-blue-200/70 bg-blue-500/10 dark:border-sky-300/20"
                      : "border-white/55 bg-white/50 hover:bg-white/75 dark:border-white/10 dark:bg-white/8 dark:hover:bg-white/12"
                  }`}
                >
                  <div className="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-xl bg-gradient-to-br from-blue-600 to-sky-500">
                    <GitBranch size={14} className="text-white" />
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
                </button>
              ))}
            </div>

            <div className="flex gap-3 border-t border-white/50 bg-white/25 px-6 py-4 dark:border-white/10 dark:bg-white/5">
              <button
                onClick={handlePickModalCancel}
                className="glass-secondary-button flex-1"
              >
                取消
              </button>
              <button
                onClick={handleCandidateConfirm}
                disabled={!selectedCandidate}
                className="glass-primary-button flex-1"
              >
                确定
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default EditSkillModal;
