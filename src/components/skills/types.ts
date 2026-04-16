export type OnboardingVariant = {
  tool: string
  name: string
  path: string
  fingerprint?: string | null
  is_link: boolean
  link_target?: string | null
}

export type OnboardingGroup = {
  name: string
  variants: OnboardingVariant[]
  has_conflict: boolean
}

export type OnboardingPlan = {
  total_tools_scanned: number
  total_skills_found: number
  groups: OnboardingGroup[]
}

export type ToolOption = {
  id: string
  label: string
}

export type ManagedSkill = {
  id: string
  name: string
  description?: string | null
  source_type: string
  source_ref?: string | null
  central_path: string
  created_at: number
  updated_at: number
  last_sync_at?: number | null
  status: string
  targets: {
    tool: string
    mode: string
    status: string
    target_path: string
    synced_at?: number | null
  }[]
}

export type GitSkillCandidate = {
  name: string
  description?: string | null
  subpath: string
}

export type LocalSkillCandidate = {
  name: string
  description?: string | null
  subpath: string
  valid: boolean
  reason?: string | null
}

export type InstallResultDto = {
  skill_id: string
  name: string
  central_path: string
  content_hash?: string | null
}

export type ToolId = 
  | 'cursor'
  | 'claude_code'
  | 'codex'
  | 'opencode'
  | 'antigravity'
  | 'amp'
  | 'kimi_cli'
  | 'augment'
  | 'cline'
  | 'codebuddy'
  | 'command_code'
  | 'continue'
  | 'crush'
  | 'junie'
  | 'iflow_cli'
  | 'kiro_cli'
  | 'kode'
  | 'mcpjam'
  | 'mistral_vibe'
  | 'mux'
  | 'openclaude'
  | 'openhands'
  | 'pi'
  | 'qoder'
  | 'qoderwork'
  | 'qwen_code'
  | 'trae'
  | 'trae_cn'
  | 'zencoder'
  | 'neovate'
  | 'pochi'
  | 'adal'
  | 'kilo_code'
  | 'roo_code'
  | 'goose'
  | 'gemini_cli'
  | 'github_copilot'
  | 'clawdbot'
  | 'droid'
  | 'windsurf'
  | 'moltbot'

export type ToolAdapter = {
  id: ToolId
  display_name: string
  relative_skills_dir: string
  relative_detect_dir: string
}

export type DetectedSkill = {
  tool: ToolId
  name: string
  path: string
  is_link: boolean
  link_target?: string | null
}

export type ToolStatus = {
  tool: ToolAdapter
  installed: boolean
  skills: DetectedSkill[]
}

export type ToolStatusDto = ToolStatus[]

export type UpdateResultDto = {
  skill_id: string
  name: string
  content_hash?: string | null
  source_revision?: string | null
  updated_targets: string[]
}

export type FeaturedSkillDto = {
  slug: string
  name: string
  summary: string
  downloads: number
  stars: number
  source_url: string
}

export type OnlineSkillDto = {
  name: string
  installs: number
  source: string
  source_url: string
}

export type SkillFileEntry = {
  path: string
  size: number
}