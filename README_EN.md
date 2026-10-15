# AI Toolkit

<div align="center">

[![Version](https://img.shields.io/badge/version-1.8.0-blue.svg)](https://github.com)
[![Platform](https://img.shields.io/badge/platform-macOS%2012%2B-lightgrey.svg)](https://github.com)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%202-orange.svg)](https://tauri.app/)

[中文](README.md) | [English](README_EN.md)

</div>

## 📖 Introduction

AI Toolkit is a **universal AI programming tools management tool** that supports unified MCP server configuration and Skills synchronization. Say goodbye to tedious manual editing—one app to rule them all.

## ✨ Key Features

### 🎯 MCP Server Management
- Support for **22** mainstream AI programming tools: Qwen Code, Claude Code, Codex, Gemini CLI, OpenCode, TRAE, TRAE CN, Qoder, Qoder CLI, CodeBuddy, Hermes Agent, Mimo Code, WorkBuddy, ZCode, Cursor, Windsurf, Cline, Roo Code, Kilo Code, Continue, Goose, Kiro
- Add, edit, and delete MCP servers in a single interface
- Automatically detects installed AI tools on your system and prompts for MCP sync when new tools are discovered
- Toggle switches **sync in real-time** to the corresponding tool's configuration file
- **JSON Paste Mode**: Copy JSON configuration directly from an MCP introduction page and paste to recognize
- **Batch Import Preview**: Paste multiple MCP Server configs and import selected entries at once
- **Connection Testing**: Built-in test connection function to ensure server configurations are valid before saving

### 🧰 Skills Management
- **Skills Panel**: Dedicated Skills management interface for centralized skill management
- **Install Location Switch**: Choose the official `~/.agents/skills/` directory or `~/.ai-toolkit/skills/` as the central Skills location
- **Batch Sync**: Click to select multiple Skills in the sync modal and sync them to multiple target tools at once
- **Git Installation**: Install Skills from GitHub/GitLab repositories with automatic repository structure parsing
- **Online Search**: Search trending Skills from skills.sh
- **Featured Recommendations**: Browse featured Skills with install counts and star information
- **One-Click Update**: Auto-detect Skills with updates available and quick update support
- **Health Check and Delete Preview**: Check Skill docs, paths, sync targets, and preview affected paths before deletion
- **Sync Progress and Conflict Strategy**: Batch sync shows per-item progress; duplicate Skill names can be renamed, skipped, or overwritten

### 🛡️ Enhancement Center
- **MCP and Tool Config Snapshots**: Back up MCP Server data and tool MCP config files with one-click restore
- **Health Checks**: Validate MCP commands, environment variables, config files, and sync status
- **Zip Package Import/Export**: Export MCP, Skills files, and settings as a `.zip` package without pushing large file contents into the page
- **Task Logs**: Track local scans, Skills sync, and import results
- **Agent Launch Presets**: Save tool, working directory, and MCP combinations for one-click launches

### 🔧 Developer Friendly
- Click on a tool name to quickly open the corresponding configuration file
- Visual interface, goodbye manual editing of JSON/TOML files
- Automatic recognition of multiple configuration file paths
- **Atomic Writing**: Temporary file + rename mechanism to prevent configuration corruption
- **Code Quality Checks**: Added `pnpm lint` / `pnpm check` powered by Oxlint and TypeScript

### 🚀 Quick Agent Launch
- **One-Click Launch**: Start AI tools directly from a terminal

### 🔎 Local Agent Discovery
- Display only installed CLI tools, desktop apps, and IDE extensions; no Agent download, install, update, or uninstall actions
- Scan automatically at startup or refresh from the sidebar, Skills, or MCP page; every module shares the same installation report
- Detect executables, app bundles, and IDE extension manifests; leftover config and Skills directories do not count as installations
- Codex CLI, desktop, and IDE share one Codex entry; both WorkBuddy editions share one WorkBuddy entry
- Added MCP and Skills adapters for Cline, Roo Code, Kilo Code, Continue, Goose, Cursor, Windsurf, and Kiro
- Kimi Code and MiniMax Code support desktop and CLI discovery, official icons, MCP, and Skills sync
- See [Agent adapter paths and official sources](docs/agent-adapters.md)

### 🔄 App Updates & Sharing
- **Startup Update Check**: Automatically checks for new releases on first launch and lets users decide whether to install
- **One-Click Sharing**: Copy the official website URL from Settings to share the app easily

### 📌 System Tray
- Closing the main window keeps the app in the system tray; an active proxy and background tasks continue running
- Restore the window from the tray's Open Main Window menu item, or launch the app again to activate the existing instance
- Quit App (Stop Proxy) exits the app and stops the proxy; `Cmd+Q` on macOS also exits

## 📸 Screenshots

### Main Panel
![Main Panel](assets/screenshots/main-panel.png)

## 🖥️ System Support

| System | Status | Description |
|--------|--------|-------------|
| **macOS 12+** | ✅ Supported | Full feature support |
| **Linux** | 🚧 In Progress | Basic functionality available |
| **Windows 10+** | ✅ Supported | MCP, Skills, and tool launching are supported |

## 🚀 Quick Start

### macOS Installation

Download the latest `AI Toolkit_x.x.x_aarch64.dmg` installer from the [Releases](https://github.com/whyfail/ai-toolkit/releases) page:

```bash
# Mount DMG
hdiutil attach AI\ Toolkit_*.dmg

# Drag to Applications folder
cp -R /Volumes/AI\ Toolkit/AI\ Toolkit.app /Applications/
```

### ⚠️ macOS Security Warning (Required for First Run)

Since the current version is not code-signed or notarized by Apple, macOS Gatekeeper may block it on first launch, showing **"Cannot be verified"** or **"File is damaged"**. Follow these steps to allow it:

**Method 1 (Terminal Command - Recommended):**
1. Drag the app to your `/Applications` folder
2. Open **Terminal** and run the following command:
   ```bash
   sudo xattr -cr "/Applications/AI Toolkit.app"
   ```
3. Enter your Mac password and press Enter (characters won't be displayed). Once the command finishes, you can double-click to open.

**Method 2 (Right-Click Open):**
1. Locate `AI Toolkit.app` in **Finder**
2. **Right-click** (or `Control + Click`) the app icon
3. Select **"Open"** from the context menu
4. Click **"Open"** again in the system warning dialog

**Method 3 (System Settings):**
1. Open **System Settings** -> **Privacy & Security**
2. Scroll down to the **Security** section
3. Find the message `"AI Toolkit" was blocked from use...`
4. Click **"Open Anyway"** and enter your password if prompted

## 📁 Supported AI Tools & Configuration Paths

| Tool | Configuration Path |
|------|-------------------|
| Qwen Code | `~/.qwen/settings.json` |
| Claude Code | `~/.claude.json` |
| OpenAI Codex | `~/.codex/config.toml` |
| Google Gemini CLI | `~/.gemini/settings.json` |
| OpenCode | `~/.config/opencode/opencode.json` |
| Qoder | `~/Library/Application Support/Qoder/SharedClientCache/mcp.json` |
| Qoder CLI | `~/.qodercli/settings.json` |
| TRAE (International) | `~/Library/Application Support/Trae/User/mcp.json` |
| TRAE CN (China) | `~/Library/Application Support/Trae CN/User/mcp.json` |
| CodeBuddy | `~/.codebuddy/mcp.json` |
| Hermes Agent | `~/.hermes/config.yaml` |
| Mimo Code | `~/.config/mimocode/mimocode.json` |
| WorkBuddy | `~/.workbuddy/.mcp.json` |
| ZCode | `~/.zcode/cli/config.json` |
| Cursor | `~/.cursor/mcp.json` |
| Windsurf | `~/.codeium/windsurf/mcp_config.json` / `~/.config/devin/mcp_config.json` |
| Cline | `~/.cline/data/settings/cline_mcp_settings.json` |
| Roo Code | `<IDE User>/globalStorage/rooveterinaryinc.roo-cline/settings/mcp_settings.json` |
| Kilo Code | `~/.config/kilo/kilo.json` / `kilo.jsonc` |
| Continue | `~/.continue/config.yaml` |
| Goose | `~/.config/goose/config.yaml` / `~/Library/Application Support/Block/goose/config.yaml` |
| Kiro | `~/.kiro/settings/mcp.json` |

The international and China editions of WorkBuddy share the same application name and the same `~/.workbuddy` configuration, MCP, and Skills directories, so they cannot coexist. Installing the other edition replaces the application without deleting shared data. On Windows, the shared paths are `%USERPROFILE%\.workbuddy\.mcp.json` and `%USERPROFILE%\.workbuddy\skills\`.

TRAE has two management entries: the international edition and the China edition. Agent and IDE in each new edition share local MCP configuration and the Skills directory (`~/.trae/skills` internationally and `~/.trae-cn/skills` in China), so Work is no longer a separate sync target. Legacy `TRAE Work`, `TRAE Work CN`, and `TRAE SOLO CN` installations and configuration remain detectable and can be previewed and migrated to the new edition. Existing configuration IDs and synced Skills records remain readable. This integration manages local configuration; cloud Agents are outside its scope.

Use “迁移旧 TRAE” on the MCP page to preview and select legacy configuration. Conflicting server IDs require a new name. Migration creates a backup and supports undo, which stops if later edits would be overwritten. Legacy Windows skills in `%APPDATA%\Trae\skills` and `%APPDATA%\Trae CN\skills` appear as read-only sources on the Skills page. Import them manually before syncing to the new edition; source files are retained.

## 🛠️ Tech Stack

- **Frontend**: React 18 · TypeScript · Vite · TailwindCSS · TanStack Query
- **Backend**: Tauri 2 · Rust · SQLite (rusqlite)

## 📄 License

MIT License

---

<div align="center">
  <p>Made with ❤️ for AI Developers</p>
</div>
