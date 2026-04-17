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

// 支持的工具 ID（与 README 和 AppType 保持一致）
export type ToolId =
  | 'qwen_code'
  | 'claude_code'
  | 'codex'
  | 'gemini_cli'
  | 'opencode'
  | 'qoder'
  | 'kilo_code'   // Qoder CLI (使用 kilo_code 作为 skills 目录标识)
  | 'trae'
  | 'trae_cn'
  | 'neovate'      // TRAE SOLO CN
  | 'codebuddy'

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