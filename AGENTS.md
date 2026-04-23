# AI Toolkit - AGENTS.md

## 项目概述

AI Toolkit是一个基于 Tauri 2 的跨平台桌面应用，用于统一管理多个 AI CLI 工具的 MCP (Model Context Protocol) 服务器配置和 Skills。支持 11 种 AI 工具，提供可视化界面进行 CRUD 操作，并自动同步配置和 Skills 到各工具。

## 技术栈

- **前端**: React 18 + TypeScript + TailwindCSS + TanStack Query
- **后端**: Rust + Tauri 2 + SQLite (rusqlite) + serde + toml
- **构建**: Vite + pnpm
- **包管理**: pnpm

## 项目结构

```
ai-toolkit/
├── src/                          # 前端源码 (React + TypeScript)
│   ├── main.tsx                  # React 入口
│   ├── App.tsx                   # 主应用：侧边栏导航 + 选项卡 (MCP/设置/关于)
│   ├── index.css                 # 全局样式 + CSS 变量主题
│   ├── types/index.ts            # TypeScript 类型定义 (McpServer, McpServerSpec 等)
│   ├── lib/api/index.ts          # Tauri invoke API 封装 (mcpApi, appApi)
│   ├── hooks/useMcp.ts           # TanStack Query hooks (CRUD + 导入)
│   └── components/mcp/
│       ├── UnifiedMcpPanel.tsx   # 主面板：服务器列表 + 搜索 + 扫描 + Agent 切换
│       ├── McpFormModal.tsx      # 添加/编辑服务器的表单弹窗 (JSON 导入 + 手动填写)
│       └── NewAgentModal.tsx     # 新工具发现弹窗
│   └── components/tool-manager/
│       └── ToolManagerPanel.tsx  # 工具管理面板：安装/更新/启动 Agent 工具
│
├── src-tauri/                    # Rust 后端源码
│   ├── Cargo.toml                # Rust 依赖配置
│   ├── tauri.conf.json           # Tauri 应用配置
│   ├── build.rs                  # Tauri 构建脚本
│   └── src/
│       ├── main.rs               # 入口 (调用 lib.rs)
│       ├── lib.rs                # Tauri 应用初始化、插件注册、命令注册
│       ├── app_state.rs          # 全局状态 AppState (持有 Database)
│       ├── error.rs              # AppError 枚举 (Database/Serialization/IO/Parse/NotFound)
│       ├── agents.rs             # Agent 工具检测逻辑 + 路径解析
│       ├── mcp/mod.rs            # AppType 枚举定义 (10 种 AI 工具)
│       ├── import/mod.rs         # MCP 配置导入/解析 (支持 JSON + TOML + 多种格式)
│       ├── services/
│       │   ├── mod.rs            # 服务模块导出
│       │   ├── mcp_service.rs    # MCP 业务逻辑层 (CRUD + 同步触发)
│       │   ├── sync.rs           # 配置文件同步生成 (核心: 各应用格式适配)
│       │   └── tool_manager.rs   # 工具安装/更新服务
│       ├── commands/
│       │   ├── mod.rs            # 命令模块导出
│       │   ├── mcp.rs            # MCP Tauri 命令 (get/upsert/delete/toggle/import/test)
│       │   ├── app.rs            # 应用配置 Tauri 命令
│       │   ├── agents.rs         # Agent 检测/启动 Tauri 命令 (含 launch_agent, get_terminals)
│       │   └── tool_manager.rs   # 工具安装/更新 Tauri 命令
│       └── database/
│           ├── mod.rs            # Database 结构体、Schema 初始化
│           └── dao/mcp.rs        # 数据模型定义 + SQLite CRUD 操作
```

## 开发命令

```bash
# 安装依赖
pnpm install

# 前端开发
pnpm dev

# Tauri 完整开发 (前端 + 后端热重载)
pnpm tauri:dev

# 构建生产版本
pnpm tauri:build

# 仅 Rust 编译检查
cd src-tauri && cargo check
```

## 支持的 AI 工具及配置格式

| 工具 | 配置文件路径 | JSON 键名 | 格式 |
|------|-------------|----------|------|
| Qwen Code | `~/.qwen/settings.json` | `mcpServers` | JSON 对象 |
| Claude Code | `~/.claude.json` | `mcpServers` | JSON 对象 |
| Codex | `~/.codex/config.toml` | `mcp_servers` | TOML |
| Gemini CLI | `~/.gemini/settings.json` | `mcpServers` | JSON 对象/数组 |
| OpenCode | `~/.config/opencode/opencode.json` | `mcp` | JSON (严格 schema) |
| Trae | `~/Library/Application Support/Trae/User/mcp.json` | `mcpServers` | JSON 对象 |
| Trae CN | `~/Library/Application Support/Trae CN/User/mcp.json` | `mcpServers` | JSON 对象 |
| TRAE SOLO CN | `~/Library/Application Support/TRAE SOLO CN/User/mcp.json` | `mcpServers` | JSON 对象 |
| Qoder | `~/Library/Application Support/Qoder/SharedClientCache/mcp.json` | `mcpServers` | JSON 对象 |
| Qoder CLI | `~/.qodercli/settings.json` | `mcpServers` | JSON 对象 |
| CodeBuddy | `~/.codebuddy/mcp.json` | `mcpServers` | JSON 对象 |

## 核心架构

### 数据流

```
前端 UI (React)
  ↕ TanStack Query + Tauri invoke
后端 Commands (Rust)
  ↕ McpService
Database (SQLite) + Config File Sync
```

### 配置同步机制 (sync.rs)

每次执行 upsert/delete/toggle 操作后，`McpService` 自动调用 `sync_all_live_configs()`:

