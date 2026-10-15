# AI Toolkit

<div align="center">

[![Version](https://img.shields.io/badge/version-1.8.0-blue.svg)](https://github.com)
[![Platform](https://img.shields.io/badge/platform-macOS%2012%2B-lightgrey.svg)](https://github.com)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%202-orange.svg)](https://tauri.app/)

[中文](README.md) | [English](README_EN.md)

</div>

## 📖 简介

AI Toolkit是一款**通用的 AI 编程工具管理工具**，支持统一管理 MCP 服务器配置、Skills 技能同步，以及平台积分与本地模型代理。

## ✨ 核心特性

### 🎯 MCP 服务器管理
- 支持 **22 种** 主流 AI 编程工具：Qwen Code, Claude Code, Codex, Gemini CLI, OpenCode, TRAE, TRAE CN, Qoder, Qoder CLI, CodeBuddy, Hermes Agent, Mimo Code, WorkBuddy, ZCode, Cursor, Windsurf, Cline, Roo Code, Kilo Code, Continue, Goose, Kiro
- 在单一界面中添加、编辑、删除 MCP 服务器
- 自动检测系统中已安装的 AI 工具，新工具发现时弹窗提示
- 切换开关**实时同步**到对应工具的配置文件
- **JSON 粘贴模式**：直接从 MCP 介绍页面复制 JSON 配置，粘贴即可识别
- **批量导入预览**：一次粘贴多个 MCP Server 配置，勾选后批量导入
- **连接测试**：内置测试连接功能，确保服务器配置有效后再保存

### 🧰 Skills 技能管理
- **Skills 面板**：独立的 Skills 管理界面，集中管理所有已安装的技能
- **安装位置切换**：支持在官方 `~/.agents/skills/` 与 `~/.ai-toolkit/skills/` 之间切换中心 Skills 目录
- **批量同步**：支持在弹窗中点击选择多个 Skills，同步到多个目标工具
- **Git 安装**：支持从 GitHub/GitLab 仓库安装 Skills，自动解析仓库结构
- **在线搜索**：从 skills.sh 搜索热门 Skills
- **精选推荐**：浏览精选 Skills 列表，包含安装量和星标信息
- **一键更新**：自动检测有更新的 Skills，支持快速更新
- **健康检查与删除预览**：检查 Skills 文档、路径、同步目标状态，删除前预览影响范围
- **同步进度与冲突策略**：批量同步展示逐项进度，安装/重命名同名技能时支持重命名、跳过或覆盖

### 🪙 积分与模型代理
- **多平台账户**：连接 WorkBuddy、TRAE CN、Qoder 账户，支持官方授权入口与已有 Token/凭据 JSON 导入；凭据保存到系统凭据库
- **积分展示与领取**：分别显示平台原生额度、更新时间、额度包与领取资格；一键检查并领取可用积分，优惠券单独展示
- **可选自动领取**：默认关闭，开启后仅在应用运行期间按北京时间每天 10:30 检查并领取，当天 10:30 后启动可补做错过的检查
- **本地模型代理**：手动启动兼容 OpenAI Chat Completions 和 Anthropic Messages 的服务，默认地址 `http://127.0.0.1:17860/v1`；支持复制独立代理 API Key，关闭窗口后继续运行，从托盘退出应用后停止
- **真实模型目录**：带平台前缀的 ID 指定账户目录模型，上游返回不同型号时拒绝响应；跨平台别名仅在已验证模型间路由
- **通用客户端配置**：在代理页复制 Base URL、API Key 与模型 ID，按客户端支持的协议添加自定义提供商

平台接口及账户权限决定余额、领取与推理的实际可用性，未支持的能力会显示状态。当前实现需使用真实账户完成平台联调验证，不能把本地检查或测试结果视为三平台 GLM 已全部可用。

### 🛡️ 增强中心
- **MCP 与工具配置文件快照**：备份 MCP Server 数据和各工具 MCP 配置文件，支持一键恢复
- **健康检查**：检测 MCP 命令、环境变量、配置文件和同步状态
- **配置压缩包导入导出**：以 `.zip` 导出 MCP、Skills 文件和设置，避免大文件内容塞入页面
- **任务日志**：记录本地扫描、Skills 同步和导入等操作结果
- **Agent 启动预设**：保存工具、工作目录和 MCP 组合，一键按预设启动

### 🔧 开发者友好
- 点击工具名称可快速打开对应的配置文件
- 可视化界面，告别手动编辑 JSON/TOML 文件
- 支持多个配置文件路径自动识别
- **原子写入**：临时文件 + 重命名机制，防止配置损坏
- **代码质量检查**：新增 `pnpm lint` / `pnpm check`，使用 Oxlint 和 TypeScript 统一检查前端代码

### 🚀 Agent 快速启动
- **一键启动**：直接从终端启动 AI 工具

### 🔎 本地 Agent 扫描
- 只扫描并展示本地已安装的 CLI、桌面应用和 IDE 扩展，不提供 Agent 下载、安装、更新或卸载功能
- 启动时自动扫描，也可从侧栏、Skills 或 MCP 页面手动刷新；所有模块共用同一份安装状态
- 安装证据来自可执行文件、应用包或 IDE 扩展安装清单，残留的配置与 Skills 目录不会被误判为已安装
- Codex CLI、桌面版和 IDE 扩展共用一个 Codex 入口；WorkBuddy 国内版与国际版共用一个 WorkBuddy 入口
- 新增 Cline、Roo Code、Kilo Code、Continue、Goose、Cursor、Windsurf 和 Kiro 的 MCP 与 Skills 适配
- 支持 Kimi Code、MiniMax Code 的桌面应用与 CLI 识别、官方图标、MCP 和 Skills 同步
- 路径与官方来源见 [Agent 适配说明](docs/agent-adapters.md)

### 🔄 应用更新与分享
- **启动检查更新**：首次打开自动检测新版本，有更新时弹窗提示，由用户决定是否安装
- **一键分享**：设置页可复制官方站点地址，方便分享给其他用户

### 📌 托盘常驻
- 关闭主窗口后应用留在系统托盘，已开启的代理和后台任务继续运行
- 点击托盘菜单的“打开主窗口”可恢复界面；再次启动应用会打开已有窗口
- 点击“退出应用（停止代理）”会真正退出并停止代理；macOS 的 `Cmd+Q` 同样会退出

## 📸 界面预览

### 主界面
![主界面](assets/screenshots/main-panel.png)

## 🖥️ 系统支持

| 系统 | 状态 | 说明 |
|------|------|------|
| **macOS 12+** | ✅ 已支持 | 完整功能支持 |
| **Linux** | 🚧 开发中 | 基础功能可用 |
| **Windows 10+** | ✅ 已支持 | 支持 MCP、Skills 和工具启动 |

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
| TRAE（国际版） | `~/Library/Application Support/Trae/User/mcp.json` |
| TRAE CN（国内版） | `~/Library/Application Support/Trae CN/User/mcp.json` |
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

WorkBuddy 国际版与国内版共享应用名及 `~/.workbuddy` 配置、MCP 和 Skills 目录，不能同时安装；安装另一版本会替换当前应用，但不会删除共享数据。Windows 对应路径为 `%USERPROFILE%\.workbuddy\.mcp.json` 和 `%USERPROFILE%\.workbuddy\skills\`。

TRAE 按国际版与国内版展示两个管理入口。新版同地区的 Agent 与 IDE 共享本地 MCP 配置和 Skills 目录（国际版 `~/.trae/skills`，国内版 `~/.trae-cn/skills`），不再分别同步到 Work。旧 `TRAE Work`、`TRAE Work CN` 与 `TRAE SOLO CN` 安装和配置保留检测兼容，可预览并迁移到新版；旧配置 ID 与已同步 Skills 记录兼容读取。本次适配管理本地配置，不管理云端 Agent。

在 MCP 页点击“迁移旧 TRAE”可选择旧配置；同名异配置需改名导入，未选择的配置保留。迁移前自动备份，完成后可撤销；若之后有新修改，自动撤销会停止以保护新工作。Windows 旧 `%APPDATA%\Trae\skills`、`%APPDATA%\Trae CN\skills` 中的技能会在 Skills 页作为旧来源展示，可手动导入后同步到新版，旧文件不会自动搬迁或删除。

## 🛠️ 技术栈

- **前端**: React 18 · TypeScript · Vite · TailwindCSS · TanStack Query
- **后端**: Tauri 2 · Rust · SQLite (rusqlite)

## 📄 许可证

MIT License

---

<div align="center">
  <p>Made with ❤️ for AI Developers</p>
</div>
