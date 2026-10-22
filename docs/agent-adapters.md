# 本地 Agent 适配

AI Toolkit 只展示本机已安装的 Agent。扫描读取可执行文件、macOS 应用包、Windows 应用程序和 IDE 扩展安装清单，不运行 Agent，也不查询最新版本或调用包管理器。配置和 Skills 目录存在本身不能证明 Agent 已安装。

Skills、MCP、设置页和同步选项共用扫描结果。IDE 扩展需要同时存在宿主编辑器和有效的 `extensions.json` 安装记录；已删除的扩展包及 `.obsolete` 标记的版本不计入。扫描刷新后，已经卸载的 Agent 会从界面移除。

## 共用配置的客户端

- Codex CLI、桌面版与 IDE 扩展统一为 `codex`。MCP 使用 `~/.codex/config.toml`，支持 `CODEX_HOME`；Skills 保留 `~/.codex/skills` 同步目标，并扫描共享的 `~/.agents/skills`。
- WorkBuddy 国内版与国际版统一为 `workbuddy`，共用 `~/.workbuddy/.mcp.json` 和 `~/.workbuddy/skills`。历史 `workbuddy-cn` ID 和启用状态继续兼容。
- 同地区的 TRAE IDE 与 Agent 使用同一个入口。国际版和国内版仍分别使用 `trae` 与 `trae-cn`，因为配置文件和 Skills 目录不同。旧 Work/SOLO 的兼容检测与显式迁移流程继续保留。
- Qoder 与 Qoder CLI 的 MCP 配置不同，继续分开；共享的 Skills 目录保持已有行为。
- Kimi Code 的桌面版与 `kimi` CLI 共用 `kimi-code`；MiniMax Code 的桌面版与 `mcode` CLI 共用 `minimax-code`。可直接凭桌面应用包识别，无需单独安装 CLI。
- DeepSeek Harness 的桌面版与 `dsh` CLI 共用 `deepseek-harness`；支持通过桌面应用启动，CLI 使用 `dsh web`。

