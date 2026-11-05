# 国际版适配（v2.2.2 实验性）

v2.2.2 延续独立的 `cn` / `intl` 区域，保留国内版平台标识和模型 ID，包含 MiniMax Code 国内／国际账号适配。各平台按以下范围发布，国际真实账号验证仍不完整。**TRAE 国际版新版模型协议尚未完成完整验证，模型目录与代理暂不开放。**

## 支持与验证矩阵

| 平台／区域 | 登录与额度 | 模型目录与代理 | 真实国际账号验证 |
| --- | --- | --- | --- |
| WorkBuddy 国内版 | 保留现有实现 | `workbuddy/<模型ID>` | 本轮未新增实测 |
| WorkBuddy 国际版 | 已实现区域 OAuth、所选账号轮询和账单解析 | `workbuddy-intl/<模型ID>`，个人账号使用认证后的产品目录，企业账号按企业配置过滤 | 真实账号余额和 20 个模型的目录查询通过；推理调用未实测 |
| TRAE 国内版 | 保留现有实现 | `traework/<模型ID>` | 本轮未新增实测 |
| TRAE 国际版 | 已实现区域 OAuth、本机导入、SG／US／USTTP 路由与原生额度 | `traework-intl/<模型ID>` 前缀预留；目录和代理暂时返回 HTTP 501 | 身份和账单只读查询通过；已取得官方目录与工具闭环响应，仍缺请求体和认证头 |
| Qoder 国内版 | 保留现有实现 | `qoder/<模型ID>` | 本轮未新增实测 |
| Qoder 国际版 | 已实现生产 OAuth、刷新、额度、活动和国际设备助手 | `qoder-intl/<模型ID>` | **未实测**；官方配置与合成契约夹具验证 |
| MiniMax Code 国内版 | 设备授权、本机导入、个人余额和每日签到 | `minimax-code/<模型ID>`，实时目录与双协议代理 | 身份、余额、签到状态和目录只读查询，以及部分文本／工具推理已有通过记录；真实领取、完整网页登录与图片理解未实测 |
| MiniMax Code 国际版 | 独立区域设备授权、本机导入、余额和签到适配 | `minimax-code-intl/<模型ID>` | **未实测**；离线转换、领取回查和区域隔离回归 |

网关模拟测试覆盖八组平台／区域路由、两种客户端协议、JSON／SSE、图片、工具结果及思考内容。模拟成功不代表官方模型端点已经联调。TRAE 国际版的真实适配器在取得完整协议前拒绝模型请求，不发送国内版聊天请求体。MiniMax Code 的具体能力和既有验证记录见 [积分与模型代理](credits-proxy.md)。

## 连接、迁移与调度

连接账户时先选择国内版或国际版。登录与本机导入 API 的可选 `region` 参数默认 `cn`；JSON 凭据也携带 `region`。TRAE 本机导入分别读取 `Trae CN` 和 `Trae`，国际版不回退到没有版本标识的旧 CLI 登录态。凭据区域和账户区域不一致时拒绝认证或刷新结果。

账户唯一键为平台、区域和用户 ID；同一用户 ID 可以在国内、国际区域共存。旧数据默认 `cn`，保留原账户 ID、钥匙串引用、余额快照和请求历史。文件数据库迁移前通过 SQLite `VACUUM INTO` 创建备份，再在事务内重建唯一约束；重复启动不重复迁移。macOS／Linux 备份权限为 `0600`。

带前缀的模型 ID 只使用对应平台与区域的账户。无前缀的精确 GLM 别名继续仅使用国内账户。认证、限流或额度失败不得转到另一区域；流开始后也不重新调度。国际模型来自账户目录，允许非 GLM 模型；身份校验接受目录明确声明的底层版本 ID，不按显示名称猜测版本。

账户卡片、模型列表、信息卡、复制入口和请求记录显示对应区域或完整代理 ID。上下文默认选择当前账号目录声明的最高档位；WorkBuddy 国际版从 `supportedLengths` 与已声明窗口中取最大值，保留原生输出上限。目录选择仍受上游实际权限和限制约束。

