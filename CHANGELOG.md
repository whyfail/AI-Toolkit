# Changelog

## v1.7.1 (2026-08-12)

### 修复

- **本地 Skill 文件被清空**：修复多个工具共享 Skills 目录时，并发同步可能将中央副本复制到自身并截断为 0 字节的问题。同步操作现在按真实目标路径串行执行，并在覆盖前阻止源目录与目标目录指向同一实体。
- **本地 Skill 安装完整性**：本地导入现在要求 `SKILL.md` 非空，并校验源目录与中央目录内容哈希；复制或数据库写入失败时自动清理不完整副本。
- **本地 Skill 元数据持久化**：本地安装成功后写入数据库，技能列表正确恢复来源和 description 信息。

## v1.7.0 (2026-07-17)

### 新功能

- **Apple Design 视觉重构**:全站样式按 Apple HIG 与 WWDC *Designing Fluid Interfaces* 重做。引入 Motion 弹簧动效、液态玻璃材质、Pressable 按压反馈、Modal spring 入场、AnimatePresence 面板切换、layoutId 滑动指示器,以及 prefers-reduced-motion/transparency/contrast 全套 a11y 兜底。
- **暗色模式**:设置页新增 light/dark/system 三段切换,跟随系统外观,实时生效。
- **Settings 主题切换器**:暗色 / 跟随系统,持久化到 localStorage,所有 glass-* 材质已在 .dark 下补齐。
- **Skill 详情 Hero**:从 SKILL.md frontmatter 解析 name + description,支持 YAML 多行块标量(`|` / `>`),并在 Hero 区单独展示,正文区自动剥离避免重复。
- **ProgressBar / Copy 反馈回弹**:复制按钮按 `AnimatePresence` 在"复制官网地址"和"已复制 ✓"之间切换,持续 1.5s。

### 改进

- **后端 SKILL.md 解析**:Rust 端 `parse_skill_md_with_reason` 重写,支持 YAML literal/folded block(`description: | / >`)、缩进规范化、空行/注释跳过,标准化与 relaxed 模式统一解析。
- **后端 description 持久化**:`install_git` / `install_git_selection` / `install_local_selection` 三条安装路径都把 SKILL.md 的 description 写入数据库,UI 不再永远显示空 description。
- **Claude plugin skill 验证**:`is_skill_dir` 要求 `.claude/skills/*` 必须包含 `plugin.json` / `.claude-plugin/plugin.json` 等 manifest 文件,空目录不再被误判为合法 skill。
- **Tauri 原生对话框**:全站移除 `window.alert / confirm / prompt`(Tauri webview 静默失败),改用 `@tauri-apps/plugin-dialog.ask` 与 toast 反馈;resolveNameConflict 兜底为"自动重命名 + toast.message",名字冲突不再静默跳过。
- **Modal portal 渲染**:统一 `<Modal>` 组件改用 React Portal 渲染到 `document.body`,脱离 `.glass-content` 的 `mask-image` 裁剪上下文,避免被遮挡。
- **侧边栏 nav 切换**:使用 `layoutId="sidebar-pill"` 实现 active 指示器在 6 个 nav 之间平滑滑动,不再硬切。
- **面板切换**:`AnimatePresence mode="wait"` + spring,切换 Tools / Skills / MCP / Enhancements / Settings 时整体跨淡。
- **Segmented controls**:Skills 筛选、AddSkillModal 三段 tab、Settings 主题切换、Enhancements 子面板切换,全部使用 Motion `layoutId` 滑动指示器。
- **按钮按压反馈**:所有 Pressable `whileTap scale(0.97)` 弹簧,icon-button `scale(0.92)`,hover 仅做 brightness 微调(不再 hover 顶起)。
- **Toast 自定义**:时长 3500ms,rounded-modal,半透明背景。
- **Scroll edge effects**:scrollbar 缩窄到 6px,hover 才显形;`.glass-content` 用 mask-image 让滚动内容在边缘渐隐。
- **Focus 焦点环**:Apple 风格 2px accent outline,键盘聚焦才显示。

### 修复

