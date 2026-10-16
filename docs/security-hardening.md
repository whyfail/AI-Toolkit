# 安全修复与验证记录

日期：2026-10-16。基于 2026-10-15 的安全排查报告和 `2.0.2` 代码实施，现纳入 **v2.1.0**。下表保留安全修复阶段的验证记录；长会话与思考历史兼容修复另见 [积分与模型代理](credits-proxy.md)。

## 已实施的修改

| 范围 | 修改与验证 |
| --- | --- |
| S1 技能目录边界 | Rust 统一校验技能名称和来源子路径，规范化后检查目录归属。安装、更新、本地导入、同步、删除、重命名、文档读取均接入校验；同步核对技能 ID、名称、数据库来源和前端路径。扫描不跟随越界目录链接或文档链接。保留中文、Unicode、空格和显式 `.` 整仓库来源。旧名称仅允许重命名中央目录内真实存在的单个目录项，越界记录不自动搬迁；界面提示异常来源，文件系统发现的技能首次编辑时补存元数据。 |
| S1 替换与恢复 | 内容先下载、复制、检查及计算哈希，再在同一文件系统内替换目标。替换失败恢复旧目录；链接只删除链接本身。安装成功但旧备份无法清理时保留备份并记录恢复位置。新增目录外哨兵、失败回滚、身份不匹配等测试。 |
| S2 WebView | Rust Tauri 升至 `2.12.4`，前端 API `2.12.4`、CLI `2.12.3`。主窗口在 Rust 中按原窗口配置创建，导航只允许精确应用来源和开发模式下的既有开发来源；拒绝 WebView 新窗口，HTTP/HTTPS 外链交由系统浏览器。Markdown 锚点仍在文档内跳转。来源相似域、异常协议和端口有回归测试。 |
| S3 OAuth | 保留原始回环 `/authorize` 回调形状，在授权参数中携带随机 state。授权码由当前 PKCE verifier 交换验证；只有绑定当前流程的 refresh-only / 错误回调可被接收。候选回调与完成登录分开，未绑定错误、旧流程和无效候选不会提前关闭监听器。校验完整请求头、Host、方法、长度及重复参数，并在完成或原有期限到达时停止。 |
| S4 Git 缓存 | 安装与列表使用同一用户私有缓存 `cache_dir()/ai-toolkit/git-v2`，不复用原公共临时缓存。校验目录所有者、链接类型和 origin；异常来源报错。缓存状态置于 checkout 外，使用随机 staging 更新；过期刷新失败保留已有 checkout。锁覆盖整个扫描/复制期间。测试覆盖本地仓库、含斜杠分支、TTL、刷新和失败保留。 |
| S5 MCP 数据库 | Unix 私有目录 `0700`，数据库和已有 journal/WAL/SHM `0600`；拒绝异常所有者、链接及硬链接。Windows 检查所有者，使用受保护 DACL 限定所有者和 SYSTEM。原数据库内容与 WAL 权限有隔离测试；不改变 MCP 配置格式。 |
| S6 资源边界 | Git 命令执行真实 deadline，同时持续读取两条输出管道并限制保留的诊断量。Unix 终止进程组，Windows 使用关闭即终止的 Job Object。GitHub 下载分块写入，目录扫描、复制和哈希受总量/深度/时间预算约束，哈希不再整文件分配内存。超时测试同时覆盖大量 stderr 和后代进程清理。 |
| S7 依赖补丁 | Vite `6.4.4`、PostCSS `8.5.30`，更新兼容范围内的 Babel、nanoid 等传递依赖。Rust 更新 rustls `0.23.46`、webpki `0.103.15`、h2 `0.4.20`、tar `0.4.46`、anyhow `1.0.104`、serde_with `3.24.0`、rand `0.8.8` 等；通过 Tauri/plist 上游约束更新 quick-xml 至 `0.41.0`。两个锁文件和 Tauri 生成的权限 schema 同步更新，CI Rust 测试使用 `--locked`。 |
| H1 签名环境 | Vite 只暴露 `VITE_` 前缀，原生签名变量不进入前端环境。新增环境边界测试，并以合成签名私钥/密码完成生产构建、扫描全部输出文件。没有据此声称历史密钥已泄露或更换更新公钥。 |

## 实际验证

