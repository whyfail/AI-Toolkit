import { useState, useEffect } from 'react';
import { X, Loader2, Github, Folder } from 'lucide-react';
import { toast } from 'sonner';
import type { ManagedSkill } from '../types';
import { skillsApi } from '@/lib/api';
import { Pressable } from '@/components/ui/Pressable';
import { Modal } from '@/components/ui/Modal';

interface EditSkillModalProps {
  open: boolean;
  skill: ManagedSkill | null;
  onClose: () => void;
  onSkillEdited: () => void;
}

function EditSkillModal({ open, skill, onClose, onSkillEdited }: EditSkillModalProps) {
  const [name, setName] = useState('');
  const [saving, setSaving] = useState(false);
  const [existingSkillNames, setExistingSkillNames] = useState<Set<string>>(new Set());

  // Sync state when skill changes
  useEffect(() => {
    if (skill) {
      setName(skill.name);
    }
  }, [skill]);

  useEffect(() => {
    if (!open) return;
    skillsApi.getManagedSkills()
      .then((skills) => setExistingSkillNames(new Set(skills.map((item) => item.name.toLowerCase()))))
      .catch(() => setExistingSkillNames(new Set()));
  }, [open]);

  if (!skill) return null;

  const isGithubSource = (skill.source_ref ?? '').startsWith('http://')
    || (skill.source_ref ?? '').startsWith('https://');

  const doSave = async () => {
    let finalName = name.trim();
    if (finalName.toLowerCase() !== skill.name.toLowerCase() && existingSkillNames.has(finalName.toLowerCase())) {
      // No native equivalent for `window.prompt` in Tauri webview; auto-rename
      // is the safer default that does not block the user.
      let index = 2;
      let nextName = `${finalName}-${index}`;
      while (existingSkillNames.has(nextName.toLowerCase())) {
        index += 1;
        nextName = `${finalName}-${index}`;
      }
      finalName = nextName;
      setName(nextName);
      toast.message(`技能名称已自动调整为 "${finalName}"`);
    }

    setSaving(true);
    try {
      await skillsApi.renameSkill({
        skillId: skill.id,
        newName: finalName,
        // 只改名，不改动来源地址（后端会整体覆写该字段）
        newSourceRef: skill.source_ref ?? null,
      });
      toast.success(`技能已重命名为 "${finalName}"`);
      onSkillEdited();
      onClose();
    } catch (err) {
      console.error('Failed to rename skill:', err);
      toast.error(`更新失败: ${err}`);
    } finally {
      setSaving(false);
    }
  };

  const handleSave = async () => {
    if (!name.trim()) {
      toast.error('技能名称不能为空');
      return;
    }
    await doSave();
  };

  return (
    <Modal open={open} onClose={onClose} size="md">
      <div className="w-full">
        {/* 头部 */}
        <div className="flex items-center justify-between border-b border-white/50 px-6 py-5 dark:border-white/10">
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-blue-600 to-sky-500 shadow-lg shadow-blue-500/15">
              {isGithubSource ? (
                <Github size={20} className="text-white" />
              ) : (
                <Folder size={20} className="text-white" />
              )}
            </div>
            <div>
              <h3 className="text-lg font-semibold">编辑技能</h3>
              <p className="text-xs text-slate-500 dark:text-slate-400">
                修改技能名称
              </p>
            </div>
          </div>
          <Pressable
            onClick={onClose}
            className="glass-icon-button"
          >
            <X size={18} />
          </Pressable>
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
        </div>

        {/* 底部 */}
        <div className="flex justify-end gap-3 border-t border-white/50 bg-white/25 px-6 py-4 dark:border-white/10 dark:bg-white/5">
          <Pressable
            onClick={onClose}
            className="glass-secondary-button"
          >
            取消
          </Pressable>
          <Pressable
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
          </Pressable>
        </div>
      </div>
    </Modal>
  );
}

export default EditSkillModal;
