# Changelog

## v1.2.4 (2026-04-17)

### 修复

- **Skills 工具检测**：修复 Trae、Trae CN、TRAE SOLO CN、Qoder、Qoder CLI 在 Skills 模块中无法被检测到的的问题
- **MCP 模块工具检测**：修复 GUI 应用（Trae、Trae CN、TRAE SOLO CN、Qoder）通过 /Applications 检测不准确的问题
- **Skills 目录路径**：修正 Qoder CLI 的 Skills 路径从 `.qodercli/skills` 到 `.qoder/skills`，TRAE SOLO CN 从 `.trae-solo-cn/skills` 到 `.trae-cn/skills`
- **Skills 模块性能**：优化 `get_managed_skills` 命令执行速度，预检测已安装工具避免重复扫描
- **MCP 模块性能**：优化 `get_tool_infos` 首次加载速度，使用并行获取版本信息
- **更新重启**：修复检查更新后下载完成但应用不自动重启的问题

### 改进

- **自动检测**：工具列表为空时自动触发首次检测，无需手动刷新
- **Skills 安装流程**：优化 Git 仓库单 Skills 自动安装流程，多 Skills 选择后触发安装

---

## v1.2.0 (2026-04-16)

### 新增功能

#### Agent 快速启动
- **一键启动**：支持直接从应用内启动 AI 工具
- **多终端支持**：Terminal, iTerm2, Warp, Hyper, Kitty, Alacritty, Fig, Kaku
- **Node.js 环境自动检测**：支持 nvm, fnm, volta, nvmd 等版本管理工具
- **Windows 终端支持**：Windows Terminal, PowerShell, CMD, Git Bash

#### 工具管理
- **安装向导**：显示各工具的多种安装方式 (Homebrew, npm, curl 脚本)
- **版本检测**：自动检测已安装工具的版本
- **一键更新**：快速更新已安装的工具

#### Qoder CLI 支持
- 新增 Qoder CLI 工具支持
- 配置文件路径：`~/.qodercli/settings.json`

### 改进
- **移除 OpenClaw**：移除已停用的 OpenClaw 支持
- **Qoder 路径更新**：Qoder 配置路径更新为 `~/Library/Application Support/Qoder/SharedClientCache/mcp.json`
- **工具数量**：从 10 种增加到 11 种

### 修复
- 若干 bug 修复和稳定性提升

---

## v1.1.0 (2026-04-14)

### 新增功能

#### Skills 管理模块
- **Skills 列表管理**：新增独立的 Skills 管理面板，支持查看所有已安装的 Skills
- **批量同步**：支持选择多个 Skills 同步到多个目标工具，提高操作效率
- **Git 仓库安装**：支持从 GitHub/GitLab 仓库安装 Skills，自动解析仓库结构
- **本地导入**：支持从本地目录导入 Skills 到集中仓库
- **在线搜索**：支持从 skills.sh 在线搜索热门 Skills
- **精选推荐**：展示精选 Skills 列表，包含安装量和星标信息
- **更新检测**：自动检测有更新的 Skills（支持 Git 仓库更新）
- **一键同步**：单个技能可快速同步到选定的 AI 工具
- **选择性安装**：支持多 Skills 仓库的选择性子目录安装

#### 工具检测与同步
- **多工具支持**：支持 Qwen Code, Claude Code, Codex, Gemini CLI, OpenCode, Trae, Trae CN, TRAE SOLO CN, Qoder, CodeBuddy
- **自动发现**：启动时自动检测系统中已安装的 AI 工具
- **实时同步**：MCP 服务器配置切换实时同步到对应工具配置文件
- **冲突检测**：检测同名 Skills 在不同工具中的安装情况

### 改进
- **项目重命名**：项目更名为 AI Tool Manager
- **数据库路径**：`~/.mcp-manager/` → `~/.ai-tool-manager/`
- **弹窗优化**：Skills 相关弹窗宽度优化，提升用户体验

### 修复
- 若干 bug 修复和稳定性提升

---

## v1.0.0 (2026-04-09)

### 首发版本
- MCP 服务器统一管理
- 支持 8+ 主流 AI 编程工具
- JSON 粘贴模式与智能解析
- 内置连接测试功能
- 配置文件原子写入保护