- **Git 安装空白 skill**:`git clone --filter=blob:none`(partial clone) 在网络抖动时会让 `std::fs::copy` 静默产生 0 字节目标文件,导致 SKILL.md 是空的。改用普通 `--depth=1`(无 filter),保证文件完整性;并在 copy 后做"中央目录空"校验,失败时显式报错而不是装出空白 skill。
- **Modal 被遮挡**:`.glass-content` 的滚动 mask-image 会裁剪子元素,导致 Skill 详情 Modal 显示不全。Modal 改用 React Portal 渲染到 body,脱离父级容器。
- **Skill 详情 modal 视觉粗糙**:Hero 区拆为大 icon + 标题 + 副描述 + 浮动 chip(metadata grid);SKILL.md 区域加 sticky 头部 + 完整 prose 样式;metadata 拆为独立 `<MetadataCell>` 卡片;健康检查 banner 分隔列表样式。

## v1.6.1 (2026-06-11)

### 新功能

- **Mimo Code 支持**：新增 Mimo Code 工具检测、MCP 配置导入/同步、Skills 同步和快速启动支持。
- **TRAE 产品线拆分**：按官网文档补齐 TRAE IDE 国际版，并将 TRAE IDE、TRAE IDE CN、TRAE Work、TRAE Work CN 拆分为独立工具展示和配置目标。

### 改进

- **TRAE 命名与文档入口**：更新 TRAE 国际版文档入口到官方 MCP 文档，TRAE Work 国际版使用 `trae.ai/work`，国内版使用 `trae.cn/work`。
- **TRAE Work CN 兼容迁移**：保留旧 `TRAE SOLO CN` 配置路径兼容，同时避免国内版误用国际版路径。

### 修复

- **Skills 批量同步弹窗**：修复 Skill 数量过多时内容超出窗口的问题，弹窗主体现在可滚动且底部操作区保持可见。

## v1.6.0 (2026-06-09)

### 新功能

- **Hermes Agent 支持**：新增 Hermes Agent 工具检测、MCP YAML 配置同步/导入、Skills 同步和快速启动支持。

## v1.5.6 (2026-06-08)

### 新功能

- **Skills 安装位置切换**：设置页新增 Skills 安装位置开关，支持官方 `~/.agents/skills/` 与 `~/.ai-toolkit/skills/`
- **工具 CLI / 桌面端分离**：工具管理支持分别检测、展示和启动 CLI 与桌面端，覆盖 OpenCode、Claude Code、Codex 等工具
- **配置压缩包导入导出**：增强中心改为导出 `.zip` 配置包，包含 MCP、设置和 Skills 文件，避免大文件写入页面导致卡顿
- **桌面端版本展示**：工具卡片新增桌面端版本展示，并区分 CLI 版本与桌面端版本

### 改进

- **Skills 管理体验**：移除导入入口，批量同步按钮常驻展示，批量同步弹窗支持点击 Skill 选择同步
- **工具管理展示控制**：工具管理顶部新增设置入口，可控制工具列表显示与隐藏
- **增强中心文案**：明确 MCP 批量导入、MCP 与工具配置文件快照的具体作用范围
- **OpenCode 检测兼容**：增强 macOS 与 Windows 桌面端检测，避免仅安装桌面端时显示未安装

### 修复

- **桌面端启动失败**：修复只安装 OpenCode 桌面端时点击启动仍提示未检测到 CLI 可执行文件的问题
- **Skills 统计一致性**：增强中心的 Skills 数量改为复用 Skills 管理页扫描逻辑，避免目录中存在 Skill 但显示 0
- **增强中心按钮换行**：修复增强中心顶部按钮超出时出现横向滚动条的问题

## v1.5.5 (2026-06-08)

### 新功能

- **增强中心**：新增配置备份回滚、MCP 健康检查、配置导入导出、任务日志、安全扫描、冲突检测和 Agent 启动预设
- **MCP 批量导入**：添加 MCP 时支持一次解析多个 server，并可勾选后批量导入到已安装工具
- **Skills 健康检查**：新增 Skills 文档、中央仓库路径、同步目标和来源状态检查
- **Skills 删除预览**：删除前展示中央仓库路径、受影响工具路径、链接/目录类型和风险提示
- **Skills 批量同步进度**：批量同步时展示每个 Skill 到每个工具的等待、同步中、成功和失败状态

### 改进

