use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolId {
    Cursor,
    ClaudeCode,
    Codex,
    OpenCode,
    Antigravity,
    Amp,
    KimiCli,
    Augment,
    Cline,
    CodeBuddy,
    CommandCode,
    Continue,
    Crush,
    Junie,
    IflowCli,
    KiroCli,
    Kode,
    McpJam,
    MistralVibe,
    Mux,
    OpenClaude,
    OpenHands,
    Pi,
    Qoder,
    QoderWork,
    QwenCode,
    Trae,
    TraeCn,
    Zencoder,
    Neovate,
    Pochi,
    AdaL,
    KiloCode,
    RooCode,
    Goose,
    GeminiCli,
    GithubCopilot,
    Clawdbot,
    Droid,
    Windsurf,
    Moltbot,
}

impl serde::Serialize for ToolId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_key())
    }
}

impl ToolId {
    pub fn as_key(&self) -> &'static str {
        match self {
            ToolId::Cursor => "cursor",
            ToolId::ClaudeCode => "claude_code",
            ToolId::Codex => "codex",
            ToolId::OpenCode => "opencode",
            ToolId::Antigravity => "antigravity",
            ToolId::Amp => "amp",
            ToolId::KimiCli => "kimi_cli",
            ToolId::Augment => "augment",
            ToolId::Cline => "cline",
            ToolId::CodeBuddy => "codebuddy",
            ToolId::CommandCode => "command_code",
            ToolId::Continue => "continue",
            ToolId::Crush => "crush",
            ToolId::Junie => "junie",
            ToolId::IflowCli => "iflow_cli",
            ToolId::KiroCli => "kiro_cli",
            ToolId::Kode => "kode",
            ToolId::McpJam => "mcpjam",
            ToolId::MistralVibe => "mistral_vibe",
            ToolId::Mux => "mux",
            ToolId::OpenClaude => "openclaude",
            ToolId::OpenHands => "openhands",
            ToolId::Pi => "pi",
            ToolId::Qoder => "qoder",
            ToolId::QoderWork => "qoderwork",
            ToolId::QwenCode => "qwen_code",
            ToolId::Trae => "trae",
            ToolId::TraeCn => "trae_cn",
            ToolId::Zencoder => "zencoder",
            ToolId::Neovate => "neovate",
            ToolId::Pochi => "pochi",
            ToolId::AdaL => "adal",
            ToolId::KiloCode => "kilo_code",
            ToolId::RooCode => "roo_code",
            ToolId::Goose => "goose",
            ToolId::GeminiCli => "gemini_cli",
            ToolId::GithubCopilot => "github_copilot",
            ToolId::Clawdbot => "clawdbot",
            ToolId::Droid => "droid",
            ToolId::Windsurf => "windsurf",
            ToolId::Moltbot => "moltbot",
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ToolAdapter {
    pub id: ToolId,
    pub display_name: &'static str,
    /// Global skill directory under user home (aligned with add-skill docs).
    pub relative_skills_dir: &'static str,
    /// Directory used to detect whether the tool is installed (aligned with add-skill docs).
    pub relative_detect_dir: &'static str,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct DetectedSkill {
    pub tool: ToolId,
    pub name: String,
    pub path: PathBuf,
    pub is_link: bool,
    pub link_target: Option<PathBuf>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ToolStatus {
    pub tool: ToolAdapter,
    pub installed: bool,
    pub skills: Vec<DetectedSkill>,
}

pub fn default_tool_adapters() -> Vec<ToolAdapter> {
    vec![
        ToolAdapter {
            id: ToolId::Cursor,
            display_name: "Cursor",
            relative_skills_dir: ".cursor/skills",
            relative_detect_dir: ".cursor",
        },
        ToolAdapter {
            id: ToolId::ClaudeCode,
            display_name: "Claude Code",
            relative_skills_dir: ".claude/skills",
            relative_detect_dir: ".claude",
        },
        ToolAdapter {
            id: ToolId::Codex,
            display_name: "Codex",
            relative_skills_dir: ".codex/skills",
            relative_detect_dir: ".codex",
        },
        ToolAdapter {
            id: ToolId::OpenCode,
            display_name: "OpenCode",
            // add-skill global path: ~/.config/opencode/skills/
            relative_skills_dir: ".config/opencode/skills",
            relative_detect_dir: ".config/opencode",
        },
        ToolAdapter {
            id: ToolId::Antigravity,
            display_name: "Antigravity",
            // add-skill global path: ~/.gemini/antigravity/global_skills/
            relative_skills_dir: ".gemini/antigravity/global_skills",
            relative_detect_dir: ".gemini/antigravity",
        },
        ToolAdapter {
            id: ToolId::Amp,
            display_name: "Amp",
            // add-skill global path: ~/.config/agents/skills/
            relative_skills_dir: ".config/agents/skills",
            relative_detect_dir: ".config/agents",
        },
        ToolAdapter {
            id: ToolId::KimiCli,
            display_name: "Kimi Code CLI",
            // add-skill global path: ~/.config/agents/skills/
            // NOTE: Shares the same skills directory with Amp.
            relative_skills_dir: ".config/agents/skills",
            relative_detect_dir: ".config/agents",
        },
        ToolAdapter {
            id: ToolId::Augment,
            display_name: "Augment",
            // add-skill global path: ~/.augment/rules/
            relative_skills_dir: ".augment/rules",
            relative_detect_dir: ".augment",
        },
        ToolAdapter {
            id: ToolId::Cline,
            display_name: "Cline",
            // add-skill global path: ~/.cline/skills/
            relative_skills_dir: ".cline/skills",
            relative_detect_dir: ".cline",
        },
        ToolAdapter {
            id: ToolId::CodeBuddy,
            display_name: "CodeBuddy",
            // add-skill global path: ~/.codebuddy/skills/
            relative_skills_dir: ".codebuddy/skills",
            relative_detect_dir: ".codebuddy",
        },
        ToolAdapter {
            id: ToolId::CommandCode,
            display_name: "Command Code",
            // add-skill global path: ~/.commandcode/skills/
            relative_skills_dir: ".commandcode/skills",
            relative_detect_dir: ".commandcode",
        },
        ToolAdapter {
            id: ToolId::Continue,
            display_name: "Continue",
            // add-skill global path: ~/.continue/skills/
            relative_skills_dir: ".continue/skills",
            relative_detect_dir: ".continue",
        },
        ToolAdapter {
            id: ToolId::Crush,
            display_name: "Crush",
            // add-skill global path: ~/.config/crush/skills/
            relative_skills_dir: ".config/crush/skills",
            relative_detect_dir: ".config/crush",
        },
        ToolAdapter {
            id: ToolId::Junie,
            display_name: "Junie",
            // add-skill global path: ~/.junie/skills/
            relative_skills_dir: ".junie/skills",
            relative_detect_dir: ".junie",
        },
        ToolAdapter {
            id: ToolId::IflowCli,
            display_name: "iFlow CLI",
            // add-skill global path: ~/.iflow/skills/
            relative_skills_dir: ".iflow/skills",
            relative_detect_dir: ".iflow",
        },
        ToolAdapter {
            id: ToolId::KiroCli,
            display_name: "Kiro CLI",
            // add-skill global path: ~/.kiro/skills/
            relative_skills_dir: ".kiro/skills",
            relative_detect_dir: ".kiro",
        },
        ToolAdapter {
            id: ToolId::Kode,
            display_name: "Kode",
            // add-skill global path: ~/.kode/skills/
            relative_skills_dir: ".kode/skills",
            relative_detect_dir: ".kode",
        },
        ToolAdapter {
            id: ToolId::McpJam,
            display_name: "MCPJam",
            // add-skill global path: ~/.mcpjam/skills/
            relative_skills_dir: ".mcpjam/skills",
            relative_detect_dir: ".mcpjam",
        },
        ToolAdapter {
            id: ToolId::MistralVibe,
            display_name: "Mistral Vibe",
            // add-skill global path: ~/.vibe/skills/
            relative_skills_dir: ".vibe/skills",
            relative_detect_dir: ".vibe",
        },
        ToolAdapter {
            id: ToolId::Mux,
            display_name: "Mux",
            // add-skill global path: ~/.mux/skills/
            relative_skills_dir: ".mux/skills",
            relative_detect_dir: ".mux",
        },
        ToolAdapter {
            id: ToolId::OpenClaude,
            display_name: "OpenClaude IDE",
            // add-skill global path: ~/.openclaude/skills/
            relative_skills_dir: ".openclaude/skills",
            relative_detect_dir: ".openclaude",
        },
        ToolAdapter {
            id: ToolId::OpenHands,
            display_name: "OpenHands",
            // add-skill global path: ~/.openhands/skills/
            relative_skills_dir: ".openhands/skills",
            relative_detect_dir: ".openhands",
        },
        ToolAdapter {
            id: ToolId::Pi,
            display_name: "Pi",
            // add-skill global path: ~/.pi/agent/skills/
            relative_skills_dir: ".pi/agent/skills",
            relative_detect_dir: ".pi",
        },
        ToolAdapter {
            id: ToolId::Qoder,
            display_name: "Qoder",
            // add-skill global path: ~/.qoder/skills/
            relative_skills_dir: ".qoder/skills",
            relative_detect_dir: ".qoder",
        },
        ToolAdapter {
            id: ToolId::QoderWork,
            display_name: "QoderWork",
            // add-skill global path: ~/.qoderwork/skills/
            relative_skills_dir: ".qoderwork/skills",
            relative_detect_dir: ".qoderwork",
        },
        ToolAdapter {
            id: ToolId::QwenCode,
            display_name: "Qwen Code",
            // add-skill global path: ~/.qwen/skills/
            relative_skills_dir: ".qwen/skills",
            relative_detect_dir: ".qwen",
        },
        ToolAdapter {
            id: ToolId::Trae,
            display_name: "Trae",
            // add-skill global path: ~/.trae/skills/
            relative_skills_dir: ".trae/skills",
            relative_detect_dir: ".trae",
        },
        ToolAdapter {
            id: ToolId::TraeCn,
            display_name: "Trae CN",
            // add-skill global path: ~/.trae-cn/skills/
            relative_skills_dir: ".trae-cn/skills",
            relative_detect_dir: ".trae-cn",
        },
        ToolAdapter {
            id: ToolId::Zencoder,
            display_name: "Zencoder",
            // add-skill global path: ~/.zencoder/skills/
            relative_skills_dir: ".zencoder/skills",
            relative_detect_dir: ".zencoder",
        },
        ToolAdapter {
            id: ToolId::Neovate,
            display_name: "Neovate",
            // add-skill global path: ~/.neovate/skills/
            relative_skills_dir: ".neovate/skills",
            relative_detect_dir: ".neovate",
        },
        ToolAdapter {
            id: ToolId::Pochi,
            display_name: "Pochi",
            // add-skill global path: ~/.pochi/skills/
            relative_skills_dir: ".pochi/skills",
            relative_detect_dir: ".pochi",
        },
        ToolAdapter {
            id: ToolId::AdaL,
            display_name: "AdaL",
            // add-skill global path: ~/.adal/skills/
            relative_skills_dir: ".adal/skills",
            relative_detect_dir: ".adal",
        },
        ToolAdapter {
            id: ToolId::KiloCode,
            display_name: "Kilo Code",
            // add-skill global path: ~/.kilocode/skills/
            relative_skills_dir: ".kilocode/skills",
            relative_detect_dir: ".kilocode",
        },
        ToolAdapter {
            id: ToolId::RooCode,
            display_name: "Roo Code",
            // add-skill global path: ~/.roo/skills/
            relative_skills_dir: ".roo/skills",
            relative_detect_dir: ".roo",
        },
        ToolAdapter {
            id: ToolId::Goose,
            display_name: "Goose",
            // add-skill global path: ~/.config/goose/skills/
            relative_skills_dir: ".config/goose/skills",
            relative_detect_dir: ".config/goose",
        },
        ToolAdapter {
            id: ToolId::GeminiCli,
            display_name: "Gemini CLI",
            // add-skill global path: ~/.gemini/skills/
            relative_skills_dir: ".gemini/skills",
            relative_detect_dir: ".gemini",
        },
        ToolAdapter {
            id: ToolId::GithubCopilot,
            display_name: "GitHub Copilot",
            // add-skill global path: ~/.copilot/skills/
            relative_skills_dir: ".copilot/skills",
            relative_detect_dir: ".copilot",
        },
        ToolAdapter {
            id: ToolId::Clawdbot,
            display_name: "Clawdbot",
            // add-skill global path: ~/.clawdbot/skills/
            relative_skills_dir: ".clawdbot/skills",
            relative_detect_dir: ".clawdbot",
        },
        ToolAdapter {
            id: ToolId::Droid,
            display_name: "Droid",
            // add-skill global path: ~/.factory/skills/
            relative_skills_dir: ".factory/skills",
            relative_detect_dir: ".factory",
        },
        ToolAdapter {
            id: ToolId::Windsurf,
            display_name: "Windsurf",
            // add-skill global path: ~/.codeium/windsurf/skills/
            relative_skills_dir: ".codeium/windsurf/skills",
            relative_detect_dir: ".codeium/windsurf",
        },
        ToolAdapter {
            id: ToolId::Moltbot,
            display_name: "MoltBot",
            // add-skill global path: ~/.moltbot/skills/
            relative_skills_dir: ".moltbot/skills",
            relative_detect_dir: ".moltbot",
        },
    ]
}

/// Tools can share the same global skills directory (e.g. Amp and Kimi Code CLI).
/// Use this to coordinate UI warnings and avoid duplicate filesystem operations.
pub fn adapters_sharing_skills_dir(adapter: &ToolAdapter) -> Vec<ToolAdapter> {
    default_tool_adapters()
        .into_iter()
        .filter(|a| a.relative_skills_dir == adapter.relative_skills_dir)
        .collect()
}

pub fn adapter_by_key(key: &str) -> Option<ToolAdapter> {
    default_tool_adapters()
        .into_iter()
        .find(|adapter| adapter.id.as_key() == key)
}

pub fn resolve_default_path(adapter: &ToolAdapter) -> Result<PathBuf> {
    let home = dirs::home_dir().context("failed to resolve home directory")?;
    Ok(home.join(adapter.relative_skills_dir))
}

pub fn resolve_detect_path(adapter: &ToolAdapter) -> Result<PathBuf> {
    let home = dirs::home_dir().context("failed to resolve home directory")?;
    Ok(home.join(adapter.relative_detect_dir))
}

pub fn is_tool_installed(adapter: &ToolAdapter) -> Result<bool> {
    Ok(resolve_detect_path(adapter)?.exists())
}

pub fn scan_tool_dir(tool: &ToolAdapter, dir: &Path) -> Result<Vec<DetectedSkill>> {
    let mut results = Vec::new();
    if !dir.exists() {
        return Ok(results);
    }

    let ignore_hint = "Application Support/com.tauri.dev/skills";

    for entry in std::fs::read_dir(dir).with_context(|| format!("read dir {:?}", dir))? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        let is_dir = file_type.is_dir() || (file_type.is_symlink() && path.is_dir());
        if !is_dir {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if tool.id == ToolId::Codex && name == ".system" {
            continue;
        }
        let (is_link, link_target) = detect_link(&path);
        if path.to_string_lossy().contains(ignore_hint)
            || link_target
                .as_ref()
                .map(|p| p.to_string_lossy().contains(ignore_hint))
                .unwrap_or(false)
        {
            continue;
        }
        results.push(DetectedSkill {
            tool: tool.id.clone(),
            name,
            path,
            is_link,
            link_target,
        });
    }

    Ok(results)
}

fn detect_link(path: &Path) -> (bool, Option<PathBuf>) {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let target = std::fs::read_link(path).ok();
            (true, target)
        }
        _ => {
            let target = std::fs::read_link(path).ok();
            if target.is_some() {
                (true, target)
            } else {
                (false, None)
            }
        }
    }
}

pub fn get_all_tool_status() -> Result<Vec<ToolStatus>> {
    let mut tool_statuses = Vec::new();

    for tool in default_tool_adapters() {
        let installed = is_tool_installed(&tool)?;
        let skills_dir = resolve_default_path(&tool)?;
        let skills = scan_tool_dir(&tool, &skills_dir)?;

        tool_statuses.push(ToolStatus {
            tool,
            installed,
            skills,
        });
    }

    Ok(tool_statuses)
}
