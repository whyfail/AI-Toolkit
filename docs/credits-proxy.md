# 积分与模型代理

AI-Toolkit 的「积分与代理」页面管理 WorkBuddy、TRAE CN、Qoder 国内账号。账号积分保留在来源平台；本地网关转发模型请求并消耗对应账号的额度。

下图为 macOS v2.0.0 实际窗口截图，昵称和账号 ID 已打码。余额、领取状态和模型数量仅代表截图时的账户状态。

![账户与积分面板，身份信息已打码](../assets/screenshots/credits-panel.png)

## 添加账号与领取

选择平台后使用网页登录授权，或导入自己的访问凭据。手动导入支持单个访问 Token 或包含 `accessToken`、`refreshToken`、`userId`、`deviceId`、`expiresAt`、`extra` 的 JSON；`expiresAt` 使用 Unix 毫秒。凭据存入系统钥匙串／凭据管理器，数据库只保存账号与积分元数据。

TRAE CN「一键导入本机凭据」优先读取新版桌面应用的个人登录态，未找到桌面登录态时兼容 CLI 钥匙串凭据。桌面应用与旧企业 CLI 的登录态独立，旧 CLI 凭据过期不代表桌面账号已退出。导入后会验证身份并同步积分；同步失败时保留弹窗并显示实际原因。

刷新余额会查询实时模型目录与领取资格。WorkBuddy 和 TRAE CN 查询签到奖励；Qoder 查询活动积分。兑换券单独显示，不能计为积分。TRAE 签到返回的每日奖励金额不能作为可消费余额。未知额度显示未知，失败时保留并标记旧快照。