- **Skills 筛选增强**：支持按来源、同步状态、健康状态和目标工具筛选技能
- **Skills 详情增强**：详情弹窗展示来源、中央路径、同步目标、最近同步时间，并支持打开技能目录
- **冲突处理策略**：添加和重命名同名 Skill 时支持自动重命名、跳过；Git 安装支持覆盖已有技能
- **操作日志**：记录工具安装/更新/扫描/卸载、Skills 同步/取消同步/更新/删除、批量同步等操作结果
- **Agent 启动体验**：启动预设支持指定工作目录，并在启动前应用对应 MCP 组合

### 修复

- **Skills 导入入口**：修复首次点击导入时可能因为异步 state 未更新而不打开导入弹窗的问题
- **Skills 统计文案**：修正“已同步到”统计语义，区分可同步工具数量和实际同步目标数量

## v1.5.4 (2026-04-30)

### 修复

- **Skills 取消同步**：修复 Qwen Code 等工具中目录软链接删除失败但前端仍提示成功的问题，删除失败现在会正确返回错误
- **Skills 删除**：修复删除技能时可能漏删残留工具目录、重复处理共享目录、吞掉删除失败以及数据库记录残留的问题

### 改进

- **代码规范**：接入 Oxlint，新增 `pnpm lint` 和 `pnpm check`，前端代码质量检查达到 0 warning / 0 error
- **前端可访问性与稳定性**：清理现有 lint warning，补充表单 label 关联、稳定列表 key、语义化弹窗遮罩和批量同步并发处理

## v1.5.3 (2026-04-30)

### 修复

- **nvmd CLI 检测回归**：修复 `~/.nvmd/bin/*` shim 被误判为残留文件，导致生产环境中 Claude Code、Codex、OpenCode、Qwen Code、CodeBuddy 等本地已安装 CLI 显示为未安装的问题
- **残留 shim 判断**：nvmd shim 现在会先执行 `--version` 验证，只有不可执行时才继续按残留 shim 处理

## v1.5.2 (2026-04-30)

### 修复

- **生产环境 CLI 检测**：修复 macOS 生产包从 Finder/Dock 启动时无法继承终端 PATH，导致本地已安装 CLI 工具扫描不出来的问题
- **工具路径兜底搜索**：增强 macOS/Linux/Windows 下的 CLI 路径查找，覆盖 nvm、mise、asdf、bun、pnpm、npm、cargo、scoop、WindowsApps 等常见安装位置

## v1.5.1 (2026-04-30)

### 文档

- **README 界面预览**：更新 README 主界面截图，展示最新的玻璃拟态工具管理界面
- **官方分享地址**：设置页分享入口改为复制官方站点地址，并同步更新 README 说明

## v1.5.0 (2026-04-30)

### 新功能

- **启动自动检查更新**：应用首次打开时自动检查 GitHub Releases，新版本可用时弹出更新窗口，由用户决定是否下载安装
- **工具使用文档入口**：工具管理卡片新增使用文档跳转按钮，覆盖 Qwen Code、Claude Code、Codex、Gemini CLI、OpenCode、Trae、Qoder、CodeBuddy 等工具
- **分享入口**：设置页新增分享按钮，一键复制 GitHub 仓库地址，方便推荐给其他用户

### 改进

- **全局 Glassmorphism 视觉升级**：工具管理、MCP 服务器、Skills 管理、设置、关于、弹窗和侧边栏统一升级为玻璃拟态风格
- **浅色主题固定**：移除主题切换功能，应用固定使用浅色主题，并将侧边栏品牌图标替换为软件 Logo
- **并发工具更新状态**：工具管理支持多个工具同时更新，每个工具卡片独立展示更新中状态

### 修复

- **Windows 工具更新**：修复 Windows 下 npm 工具被误判为 Winget 导致更新失败的问题，优先使用工具自身更新命令
- **Windows 错误输出清理**：过滤 Winget/npm 进度条和 ANSI 控制字符，避免错误 toast 被进度条内容刷屏

## v1.4.0 (2026-04-28)

### 新功能

- **工具卸载功能**：新增工具卸载功能，支持通过 brew/npm/winget/scoop 卸载已安装的工具
  - 工具卡片右上角新增卸载按钮（仅对已安装且检测到安装方式的工具显示）
  - 卸载前弹出确认对话框，卸载过程中显示 loading 遮罩
  - 卸载后仅更新当前工具状态，不影响其他工具

### 修复

