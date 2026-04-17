/**
 * 统一工具定义
 * 所有模块共享这套工具定义，与 README 支持的 11 种工具保持一致
 */

// 工具 ID 类型（使用 kebab-case，与 AppType serde 名一致）
export type ToolId =
  | 'qwen-code'
  | 'claude'
  | 'codex'
  | 'gemini'
  | 'opencode'
  | 'trae'
  | 'trae-cn'
  | 'trae-solo-cn'
  | 'qoder'
  | 'qodercli'
  | 'codebuddy';

// 工具元数据
export interface ToolMeta {
  id: ToolId;
  name: string;
  displayName: string;
  skillsDir: string;       // skills 目录（用于同步）
  detectDir: string;       // 检测目录（用于判断是否安装）
  binaryName: string;      // CLI 命令名
}

// 所有支持的工具列表
export const SUPPORTED_TOOLS: ToolMeta[] = [
  {
    id: 'qwen-code',
    name: 'qwen-code',
    displayName: 'Qwen Code',
    skillsDir: '.qwen/skills',
    detectDir: '.qwen',
    binaryName: 'qwen',
  },
  {
    id: 'claude',
    name: 'claude',
    displayName: 'Claude Code',
    skillsDir: '.claude/skills',
    detectDir: '.claude',
    binaryName: 'claude',
  },
  {
    id: 'codex',
    name: 'codex',
    displayName: 'Codex',
    skillsDir: '.codex/skills',
    detectDir: '.codex',
    binaryName: 'codex',
  },
  {
    id: 'gemini',
    name: 'gemini',
    displayName: 'Gemini CLI',
    skillsDir: '.gemini/skills',
    detectDir: '.gemini',
    binaryName: 'gemini',
  },
  {
    id: 'opencode',
    name: 'opencode',
    displayName: 'OpenCode',
    skillsDir: '.config/opencode/skills',
    detectDir: '.config/opencode',
    binaryName: 'opencode',
  },
  {
    id: 'qoder',
    name: 'qoder',
    displayName: 'Qoder',
    skillsDir: '.qoder/skills',
    detectDir: '.qoder',
    binaryName: 'qoder',
  },
  {
    id: 'qodercli',
    name: 'qodercli',
    displayName: 'Qoder CLI',
    skillsDir: '.qodercli/skills',
    detectDir: '.qodercli',
    binaryName: 'qodercli',
  },
  {
    id: 'trae',
    name: 'trae',
    displayName: 'Trae',
    skillsDir: '.trae/skills',
    detectDir: '.trae',
    binaryName: 'trae',
  },
  {
    id: 'trae-cn',
    name: 'trae-cn',
    displayName: 'Trae CN',
    skillsDir: '.trae-cn/skills',
    detectDir: '.trae-cn',
    binaryName: 'trae',
  },
  {
    id: 'trae-solo-cn',
    name: 'trae-solo-cn',
    displayName: 'TRAE SOLO CN',
    skillsDir: '.trae-solo-cn/skills',
    detectDir: '.trae-solo-cn',
    binaryName: 'trae',
  },
  {
    id: 'codebuddy',
    name: 'codebuddy',
    displayName: 'CodeBuddy',
    skillsDir: '.codebuddy/skills',
    detectDir: '.codebuddy',
    binaryName: 'codebuddy',
  },
];

// 工具颜色映射
export const APP_COLORS: Record<ToolId, string> = {
  'qwen-code': 'bg-purple-500',
  'claude': 'bg-orange-500',
  'codex': 'bg-blue-500',
  'gemini': 'bg-green-500',
  'opencode': 'bg-cyan-500',
  'trae': 'bg-indigo-500',
  'trae-cn': 'bg-violet-500',
  'trae-solo-cn': 'bg-fuchsia-500',
  'qoder': 'bg-yellow-500',
  'qodercli': 'bg-amber-500',
  'codebuddy': 'bg-red-500',
};

// 根据 ID 获取工具元数据
export function getToolMeta(id: string): ToolMeta | undefined {
  return SUPPORTED_TOOLS.find(t => t.id === id);
}

// 根据 binary 名获取工具元数据
export function getToolMetaByBinary(binaryName: string): ToolMeta | undefined {
  return SUPPORTED_TOOLS.find(t => t.binaryName === binaryName);
}

// 获取所有可启动的工具（具有 CLI 命令）
export const LAUNCHABLE_TOOLS: ToolId[] = [
  'qwen-code', 'claude', 'codex', 'gemini', 'opencode', 'qodercli', 'codebuddy'
];

// 判断工具是否可启动
export function isLaunchable(toolId: string): boolean {
  return LAUNCHABLE_TOOLS.includes(toolId as ToolId);
}

// 获取共享 skills 目录的工具组
// 例如：Qoder 和 Qoder CLI 共享目录的情况
export function getToolsSharingSkillsDir(toolsDir: string): ToolMeta[] {
  return SUPPORTED_TOOLS.filter(t => t.skillsDir === toolsDir);
}
