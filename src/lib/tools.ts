/**
 * 统一工具定义
 * 所有模块共享这套工具定义，覆盖 CLI、桌面应用和 IDE 扩展，共用配置的客户端使用统一 ID
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
  | 'qoder'
  | 'qodercli'
  | 'codebuddy'
  | 'hermes'
  | 'mimo-code'
  | 'kimi-code'
  | 'minimax-code'
  | 'deepseek-harness'
  | 'workbuddy'
  | 'zcode'
  | 'cursor'
  | 'windsurf'
  | 'cline'
  | 'roo'
  | 'kilo'
  | 'continue'
  | 'goose'
  | 'kiro';

// 工具元数据
export interface ToolMeta {
  id: ToolId;
  name: string;
  displayName: string;
  skillsDir: string;       // skills 目录（用于同步）
  detectDir: string;       // 检测目录（用于判断是否安装）
  binaryName: string;      // CLI 命令名
  docsUrl: string;         // 使用文档链接
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
    docsUrl: 'https://qwenlm.github.io/qwen-code-docs/en/',
  },
  {
    id: 'claude',
    name: 'claude',
    displayName: 'Claude Code',
    skillsDir: '.claude/skills',
    detectDir: '.claude',
    binaryName: 'claude',
    docsUrl: 'https://code.claude.com/docs',
  },
  {
    id: 'codex',
    name: 'codex',
    displayName: 'Codex',
    skillsDir: '.codex/skills',
    detectDir: '.codex',
    binaryName: 'codex',
    docsUrl: 'https://github.com/openai/codex',
  },
  {
    id: 'gemini',
    name: 'gemini',
    displayName: 'Gemini CLI',
    skillsDir: '.gemini/skills',
    detectDir: '.gemini',
    binaryName: 'gemini',
    docsUrl: 'https://github.com/google-gemini/gemini-cli/tree/main/docs',
  },
  {
    id: 'opencode',
    name: 'opencode',
    displayName: 'OpenCode',
    skillsDir: '.config/opencode/skills',
    detectDir: '.config/opencode',
    binaryName: 'opencode',
    docsUrl: 'https://opencode.ai/docs',
  },
  {
    id: 'qoder',
    name: 'qoder',
    displayName: 'Qoder',
    skillsDir: '.qoder/skills',
    detectDir: '.qoder',
    binaryName: 'qoder',
    docsUrl: 'https://docs.qoder.com/qoderwork/introduction',
  },
  {
    id: 'qodercli',
    name: 'qodercli',
    displayName: 'Qoder CLI',
    skillsDir: '.qoder/skills',
    detectDir: '.qoder',
    binaryName: 'qodercli',
    docsUrl: 'https://docs.qoder.com/extensions/skills',
  },
  {
    id: 'trae',
    name: 'trae',
    displayName: 'TRAE',
    skillsDir: '.trae/skills',
    detectDir: '.trae',
    binaryName: 'trae',
    docsUrl: 'https://docs.trae.ai/ide/add-mcp-servers',
  },
  {
    id: 'trae-cn',
    name: 'trae-cn',
    displayName: 'TRAE CN',
    skillsDir: '.trae-cn/skills',
    detectDir: '.trae-cn',
    binaryName: 'trae',
    docsUrl: 'https://docs.trae.cn/ide_add-mcp-servers',
  },
  {
    id: 'codebuddy',
    name: 'codebuddy',
    displayName: 'CodeBuddy',
    skillsDir: '.codebuddy/skills',
    detectDir: '.codebuddy',
    binaryName: 'codebuddy',
    docsUrl: 'https://www.codebuddy.ai/docs/cli/cli-reference',
  },
  {
    id: 'hermes',
    name: 'hermes',
    displayName: 'Hermes Agent',
    skillsDir: '.hermes/skills',
    detectDir: '.hermes',
    binaryName: 'hermes',
    docsUrl: 'https://hermes-agent.nousresearch.com/docs/',
  },
  {
    id: 'mimo-code',
    name: 'mimo-code',
    displayName: 'Mimo Code',
    skillsDir: '.config/mimocode/skills',
    detectDir: '.config/mimocode',
    binaryName: 'mimo',
    docsUrl: 'https://mimo.xiaomi.com/mimocode/install',
  },
  {
    id: 'kimi-code',
    name: 'kimi-code',
    displayName: 'Kimi Code',
    skillsDir: '.kimi-code/skills',
    detectDir: '.kimi-code',
    binaryName: 'kimi',
    docsUrl: 'https://github.com/MoonshotAI/kimi-code',
  },
  {
    id: 'minimax-code',
    name: 'minimax-code',
    displayName: 'MiniMax Code',
    skillsDir: '.minimax/skills',
    detectDir: '.minimax',
    binaryName: 'mcode',
    docsUrl: 'https://github.com/MiniMax-AI/minimax-code',
  },
  {
    id: 'deepseek-harness',
    name: 'deepseek-harness',
    displayName: 'DeepSeek Harness',
    skillsDir: '.dsh/skills',
    detectDir: '.dsh',
    binaryName: 'dsh',
    docsUrl: 'https://github.com/deepseek-ai/deepseek-harness',
  },
  {
    id: 'workbuddy',
    name: 'workbuddy',
    displayName: 'WorkBuddy',
    skillsDir: '.workbuddy/skills',
    detectDir: '.workbuddy',
    binaryName: 'workbuddy',
    docsUrl: 'https://www.workbuddy.ai/',
  },
  {
    id: 'zcode',
    name: 'zcode',
    displayName: 'ZCode',
    skillsDir: '.zcode/skills',
    detectDir: '.zcode',
    binaryName: 'zcode',
    docsUrl: 'https://docs.z.ai',
  },
  {
    id: 'cursor',
    name: 'cursor',
    displayName: 'Cursor',
    skillsDir: '.cursor/skills',
    detectDir: '.cursor',
    binaryName: 'cursor-agent',
    docsUrl: 'https://cursor.com/docs/mcp',
  },
  {
    id: 'windsurf',
    name: 'windsurf',
    displayName: 'Windsurf',
    skillsDir: '.codeium/windsurf/skills',
    detectDir: '.codeium/windsurf',
    binaryName: 'windsurf',
    docsUrl: 'https://docs.windsurf.com/windsurf/cascade/mcp',
  },
  {
    id: 'cline',
    name: 'cline',
    displayName: 'Cline',
    skillsDir: '.cline/skills',
    detectDir: '.cline',
    binaryName: 'cline',
    docsUrl: 'https://docs.cline.bot/getting-started/config',
  },
  {
    id: 'roo',
    name: 'roo',
    displayName: 'Roo Code',
    skillsDir: '.roo/skills',
    detectDir: '.roo',
    binaryName: '',
    docsUrl: 'https://docs.roocode.com/features/mcp/using-mcp-in-roo',
  },
  {
    id: 'kilo',
    name: 'kilo',
    displayName: 'Kilo Code',
    skillsDir: '.kilo/skills',
    detectDir: '.config/kilo',
    binaryName: 'kilo',
    docsUrl: 'https://kilo.ai/docs/automate/mcp/using-in-cli',
  },
  {
    id: 'continue',
    name: 'continue',
    displayName: 'Continue',
    skillsDir: '.continue/skills',
    detectDir: '.continue',
    binaryName: 'cn',
    docsUrl: 'https://docs.continue.dev/cli/configuration',
  },
  {
    id: 'goose',
    name: 'goose',
    displayName: 'Goose',
    skillsDir: '.config/goose/skills',
    detectDir: '.config/goose',
    binaryName: 'goose',
    docsUrl: 'https://goose.vcorp.ai/docs/getting-started/using-extensions/',
  },
  {
    id: 'kiro',
    name: 'kiro',
    displayName: 'Kiro',
    skillsDir: '.kiro/skills',
    detectDir: '.kiro',
    binaryName: 'kiro-cli',
    docsUrl: 'https://kiro.dev/docs/cli/chat/configuration/',
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
  'qoder': 'bg-yellow-500',
  'qodercli': 'bg-amber-500',
  'codebuddy': 'bg-red-500',
  'hermes': 'bg-teal-500',
  'mimo-code': 'bg-rose-500',
  'kimi-code': 'bg-slate-900',
  'minimax-code': 'bg-orange-500',
  'deepseek-harness': 'bg-blue-600',
  'workbuddy': 'bg-lime-600',
  'zcode': 'bg-emerald-500',
  'cursor': 'bg-slate-500',
  'windsurf': 'bg-teal-500',
  'cline': 'bg-amber-500',
  'roo': 'bg-sky-500',
  'kilo': 'bg-yellow-500',
  'continue': 'bg-green-600',
  'goose': 'bg-orange-600',
  'kiro': 'bg-fuchsia-500',
};

// 旧版 Work ID 仅用于读取历史记录，新配置使用统一 TRAE ID。
export function normalizeToolId(id: string): string {
  if (id === 'trae-work') return 'trae';
  if (id === 'trae-solo-cn') return 'trae-cn';
  if (id === 'workbuddy-cn') return 'workbuddy';
  if (id === 'codex-cli' || id === 'codex-desktop') return 'codex';
  return id;
}

// 根据 ID 获取工具元数据
export function getToolMeta(id: string): ToolMeta | undefined {
  return SUPPORTED_TOOLS.find(t => t.id === normalizeToolId(id));
}

// 根据 binary 名获取工具元数据
export function getToolMetaByBinary(binaryName: string): ToolMeta | undefined {
  return SUPPORTED_TOOLS.find(t => t.binaryName === binaryName);
}

// 获取共享 skills 目录的工具组
// 例如：Qoder 和 Qoder CLI 共享目录的情况
export function getToolsSharingSkillsDir(toolsDir: string): ToolMeta[] {
  return SUPPORTED_TOOLS.filter(t => t.skillsDir === toolsDir);
}