Codex 的配置依据：[OpenAI 官方配置说明](https://developers.openai.com/codex/config-basic/)、[Skills 说明](https://developers.openai.com/codex/skills/)。

## 新增适配

| Agent | 全局 MCP 配置 | Skills 同步目录 | 安装证据 | 官方来源 |
| --- | --- | --- | --- | --- |
| Cline | `~/.cline/data/settings/cline_mcp_settings.json`；兼容编辑器旧 globalStorage 文件 | `~/.cline/skills` | `cline` 或已安装的 Cline 扩展 | [配置](https://docs.cline.bot/getting-started/config)、[Skills](https://docs.cline.bot/customization/skills) |
| Roo Code | `<IDE User>/globalStorage/rooveterinaryinc.roo-cline/settings/mcp_settings.json` | `~/.roo/skills` | 已安装的 Roo Code 扩展 | [MCP 源码](https://github.com/RooCodeInc/Roo-Code/blob/main/src/services/mcp/McpHub.ts)、[Skills](https://docs.roocode.com/features/skills) |
| Kilo Code | `~/.config/kilo/kilo.json` 或 `kilo.jsonc`；兼容旧扩展 `mcp_settings.json` | `~/.kilo/skills` | `kilo`、桌面应用或已安装的 Kilo 扩展 | [MCP](https://kilo.ai/docs/automate/mcp/using-in-cli)、[Skills](https://kilo.ai/docs/customize/skills) |
| Continue | `~/.continue/config.yaml` | `~/.continue/skills` | `cn` 或已安装的 Continue 扩展 | [配置路径](https://docs.continue.dev/cli/configuration)、[YAML 格式](https://docs.continue.dev/reference) |
| Goose | macOS `~/Library/Application Support/Block/goose/config.yaml`；兼容 `~/.config/goose/config.yaml`；支持 `GOOSE_PATH_ROOT` | MCP 配置旁的 `skills` 目录，兼容读取 `~/.agents/skills` | `goose` 或 Goose 桌面应用 | [配置路径源码](https://github.com/aaif-goose/goose/blob/main/crates/goose/src/config/paths.rs)、[MCP](https://goose.vcorp.ai/docs/getting-started/using-extensions/)、[Skills](https://goose.vcorp.ai/docs/guides/context-engineering/using-skills/) |
| Cursor | `~/.cursor/mcp.json` | `~/.cursor/skills` | `cursor-agent`、`cursor` 或 Cursor 应用 | [MCP](https://cursor.com/docs/mcp)、[Skills](https://cursor.com/docs/skills) |
| Windsurf | `~/.codeium/windsurf/mcp_config.json`；兼容 Devin 配置位置 | `~/.codeium/windsurf/skills` | `windsurf`、`devin` 或对应桌面应用 | [MCP](https://docs.windsurf.com/windsurf/cascade/mcp)、[Skills](https://docs.windsurf.com/windsurf/cascade/skills) |
| Kiro | `~/.kiro/settings/mcp.json` | `~/.kiro/skills` | `kiro-cli`、`kiro` 或 Kiro 应用 | [配置作用域与路径](https://kiro.dev/docs/cli/chat/configuration/) |
| Kimi Code | `~/.kimi-code/mcp.json`；支持 `KIMI_CODE_HOME` | `~/.kimi-code/skills`，跟随 `KIMI_CODE_HOME` | `kimi` 或 Kimi Code 桌面应用 | [MCP](https://github.com/MoonshotAI/kimi-code/blob/main/docs/en/customization/mcp.md)、[Skills](https://github.com/MoonshotAI/kimi-code/blob/main/docs/en/customization/skills.md) |
| MiniMax Code | `~/.minimax/mcp.json`；支持 `MINIMAX_DATA_DIR`、`MAVIS_DATA_DIR` | `<数据目录>/skills`，默认 `~/.minimax/skills` | `mcode` 或 MiniMax Code 桌面应用 | [MCP 源码](https://github.com/MiniMax-AI/minimax-code/blob/main/packages/local-runtime-v2/src/service/mcp/runtime/local-mcp.service.ts)、[Skills 作用域](https://github.com/MiniMax-AI/minimax-code/blob/main/packages/local-runtime/assets/skills/skill-creator/SKILL.md) |
| DeepSeek Harness | `~/.dsh/cordis.patch.yml`；支持 `DSH_HOME`，作为所有 profile 共用的机器配置层 | `<DSH_HOME>/skills`，默认 `~/.dsh/skills`；兼容读取 `~/.agents/skills` 或 `DSH_AGENTS_HOME/skills` | `dsh` 或 DeepSeek Harness 桌面应用 | [全局配置层](https://github.com/deepseek-ai/deepseek-harness/blob/master/apps/cli/src/profile-boot.ts)、[MCP](https://github.com/deepseek-ai/deepseek-harness/blob/master/packages/mcp/mcp-client/README.md)、[Skills](https://github.com/deepseek-ai/deepseek-harness/blob/master/packages/skill/skill-filesystem/src/index.ts) |

社区适配也可参考 [Vercel 开源 Skills 适配目录](https://github.com/vercel-labs/skills/blob/main/src/agents.ts)；配置格式以各 Agent 的官方说明及源码为准。

`<IDE User>` 根据平台定位：macOS 为 `~/Library/Application Support/<IDE>/User`，Windows 为 `%APPDATA%/<IDE>/User`，Linux 为 `$XDG_CONFIG_HOME/<IDE>/User` 或 `~/.config/<IDE>/User`。兼容 VS Code、Insiders、VSCodium、Cursor、Windsurf/Devin、TRAE 和 Kiro；优先读取实际安装该扩展的编辑器配置。

## 格式与数据保留

- Cline、Roo、Cursor、Windsurf、Kiro 使用 `mcpServers` JSON；Kilo 新格式使用 `mcp`、命令数组与 `environment`，旧扩展仍使用 `mcpServers`。
- JSONC 支持注释和尾随逗号，写回为有效 JSON，保留非 MCP 配置字段。
- Continue 使用 YAML 的 `mcpServers` 列表，保留 `uses` 引用及模型等设置。
- Goose 使用 YAML `extensions`，转换 `cmd`、`envs`、`uri`，保留内置扩展。MCP 与内置扩展同名时停止写入。Skills 同步到官方支持的专用配置目录，避免将共享中央目录作为可删除的同步副本。
- Codex 的 TOML 同步保留远程 URL、HTTP headers、工作目录及非 MCP 设置。
- Kimi Code 与 MiniMax Code 使用 `mcpServers` JSON；Kimi 的 SSE 条目转换为官方 `transport: "sse"`。两者均兼容读取 `~/.agents/skills`，同步写入各自专用目录，保留共享中央目录。
- DeepSeek Harness 在机器级 `cordis.patch.yml` 的根级 `insert` 中管理 `@deepseek-ai/dsh-mcp-client`，支持 stdio 与 Streamable HTTP；MCP ID 须为 1–32 位字母、数字、下划线或连字符。保留其他插件、嵌套作用域和非 MCP 配置中的 `!!js` 表达式，不修改 profile 文件。MCP 自身含动态表达式、格式损坏或使用旧 SSE 时会返回错误并保留原文件。
- MCP 同步仅面向扫描确认已安装的 Agent，不为未安装的 Agent 创建配置文件。无法解析的 JSON/JSONC/YAML 文件不会被覆盖。

Kimi Code、MiniMax Code 的路径核对时间为 2026-10-15，图标取自本机官方应用包；其余路径核对时间为 2026-10-14。Linux 仍沿用项目的开发中支持状态；Windows 路径已适配，需要在 Windows 主机上完成实际应用验证。

DeepSeek Harness 的路径与格式核对时间为 2026-10-22，依据官方源码与本机 `DeepSeek Harness.app` 0.2.1-rc.2；图标取自官方站点的蓝色鲸鱼 SVG，保留原始纯矢量路径，适配列表小尺寸与高 DPI 显示。