WorkBuddy 国际个人账号从认证后的 `/v3/config` 读取模型和 `cli` 可选范围，不请求旧 IDE 的 `/console/enterprises/personal/models`（实测返回 HTTP 500）。企业账号按官方 5.9.4 的 `/console/enterprises/{enterpriseId}/config/models` 配置及 `chat` 标签处理。目录和聊天采用配置明确返回的国际模型节点；仅接受 `www.workbuddy.ai` / `www.codebuddy.ai` 的 HTTPS 根地址，拒绝国内域名、任意主机及带路径、凭据或查询参数的地址。企业目录失败仍保留错误，不以个人模型覆盖权限。

## 原生额度与奖励

积分、USD、请求次数按原生单位展示，跨单位不合计。未知余量保持未知；TRAE USD 保留小数，旧请求配额独立展示。WorkBuddy 国际账单以汇总为总额依据，资源明细按资源 ID 去重，不把明细再次加到总额。

WorkBuddy 国际版符合条件的客户端活跃奖励由平台自动结算，一般次日发放；没有手动签到接口。仅登录或由脚本产生的活动不等于符合条件的活跃，说明遵循[官方订阅规则](https://www.codebuddy.ai/docs/workbuddy/Subscription)。TRAE 国际版不调用国内签到接口。

Qoder 国际活动从实时列表读取任务和领取资格，只处理明确可领取的积分任务；不使用固定活动 ID。结果不明确时回查奖励与列表，不重复提交。国际设备助手从国际客户端目录读取，环境编号 `4`；国内维持 `0`。官方活动规则见[每日积分说明](https://docs.qoder.com/events/100credits)，平台设备校验拒绝时保留失败。

## TRAE 协议核实状态

基线为官方国际版 **3.6.13 / 2.3.99397**。隔离配置正常登录后，官方客户端普通最小对话曾返回 `toolkit-ok`。AI Toolkit 的独立只读测试已验证本机凭据解析、身份及原生单位账单查询；没有执行活动领取。

HTTPS 代理采样因原生 TLS 证书校验失败，未取得请求体。该临时信任已撤销，采样代理已停止，系统代理未改动，证书校验保持启用。

经授权使用隔离客户端内置的 `--AutoCollectorIDE=Trae` 与仅监听 `127.0.0.1` 的诊断端口，通过官方 NetLog 取得目录及工具闭环响应。原始网络日志通过内存管道处理，脱敏后保存字段投影。样本确认了 `batch_get_detail_param` 的功能目录、`create_agent_task` 和 `commit_toolcall_result` 端点，以及 `model_config`、`thought`、`tool_call`、`turn_completion` 等事件。工具参数采用逐字段片段，不能直接沿用国内版 `output` / `done` 处理方式。

样本位于 `trae_intl_catalog_observed.json` 和 `trae_intl_stream_observed.json`。目录明确声明的底层 `__dev` / `__max` 版本可用于身份核验；账号标识、历史消息和私人路径已移除或替换，token 限制因采样脱敏未保留，不能据此推断上下文大小。这些是响应证据，尚不构成完整聊天契约。

原生模块绕过 Electron `net.fetch`，NetLog 不含上传请求体；生产模块静态设置 `max_level_info`，详细日志参数也无法开启请求体输出。尝试详细日志时使用临时内存盘，原始数据未写入持久磁盘。采样结束已关闭隔离客户端及诊断端口、卸载内存盘，没有增加证书信任。

仍须取得最小对话和工具闭环的请求体、认证头，再实现国际模型适配并完成 AI Toolkit 的真实最小调用。当前 `trae_intl_billing.json` 仅为账单合成夹具，响应样本也不能证明新版聊天兼容国内协议。

## 协议与离线验证

本地代理沿用 OpenAI Chat Completions 与 Anthropic Messages。Messages 请求明确启用 `thinking` 且未设置 `display: omitted` 时，可以返回无签名思考块；空签名不代表 Anthropic 签名证明。默认请求保持仅输出正文和工具；历史中的非空签名思考仍明确拒绝。SSE 先结束思考块，再输出正文及工具块，保留工具调用 ID 和结果。

夹具位于 `src-tauri/src/credits/providers/fixtures/`，分别注明合成数据或官方脱敏响应，不含真实账户、令牌或采样私钥。测试覆盖旧库迁移与重复迁移、跨区域同用户、刷新后的区域保留、WorkBuddy 余额去重和目录权限、TRAE USD 精度与请求配额、官方目录声明的底层版本、Qoder 国际设备编号、区域隔离和双协议网关。

离线检查命令为 `pnpm check`、`cargo test --manifest-path src-tauri/Cargo.toml --locked` 和 `pnpm build`。真实 TRAE 只读诊断测试默认忽略，仅在显式指定隔离配置并启用诊断开关后运行；它不保存账户凭据到项目。

2026-11-03 本轮验证：`pnpm check` 的 38 项测试通过，Rust 完整测试 307 项通过、4 项忽略，主项目及官网构建成功，两个仓库的 `git diff --check` 通过。主项目前端构建仍有既有的 500 kB 分块大小提示。WorkBuddy 保存账号的真实余额和 20 个模型目录只读检查单独运行通过，开发环境后台快照的 HTTP 500 诊断已清除；未执行推理或领取。TRAE 真实身份与账单只读检查单独运行通过；官方客户端普通对话和读取测试文件成功，尚未完成 AI Toolkit 国际模型端点的真实调用。

2026-11-04 v2.2.0 发布检查：前端 38 项测试、Rust 309 项测试通过，4 项真实账号或诊断测试按默认设置忽略，主项目生产构建通过。本次发布检查未额外执行真实推理或领取；矩阵中的真实账号记录来自上述既有验证。

2026-11-05 v2.2.1 发布检查：前端 42 项、Rust 325 项测试通过，6 项真实账号或诊断测试按默认设置忽略，主项目生产构建通过。本次发布检查未额外执行真实授权、推理或领取；各平台真实账号状态沿用已有验证记录。

2026-11-05 v2.2.2 发布检查：前端 42 项、Rust 326 项测试通过，6 项真实账号或诊断测试按默认设置忽略，主项目生产构建通过。新增本地 HTTP 回归验证目录容量、多账号共同最小值与未知输出上限；未额外执行真实推理或领取。

## English status

v2.2.2 retains **experimental international adapters** and MiniMax Code CN/global accounts within the scope below. Accounts and credentials carry `cn` or `intl`; the default remains China. Account uniqueness and routing include the region. Existing account IDs, secure credentials, snapshots and history survive a backed-up, transactional migration. Credits, USD and request quotas are never added together, and unknown balances remain unknown.

WorkBuddy international billing and a 20-model catalog passed a live read-only check. Personal accounts use authenticated `/v3/config` model and agent permissions; enterprise accounts use the official enterprise configuration endpoint. Chat uses the configuration-declared, validated international model node. WorkBuddy inference remains untested live. Qoder international adapters have offline contract coverage but are **not tested with real international accounts**. Their model prefixes are `workbuddy-intl/` and `qoder-intl/`. Bare exact GLM aliases remain domestic-only. Requests never retry across regions. WorkBuddy activity rewards settle automatically under the official rules; Qoder claims only live eligible credit tasks using the international helper environment `4`.

TRAE international 3.6.13 isolated desktop import, identity and billing passed a live read-only check. Sanitized official catalog and tool-loop SSE responses are now recorded, including explicitly declared underlying model versions. Authentication headers and request bodies remain unavailable: native requests bypass Electron fetch, NetLog omits upload bodies, and the production module statically disables debug traces. Scoped trust was revoked, the temporary client and localhost diagnostic port were closed, and the RAM volume used for verbose diagnostics was detached. `traework-intl/` remains reserved; catalog and chat return HTTP 501 pending the complete request contract and a successful AI Toolkit integration. Full international integration is **not complete**.

MiniMax Code uses separate `minimax-code/` and `minimax-code-intl/` routes, regional device OAuth, safe local access-token import, personal credits, daily check-in, live model discovery, and a managed Messages adapter. Prior CN checks cover identity, billing, sign-in status, model discovery, and selected short text/tool inference through an isolated gateway. Global accounts, real credit claims, image understanding, and the complete browser login flow remain unverified. Offline gateway tests cover all eight platform/region combinations.