1. 遍历所有 MCP 服务器，按启用的应用类型分组
2. 对每个应用读取其现有配置文件，**保留非 MCP 字段**
3. 替换 MCP 相关键值，原子写入配置文件

**特殊格式处理**:
- **Codex**: 独立 `sync_codex_config()` 生成 TOML 格式
- **OpenCode**: 使用 `build_opencode_mcp_json()` 生成严格 schema 格式（`type` 必填、`command` 为 `string[]`、环境变量用 `environment`）
- **其他工具**: 通用 `build_mcp_json()` 生成 `command`/`args`/`env` 格式

### 配置导入机制 (import/mod.rs)

启动时自动扫描所有工具的配置文件，解析已有 MCP 配置并导入数据库。支持:
- `mcpServers` 对象格式
- `mcpServers` 数组格式 (Gemini CLI)
- `mcp` 键 (OpenCode/Trae)
- TOML `mcp_servers` (Codex)
- 顶层即为服务器对象的格式

### Agent 检测机制 (agents.rs)

- 启动时检测所有 10 种工具是否已安装（通过配置文件是否存在判断）
- 与上次检测结果对比，发现新工具时通过 Tauri 事件 `agents-detected` 通知前端
- 检测状态持久化到 `~/.ai-toolkit/detected.json`

## 数据存储

- **数据库**: `~/.ai-toolkit/ai-toolkit.db` (SQLite)
- **检测状态**: `~/.ai-toolkit/detected.json`

### 数据库表结构

`mcp_servers` 表:
- `id` (TEXT PK) - 服务器唯一标识
- `name` (TEXT) - 显示名称
- `server_config` (TEXT) - JSON 序列化的 McpServerSpec
- `description`, `homepage`, `docs` (TEXT) - 元信息
- `tags` (TEXT) - JSON 数组
- `enabled_*` (BOOLEAN x10) - 各应用启用状态
- `updated_at` (INTEGER) - 更新时间戳

## Tauri 命令 (前后端接口)

| 命令 | 功能 |
|------|------|
| `get_mcp_servers` | 获取所有 MCP 服务器 |
| `upsert_mcp_server` | 添加/更新服务器 (含自动同步) |
| `delete_mcp_server` | 删除服务器 (含自动同步) |
| `toggle_mcp_app` | 切换应用启用状态 (含自动同步) |
| `import_mcp_from_apps` | 从所有应用导入配置 |
| `import_mcp_from_app` | 从指定应用导入配置 |
| `test_mcp_connection` | 测试 MCP 连接 |
| `get_app_configs` | 获取应用配置信息 |
| `detect_agents` | 检测已安装的 Agent 工具 |
| `sync_agent_mcp` | 同步 Agent MCP 配置 |
| `open_config_file` | 打开配置文件 |
| `launch_agent` | 启动 Agent 工具 (支持多种终端) |
| `get_terminals` | 获取系统已安装的终端列表 |
| `get_tool_infos` | 获取所有工具的安装信息 |
| `get_tool_info` | 获取指定工具的安装信息 |
| `install_tool` | 安装工具 |
| `update_tool` | 更新工具 |
| `get_managed_skills` | 获取所有管理中的技能 |
| `get_tool_status` | 获取所有工具的安装状态和技能列表 |
| `get_onboarding_plan` | 生成导入计划（从已安装工具扫描技能） |
| `install_git` | 从 Git URL 安装技能（单一技能） |
| `list_git_skills` | 列出 Git 仓库中的所有技能候选 |
| `install_git_selection` | 安装 Git 仓库中指定的子路径技能 |
| `install_local_selection` | 从本地文件夹安装技能 |
| `validate_local_skill` | 验证本地文件夹是否为合规的技能目录 |
| `sync_skill_to_tool` | 将技能同步到指定工具 |
| `unsync_skill_from_tool` | 从指定工具取消同步 |
| `import_existing_skill` | 导入现有技能到 central repo |
| `delete_managed_skill` | 删除技能（central repo + 所有工具目录） |
| `update_skill` | 更新 Git 技能（重新拉取） |
| `rename_skill` | 重命名技能（central repo + 所有工具 + 数据库） |
| `get_skill_readme` | 读取技能的 SKILL.md 内容 |
| `search_skills_online` | 在线搜索技能（skills.sh API） |
| `get_featured_skills` | 获取热门技能列表 |

## 版本管理

发布新版本时，需要同步修改以下文件中的版本号：

| 文件 | 路径 | 键名/位置 |
|------|------|----------|
| package.json | 根目录 | `version` |
| tauri.conf.json | src-tauri/ | `version` |
| Cargo.toml | src-tauri/ | `version` |
| mcp.rs | src-tauri/src/commands/ | `clientInfo.version` |

**注意**: CHANGELOG.md 中的版本记录保留历史，不需同步修改。

---

## 开发注意事项

### 各工具配置格式差异

添加新工具支持时需要注意:
1. **OpenCode** 的 MCP 配置有严格 schema (`additionalProperties: false`)，必须使用专用的 `build_opencode_mcp_json()` 生成
2. **Codex** 使用 TOML 格式，需要独立的 `sync_codex_config()` 处理
3. **Gemini CLI** 的 `mcpServers` 可能是数组格式

### 路径处理

- 使用 `agents.rs` 中的 `resolve_path()` 统一处理 `~` 和 Windows 环境变量 (`%USERPROFILE%`, `%APPDATA%`)
- 跨平台路径拼接需要考虑 macOS (`~/Library/Application Support/`)、Linux (`~/.config/`)、Windows (`%APPDATA%`) 差异

### 配置同步安全

- 使用原子写入（先写 `.tmp` 临时文件，再 rename）防止配置损坏
- 同步时保留配置文件中的非 MCP 字段，避免覆盖用户的其他配置