Qoder CN 的[每日 100 Credits 活动](https://docs.qoder.cn/events/100credits)每天北京时间 10:00 开放新一轮领取，奖励领取后 30 天有效。刷新账户后，账户卡片的“领取积分”和顶部“一键领取积分”均直接调用官方活动接口，在 AI Toolkit 内完成领取并记录结果。活动 ID、名称、金额和领取资格来自当前账号的实时活动列表，不能用固定 ID 或活动入口链接推断领取资格。

Qoder 活动请求携带已保存的设备身份及官方活动客户端请求头。本机已安装国内客户端或国内 CLI 的设备校验助手时，领取前通过该助手读取当前账号的设备凭据，无需启动 Qoder 界面；不伪造设备令牌，也不轮换设备身份。上游拒绝校验时保留失败原因。领取超时或返回待确认时只回查奖励和活动状态，不自动重复提交。

Qoder 账户卡片与 TRAE 一样仅显示积分任务、金额及领取状态，不展示活动说明、规则入口、平台网页活动或优惠券。只有平台明确返回 `CLAIM_BENEFIT` 和 `CREDITS` 的任务才会由“一键领取积分”处理；兑换码、优惠券和查看详情活动不会被当作积分领取。

「一键领取积分」逐账号处理当前符合条件的奖励。自动领取默认关闭；开启后仅在应用运行期间按北京时间每日 10:30 检查并领取，10:30 前启动会等待到点，当天 10:30 后启动会补做检查，从托盘退出应用后停止。同一账号的任务串行执行，超时或状态不明确时先查询，不盲目重复领取。

## 选择目录模型与开启代理

切换到「本地代理」可查看运行状态、Base URL、支持的协议与 API Key 复制入口；界面不显示实际密钥。

![本地模型代理连接参数](../assets/screenshots/proxy-panel.png)

连接账号并刷新模型目录后，在「本地代理」复制带平台前缀的模型 ID，例如 `workbuddy/glm-5.3-flash`。该 ID 指定来源平台及其目录中的模型，上游返回不同型号时代理会拒绝响应。目录发现不等同于推理或工具能力已经实测；普通调用会消耗来源平台额度。

开启代理后默认地址为 `http://127.0.0.1:17860/v1`。接口包括 `GET /health`、`GET /v1/models`、OpenAI Chat Completions 的 `POST /v1/chat/completions` 和 Anthropic Messages 的 `POST /v1/messages`。使用网关生成的 API Key：OpenAI 接口使用 `Authorization: Bearer <key>`；Messages 接口支持 `x-api-key` 或 Bearer。修改端口前停止代理；轮换密钥后更新客户端中的密钥。

`workbuddy/<实际模型ID>`、`traework/<实际模型ID>`、`qoder/<实际模型ID>` 固定来源平台。准确的 `glm-<版本>` 别名在已验证支持同一型号的账号间调度，优先使用即将到期的积分。不同 GLM 版本不会合并。只有确认未执行的认证、限流或额度错误可以切换账号；开始返回内容后不重试。网络超时不会被当作尚未执行。

日志只记录来源账号、模型、结果、耗时与上游报告的 Token 数，不保存请求正文、授权令牌或 API Key。Token 数不能直接换算为平台积分扣减。

## 通用客户端配置

1. 连接账号、刷新模型目录并开启代理。
2. 在「本地代理」复制实际 Base URL、代理密钥和目录模型 ID。
3. 在客户端的模型设置中添加自定义提供商，选择 OpenAI Chat Completions 或 Anthropic Messages，填写上述参数，再添加模型并启用。
4. 先测试普通对话，再测试读取文件、修改文件及返回结果的工具调用任务。

两种协议均使用上述带 `/v1` 的 Base URL；客户端分别请求 `/chat/completions` 或 `/messages`。代理支持文本和工具调用；不支持的内容或参数会返回明确错误。代理地址是模型服务地址，应填写到提供商的 Base URL 字段。

## 实现依据与验证边界

协议参考固定为 [cpa-multi-plugins 208a4194](https://github.com/mmqz/cpa-multi-plugins/tree/208a4194e41cf4e6a3a92529fda8f3517df5ae44)。平台适配器直接请求官方来源，许可声明见 `src-tauri/src/credits/providers/NOTICE.md`。

离线测试验证解析、签名、模型隔离、SSE、工具参数、数据库和本地网关。三平台真实账号的登录、领取、余额、GLM 选择与客户端编程闭环仍需通过应用授权实测；通过离线测试不能证明当前账号拥有 GLM 权限。无法直连或准确确认型号的路线必须保持未验证，不能宣称三平台 GLM 已全部可用。

TRAE CN 3.4.2 的 macOS 桌面凭据读取、真实账号身份和积分余额查询已通过只读实测；该验证未执行领取或模型推理。

Qoder 每日活动的请求与返回契约已核对官方活动页脚本及本机 Qoder CN 0.4.4 的活动日志。本机官方设备校验助手的无界面读取已通过实测；领取接口通过本地 HTTP 回归测试验证。当前 AI Toolkit 保存的 Qoder 账号存在钥匙串读取错误，尚未完成真实账号领取到账验证。

本次修复已用本机 WorkBuddy 账号的 `workbuddy/glm-5.3-flash` 完成一次短文本 Messages 请求实测，返回 HTTP 200 和正常文本。网关状态、代理测试密钥和请求记录使用独立内存存储，未修改本机账号数据库；这不代表其他模型或三平台工具能力均已实测。

macOS 使用 Keychain，Windows 使用 Credential Manager，Linux 使用 Secret Service；凭据库不可用时返回明确错误，不退回明文文件。构建与测试命令：`pnpm check`、`cargo test --manifest-path src-tauri/Cargo.toml`、`pnpm tauri build`。Windows 安装包由现有 Windows CI 构建验证，本机 macOS 无法完成 Windows 运行验收。

## 系统凭据授权弹窗

凭据首次读取成功后只缓存在 Rust 后端内存中，退出应用后释放；macOS 共享凭据条目只读取、解码一次。启动、后台刷新、自动领取和代理请求均禁止钥匙串交互，需要授权时返回明确错误。只有点击“刷新”、登录等主动操作，才允许为共享条目请求一次授权。

旧版本的账号凭据可能被拆成多条钥匙串记录，macOS 对每条记录分别授权，“始终允许”某一条不等于授权其余记录。旧条目现在只进行静默迁移，任何一块需要授权时，立即停止该账号的迁移并提示重新登录；不会逐块弹窗，也不会阻止已迁移账号使用。重新登录直接更新共享条目，不先读取旧分块；数据库保存失败时保留原凭据。

本地代理 API Key 不属于平台账号。点击“一键开启代理”“复制 API Key”或“检测连接”时，只针对该密钥授权并迁移旧版记录；拒绝或取消后，可再次点击同一按钮重试。授权或写入失败时保留原密钥，不自动轮换，不要求重新登录平台账号，也不会读取其他账号的旧分块。迁移完成后复用内存缓存，后台请求仍禁止钥匙串交互。

Token 更新使用 SecItemAdd/Update，删除通过元数据查询条目名称后调用 SecItemDelete，避免旧 keyring 实现更新／删除前先读取秘密数据而再次触发授权。静默访问使用 macOS 原生交互开关，进程内所有钥匙串调用串行化，结束后恢复开关，避免影响主动导入 TRAE CLI 凭据。Windows/Linux 保持原有系统凭据后端与分块布局。

共享凭据库不可用或用户拒绝授权后，本次运行暂停系统凭据访问，直到下一次主动操作才重试。回归测试覆盖启动零授权请求、共享条目单次授权、旧分块失败隔离、重新登录及回滚、删除时不读取旧秘密数据；本机原生测试使用独立测试钥匙串验证锁定后连续 8 次读取直接返回权限错误，并检查交互开关及错误退出后的恢复。缓存不写入数据库或明文文件。这些修改需要重新构建的应用或更新安装包才会生效。

macOS 的“始终允许”与应用的代码签名身份有关。已检查的本机开发程序和现装应用均使用 ad-hoc 签名，重新编译或更换安装包后仍可能需要首次授权；要让正式版本升级后继续被识别为同一应用，应使用稳定的 Developer ID Application 身份签名。`TAURI_SIGNING_PRIVATE_KEY` 只负责更新包校验，不能代替 macOS 应用签名；正式构建时安装相应证书并设置 `APPLE_SIGNING_IDENTITY`。见 [Apple 代码身份说明](https://developer.apple.com/library/archive/documentation/Security/Conceptual/CodeSigningGuide/AboutCS/AboutCS.html)和 [Tauri macOS 签名配置](https://v2.tauri.app/distribute/sign/macos/)。

Windows 通过 [CredRead](https://learn.microsoft.com/windows/win32/api/wincred/nf-wincred-credreadw) 读取当前登录会话的凭据，正常读取不使用 macOS 的逐应用授权机制。Linux 的 Secret Service 在密钥环锁定时[可以弹出解锁窗口](https://specifications.freedesktop.org/secret-service/latest/unlocking.html)，是否自动解锁取决于桌面环境和登录配置。此处验证了缓存与失败暂停逻辑，没有在 Windows/Linux 实机验证系统弹窗行为。