| 检查 | 结果 |
| --- | --- |
| Node 环境 | nvmd 临时选择已有 Node `20.20.2`，使用 Corepack pnpm `9.15.9`；未修改全局默认版本。原 node_modules 使用 pnpm 11 store，已按 CI pnpm 9 重建。 |
| `pnpm check` | TypeScript 和 oxlint 通过，12 项 Node 测试通过。 |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked` | 201 项通过，0 失败；原有 2 项真实账号测试保持忽略。二进制与文档测试目标通过。 |
| 前端生产构建 | 通过。合成签名值扫描覆盖 12 个输出文件，未命中私钥或密码。 |
| `pnpm tauri build --no-bundle --ci -- --locked` | macOS arm64 原生 Release 编译通过，产物为 `src-tauri/target/release/AiToolkit`；未生成安装包、签名、启动应用或发布。 |
| Windows 交叉检查 | 使用已安装的 `x86_64-pc-windows-msvc` 目标，对新增路径、私有权限、Job Object 代码及相关测试进行隔离类型检查，通过。此检查不代表整应用 Windows 构建或运行通过。 |
| JavaScript 在线复查 | 全依赖公告由 21 条降至 2 条（高危 1、中危 1）；生产依赖 0 条。剩余项见下表。 |
| Rust OSV 在线复查 | 查询锁文件中全部 699 个 crates.io 包版本；包版本命中由 21 个降至 6 个，包含 3 个条件性/平台相关漏洞匹配和 3 个停止维护提醒，GHSA/RUSTSEC 别名不重复计数。 |

本地原始记录位于工作区上层的 `security-review-2026-10-15/`：`rust-tests-after.log`、`native-build-after.log`、`windows-security-typecheck.log`、`frontend-build-after.json`、`node-audit-after.json`、`node-audit-production-after.json`、`rust-osv-after.json` 和 `rust-advisories-after.json`。这些记录不随本仓库提交。

构建仍提示主 chunk 大于 500 kB、原 bundle identifier 以 `.app` 结尾；本次未更改这些既有非安全配置。

## 残余依赖事项

这些是待处理项，不表示已经清零或已获得风险接受批准。维护责任为项目维护者，建议于 **2026-10-30** 复查；此前若对应输入或发布平台改变，应提前评估。

| 组件 | 本次未强行迁移的理由与后续要求 |
| --- | --- |
| braces `3.0.3` | [公告](https://github.com/advisories/GHSA-vfj7-8cjw-p6xm)当前没有可用补丁。现有用途为构建工具链的 glob，配置来自仓库；不在桌面运行时处理远程用户模式。跟踪兼容补丁，不接受不可信构建配置。 |
| postcss-selector-parser `6.1.4` | [剩余公告](https://github.com/advisories/GHSA-rj75-hqrm-r3gf)要求 `>=7.1.6`，Tailwind 3 仍要求 6.x。保持现有样式体系，通过兼容的上游升级处理，未强制跨主版本覆盖。 |
| glib `0.18.5` | [公告](https://rustsec.org/advisories/RUSTSEC-2024-0429.html)属于 Linux GTK 路径。本轮 macOS/Windows 发布目标不激活该实现；发布 Linux 版本前必须评估兼容 GTK/Tauri 链升级。 |
| rand `0.7.3` | [公告](https://rustsec.org/advisories/RUSTSEC-2026-0097.html)要求自定义 logger 重入 ThreadRng 等联合条件；该旧版来自 Tauri HTML 工具链的 phf 构建依赖。本项目直接使用的 rand 已更新到 `0.8.8`，旧链等待上游迁移。 |
| rsa `0.9.10` | [Marvin 公告](https://rustsec.org/advisories/RUSTSEC-2023-0071.html)未给出修复版本。应用使用 RSA 公钥加密，未发现 RSA 私钥解密入口；若未来增加私钥运算，必须重新评估。 |
| fxhash、proc-macro-error、unsafe-libyaml | 上游停止维护提醒，按依赖维护风险跟踪；不将其作为已验证的远程攻击漏洞，也未全局忽略扫描结果。 |

## 发布前仍需人工回归

Windows 整应用构建、WebView 导航/外链、junction 和 ACL 的实际行为；macOS/Windows 的真实 SSH 私有仓库及桌面交互；TRAE、Qoder、WorkBuddy 真实登录、刷新及领取；合成更新包的正确/错误签名路径。这些没有由上述单元测试替代。

**OAuth 兼容边界：** 没有可验证 state 的 refresh-only 旧回调会被拒绝，授权码流程继续使用 PKCE；不能宣称该旧变体已经兼容所有官方授权页。真实登录回归应优先验证此项。不要通过重新接收未绑定凭证来绕过验证。

## 资源预算

默认总量为 100,000 个条目、2 GiB、128 层和每次处理 300 秒；技能文档读取上限为 4 MiB。需要处理更大合法技能时，可为应用进程显式设置下列环境变量；无效或为零的值返回错误。

| 环境变量 | 默认值 |
| --- | --- |
| `AI_TOOLKIT_SKILL_MAX_ENTRIES` | `100000` |
| `AI_TOOLKIT_SKILL_MAX_BYTES` | `2147483648` |
| `AI_TOOLKIT_SKILL_MAX_DEPTH` | `128` |
| `AI_TOOLKIT_SKILL_TIMEOUT_SECS` | `300`，最多 `86400` |

Git 缓存继续支持原有 `MCP_MANAGER_GIT_CACHE_TTL_SECS`；`0` 表示每次重新获取。

## v2.1.0 发布前检查

2026-10-16 将上述安全加固与代理长会话修复合并至 v2.1.0 后，重新通过 `pnpm check`（12 项 Node 测试）、前端生产构建、`cargo test --manifest-path src-tauri/Cargo.toml --locked`（205 项通过、2 项真实账号测试忽略）、`cargo build --manifest-path src-tauri/Cargo.toml --release --locked`，以及官网 VitePress 构建。应用配置、Rust 锁文件、中英文 README 和界面初始版本已统一为 2.1.0。

Windows 打包命令改为 `--bundles msi`，更新产物继续由 `createUpdaterArtifacts` 配置生成。此处验证了本机编译与发布配置，Windows 整应用构建和真实账户、桌面交互的验证边界仍以上述说明为准。
