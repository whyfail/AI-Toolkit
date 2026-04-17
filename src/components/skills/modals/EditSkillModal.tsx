import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { X, Loader2, Github, Folder, GitBranch } from 'lucide-react';
import { toast } from 'sonner';
import type { ManagedSkill } from '../types';

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

  if (!open || !skill) return null;

  const doSave = async () => {
    setSaving(true);
    try {
      await invoke('rename_skill', {
        skillId: skill.id,
        newName: name.trim(),
        newSourceRef: sourceRef.trim() || null,
      });
      toast.success(`技能 "${name}" 已更新`);
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
      const candidates = await invoke<GitSkillCandidate[]>('list_git_skills', {
        repoUrl: trimmedSourceRef,
      });
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
      <div className="bg-[hsl(var(--card))] rounded-2xl w-full max-w-md shadow-2xl border border-[hsl(var(--border))] overflow-hidden">
        {/* 头部 */}
        <div className="flex items-center justify-between px-6 py-5 border-b border-[hsl(var(--border))]">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-lg bg-[hsl(var(--primary))] flex items-center justify-center">
              {isGitHubUrl ? (
                <Github size={20} className="text-white" />
              ) : (
                <Folder size={20} className="text-white" />
              )}
            </div>
            <div>
              <h3 className="text-lg font-semibold">编辑技能</h3>
              <p className="text-xs text-[hsl(var(--muted-foreground))]">
                修改技能名称和来源地址
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-2 hover:bg-[hsl(var(--muted))] rounded-lg transition-colors"
          >
            <X size={18} className="text-[hsl(var(--muted-foreground))]" />
          </button>
        </div>

        {/* 内容 */}
        <div className="px-6 py-5 space-y-4">
          {/* 名称 */}
          <div>
            <label className="block text-sm font-medium mb-2">
              技能名称
            </label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="w-full px-3 py-2.5 bg-[hsl(var(--muted))] border border-[hsl(var(--border))] rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-[hsl(var(--ring))] focus:border-transparent transition-all"
              placeholder="技能名称"
            />
            <p className="text-xs text-[hsl(var(--muted-foreground))] mt-1.5">
              修改名称将同步重命名技能文件夹
            </p>
          </div>

          {/* 来源地址 */}
          <div>
            <label className="block text-sm font-medium mb-2">
              来源地址 <span className="text-[hsl(var(--muted-foreground))] font-normal">(可选)</span>
            </label>
            <textarea
              value={sourceRef}
              onChange={(e) => setSourceRef(e.target.value)}
              rows={3}
              className="w-full px-3 py-2.5 bg-[hsl(var(--muted))] border border-[hsl(var(--border))] rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-[hsl(var(--ring))] focus:border-transparent transition-all resize-y min-h-[80px]"
              placeholder="Git URL 或本地路径"
            />
            <p className="text-xs text-[hsl(var(--muted-foreground))] mt-1.5">
              Git 技能的原始仓库地址
            </p>
          </div>
        </div>

        {/* 底部 */}
        <div className="px-6 py-4 border-t border-[hsl(var(--border))] bg-[hsl(var(--muted)/30] flex justify-end gap-3">
          <button
            onClick={onClose}
            className="px-4 py-2 rounded-lg text-sm font-medium bg-[hsl(var(--secondary))] hover:brightness-[0.95] text-[hsl(var(--secondary-foreground))] transition-all border border-[hsl(var(--border))]"
          >
            取消
          </button>
          <button
            onClick={handleScan}
            disabled={saving || scanning}
            className="px-4 py-2 rounded-lg text-sm font-medium bg-[hsl(var(--secondary))] hover:brightness-[0.95] text-[hsl(var(--secondary-foreground))] transition-all border border-[hsl(var(--border))] disabled:opacity-50"
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
            className="px-4 py-2 rounded-lg text-sm font-medium bg-[hsl(var(--primary))] hover:brightness-[0.9] text-white transition-all shadow-sm disabled:opacity-50"
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
          <div className="bg-[hsl(var(--card))] rounded-2xl w-full max-w-md shadow-2xl border border-[hsl(var(--border))] overflow-hidden">
            <div className="flex items-center justify-between px-6 py-5 border-b border-[hsl(var(--border))]">
              <div>
                <h3 className="text-lg font-semibold">选择技能</h3>
                <p className="text-xs text-[hsl(var(--muted-foreground))] mt-0.5">
                  该仓库包含 {gitCandidates.length} 个技能
                </p>
              </div>
              <button
                onClick={handlePickModalCancel}
                className="p-2 hover:bg-[hsl(var(--muted))] rounded-lg transition-colors"
              >
                <X size={18} className="text-[hsl(var(--muted-foreground))]" />
              </button>
            </div>

            <div className="max-h-64 overflow-y-auto p-4 space-y-2">
              {gitCandidates.map((candidate, index) => (
                <button
                  key={index}
                  onClick={() => handleCandidateSelect(candidate)}
                  className={`w-full flex items-center gap-3 p-3 rounded-lg border transition-all text-left ${
                    selectedCandidate?.subpath === candidate.subpath
                      ? "border-[hsl(var(--primary))] bg-[hsl(var(--primary))/10]"
                      : "border-[hsl(var(--border))] bg-[hsl(var(--card))] hover:border-[hsl(var(--ring))]"
                  }`}
                >
                  <div className="w-8 h-8 rounded-lg bg-[hsl(var(--primary))] flex items-center justify-center flex-shrink-0">
                    <GitBranch size={14} className="text-white" />
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="text-sm font-medium truncate">{candidate.name}</div>
                    {candidate.description && (
                      <div className="text-xs text-[hsl(var(--muted-foreground))] mt-0.5 line-clamp-1">
                        {candidate.description}
                      </div>
                    )}
                    <div className="text-xs text-[hsl(var(--muted-foreground))] mt-1 font-mono">
                      {candidate.subpath}
                    </div>
                  </div>
                </button>
              ))}
            </div>

            <div className="px-6 py-4 border-t border-[hsl(var(--border))] bg-[hsl(var(--muted)/30] flex gap-3">
              <button
                onClick={handlePickModalCancel}
                className="flex-1 px-4 py-2.5 bg-[hsl(var(--secondary))] hover:brightness-[0.95] text-[hsl(var(--secondary-foreground))] rounded-lg text-sm font-medium transition-all border border-[hsl(var(--border))]"
              >
                取消
              </button>
              <button
                onClick={handleCandidateConfirm}
                disabled={!selectedCandidate}
                className="flex-1 px-4 py-2.5 bg-[hsl(var(--primary))] hover:brightness-[0.9] text-white rounded-lg text-sm font-medium transition-all disabled:opacity-50"
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
