# AI Toolkit

<div align="center">

[![Version](https://img.shields.io/badge/version-1.2.4-blue.svg)](https://github.com)
[![Platform](https://img.shields.io/badge/platform-macOS%2012%2B-lightgrey.svg)](https://github.com)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%202-orange.svg)](https://tauri.app/)

[中文](README.md) | [English](README_EN.md)

</div>

## 📖 简介

AI Toolkit是一款**通用的 AI 编程工具管理工具**，支持统一管理 MCP 服务器配置和 Skills 技能同步。告别繁琐的手动编辑，一个应用即可管理所有 AI CLI 工具的插件服务。

## ✨ 核心特性

### 🎯 MCP 服务器管理
- 支持 **11 种** 主流 AI 编程工具：Qwen Code, Claude Code, Codex, Gemini CLI, OpenCode, Trae, Trae CN, TRAE SOLO CN, Qoder, Qoder CLI, CodeBuddy
- 在单一界面中添加、编辑、删除 MCP 服务器
- 自动检测系统中已安装的 AI 工具，新工具发现时弹窗提示
- 切换开关**实时同步**到对应工具的配置文件
- **JSON 粘贴模式**：直接从 MCP 介绍页面复制 JSON 配置，粘贴即可识别
- **连接测试**：内置测试连接功能，确保服务器配置有效后再保存

### 🧰 Skills 技能管理
- **Skills 面板**：独立的 Skills 管理界面，集中管理所有已安装的技能
- **批量同步**：支持选择多个 Skills 同步到多个目标工具
- **Git 安装**：支持从 GitHub/GitLab 仓库安装 Skills，自动解析仓库结构
- **在线搜索**：从 skills.sh 搜索热门 Skills
- **精选推荐**：浏览精选 Skills 列表，包含安装量和星标信息
- **一键更新**：自动检测有更新的 Skills，支持快速更新

### 🔧 开发者友好
- 点击工具名称可快速打开对应的配置文件
- 可视化界面，告别手动编辑 JSON/TOML 文件
- 支持多个配置文件路径自动识别
- **原子写入**：临时文件 + 重命名机制，防止配置损坏

### 🚀 Agent 快速启动
- **一键启动**：直接从应用内启动 AI 工具
- **多终端支持**：Terminal, iTerm2, Warp, Hyper, Kitty, Alacritty, Fig, Kaku
- **Node.js 环境自动检测**：支持 nvm, fnm, volta, nvmd 等版本管理工具
- **Windows 终端支持**：Windows Terminal, PowerShell, CMD, Git Bash

### 📦 工具管理
- **安装向导**：显示各工具的多种安装方式 (Homebrew, npm, curl 脚本)
- **版本检测**：自动检测已安装工具的版本
- **一键更新**：快速更新已安装的工具

## 📸 界面预览

### 主界面
![主界面](assets/screenshots/main-panel.png)

## 🖥️ 系统支持

| 系统 | 状态 | 说明 |
|------|------|------|
| **macOS 12+** | ✅ 已支持 | 完整功能支持 |
| **Linux** | 🚧 开发中 | 基础功能可用 |
| **Windows 10+** | 🚧 开发中 | 路径适配进行中 |

## 🚀 快速开始

### macOS 安装

从 [Releases](https://github.com/whyfail/ai-toolkit/releases) 页面下载最新的 `AI Toolkit_x.x.x_aarch64.dmg` 安装包：

```bash
# 挂载 DMG
hdiutil attach AI\ Toolkit_*.dmg

# 拖动到 Applications 文件夹
cp -R /Volumes/AI\ Toolkit/AI\ Toolkit.app /Applications/
```

### ⚠️ macOS 安全提示（首次运行必看）

由于当前版本未进行 Apple 代码签名和公证，macOS 安全机制可能会拦截未签名应用，提示 **"无法验证开发者"** 或 **"文件已损坏"**。请按以下步骤放行：

**方法一（终端命令 - 最推荐）：**
1. 将应用拖入 `/Applications` 文件夹
2. 打开 **终端 (Terminal)**，执行以下命令：
   ```bash
   sudo xattr -cr "/Applications/AI Toolkit.app"
   ```
3. 输入开机密码并回车（密码输入时不显示），提示成功后即可双击打开

**方法二（右键打开）：**
1. 在 `访达 (Finder)` 中找到 `AI Toolkit.app`
2. **右键点击**（或按住 `Control` 键点击）应用图标
3. 在弹出的菜单中选择 **"打开"**
4. 在弹出的系统警告框中，再次点击 **"打开"** 即可

**方法三（系统设置放行）：**
1. 打开 **"系统设置"** -> **"隐私与安全性"**
2. 向下滚动到 **"安全性"** 区域
3. 找到提示 `"AI Toolkit" 已被阻止使用...`
4. 点击 **"仍要打开"**，输入密码确认即可

## 📁 支持的 AI 工具及配置路径

| 工具 | 配置文件路径 |
|------|-------------|
| Qwen Code | `~/.qwen/settings.json` |
| Claude Code | `~/.claude.json` |
| OpenAI Codex | `~/.codex/config.toml` |
| Google Gemini CLI | `~/.gemini/settings.json` |
| OpenCode | `~/.config/opencode/opencode.json` |
| Qoder | `~/Library/Application Support/Qoder/SharedClientCache/mcp.json` |
| Qoder CLI | `~/.qodercli/settings.json` |
| Trae | `~/Library/Application Support/Trae/User/mcp.json` |
| Trae CN | `~/Library/Application Support/Trae CN/User/mcp.json` |
| TRAE SOLO CN | `~/Library/Application Support/TRAE SOLO CN/User/mcp.json` |
| CodeBuddy | `~/.codebuddy/mcp.json` |

## 🛠️ 技术栈

- **前端**: React 18 · TypeScript · Vite · TailwindCSS · TanStack Query
- **后端**: Tauri 2 · Rust · SQLite (rusqlite)

## 📄 许可证

MIT License

---

<div align="center">
  <p>Made with ❤️ for AI Developers</p>
</div>