- **nvmd shim 检测**：修复通过 nvmd (nvm-desktop) 安装的工具卸载后仍显示为已安装的问题
  - `which_binary()` 新增 `is_nvmd_shim()` 检测，识别 nvmd 的符号链接和硬拷贝 shim
  - `is_agent_installed()` / `is_tool_installed()` 移除数据目录/配置文件误判检测，仅依赖 binary 和 GUI 应用检测

### 改进

- **状态更新优化**：工具安装、更新、卸载操作改为局部状态更新，仅刷新当前操作的工具卡片，不再触发全量刷新，避免其他工具卡片闪烁

## v1.3.2 (2026-04-27)

### 新功能

- **首次加载自动检测安装方式**：工具管理页首次加载时自动检测已安装工具的安装方式（Homebrew/npm/curl 等），无需手动点击扫描
- **首次加载自动后台扫描版本号**：工具列表渲染后自动后台扫描已安装工具的版本号和最新版本号，完成后自动更新展示，无需手动操作

### 修复

- **Windows 平台工具检测**：修复 Windows 下 Trae CN、Trae、TRAE SOLO CN、Qoder 等 GUI 应用首次安装时无法被检测到的问题
  - `is_agent_installed()` 增加 Windows GUI 应用 exe 搜索检测
  - 增加应用数据目录存在性检测，首次安装未配置 MCP 时也能识别
  - 补全 `is_app_installed_windows()` 的搜索子目录，覆盖更多安装路径

### 改进

- **工具管理首屏体验**：安装方式和版本号不再需要手动扫描，列表加载后自动获取并展示
- **设置页精简**：支持的应用列表只显示应用名称，不再显示配置文件路径
- **CodeBuddy 重命名**：显示名称改为 CodeBuddy CN CLI

## v1.3.1 (2026-04-24)

### 新功能

- **终端偏好设置**：新增默认启动终端配置，支持在设置页选择 macOS 与 Windows 的首选终端

### 改进

- **macOS 启动体验**：优化 Terminal、iTerm、Warp、Ghostty 的工具启动流程，改进工作目录、命令解析与脚本执行稳定性
- **Windows 兼容性**：完善配置路径展示、工具检测、Agent 启动与终端回退逻辑，提升 Windows 下的可用性
- **工具管理性能**：首次进入工具管理页时优先返回基础安装信息，避免版本与网络检测阻塞首屏
- **后台检测更新**：首次启动时工具检测改为后台完成后推送更新，减少首页长时间停留在“正在扫描中”的情况

## v1.3.0 (2026-04-23)

### 新功能

- **数据迁移**：从旧版 `ai-tool-manager` 升级时自动迁移数据目录 `~/.ai-tool-manager/` → `~/.ai-toolkit/`，包括数据库、检测状态和 Skills 仓库，迁移后旧目录备份为 `.ai-tool-manager.bak/`

### 改进

- **项目重命名完成**：修复 Cargo.toml `[lib] name`、`main.rs` 中残留的 `ai_tool_manager` 引用，统一更新为 `ai_toolkit`
- **启动功能简化**：移除终端选择功能，统一使用系统默认终端（macOS: Terminal.app, Windows: Windows Terminal），避免辅助功能权限问题
- **删除确认优化**：MCP 服务器和 Skills 删除弹窗增加 loading 状态，防止重复点击；MCP 删除确认弹窗显示服务器名称而非 ID
- **项目描述更新**：package.json 和 Cargo.toml 的 description 更新为 "Universal MCP Server & Skills Manager for AI CLI Tools"
- **Release 流程更新**：release.yml 中的发布名称和下载文件名统一为 AI Toolkit

---

## v1.2.9 (2026-04-21)

### 改进

- **Git 技能添加流程优化**：重新设计 Git 仓库添加技能的交互流程
  - 初始仅显示 URL 输入框和"预览仓库"按钮
  - 预览后单个技能：锁定 URL，显示技能名称输入框和同步工具列表
  - 预览后多个技能：弹出选择窗口，确认后每个技能显示独立的名称输入框
  - 支持自定义技能名称作为 skills 文件夹名称
- **本地文件夹验证**：添加本地技能时自动验证文件夹是否为合规的技能目录（需包含 SKILL.md 或 skill.json），不合规则禁用添加按钮并提示原因

---

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
- **数据库路径**：`~/.mcp-manager/` → `~/.ai-toolkit/`
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
