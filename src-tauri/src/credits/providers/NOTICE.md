# Protocol references

The native CN adapters reference the public protocol implementations in
[mmqz/cpa-multi-plugins](https://github.com/mmqz/cpa-multi-plugins), pinned to
commit `208a4194e41cf4e6a3a92529fda8f3517df5ae44` (2026-10-12).
Qoder COSY signing and request-body encoding are adapted from
`plugins/qoder/sign.go` and `plugins/qoder/encoding.go`.
OAuth, billing, rewards, model discovery, and SSE/tool message shapes reference
the corresponding `plugins/{workbuddy,trae,qoder}` files.

Qoder multimodal message conversion also follows the installed official Qoder CN
0.4.4 agent SDK's `RequestConverter` and request builder: image-bearing user/tool
messages preserve both `content` and `contents`, and the model configuration sets
`is_vl`. Base64 data URLs remain intact, matching the client's inline image path.

WorkBuddy's image arrays follow the installed official CLI request processor's
`normalizeMessageImages`: user and tool content preserve OpenAI `image_url`
parts. TRAE preserves multimodal content arrays while converting tool calls to
`function_call`, following the pinned reference's `plugins/trae/upstream/payload.go`.
These request-shape checks do not establish either account's live vision access.

No source from Buddy Switch (PolyForm Noncommercial) is included. Requests use
official platform origins selected by account region. The reference's community Trae reverse proxy,
content-filter transformations, guessed campaign IDs, and automatic device
rotation are not included.

TRAE check-in routing follows the installed official CN 3.4.4 client's
`out/main.js`: `_requestCheckinCredits` resolves the separate boot-config
`ugApi`, rather than the OAuth account realm. Its local request logs and
read-only checks confirm `https://api.trae.cn` serves check-in status for
`.com.cn` accounts too; the `.com.cn` host serves billing but returns 404
for check-in. Status and claim share this UG gateway.

Qoder CN daily credits use the independently implemented campaign contract
from the [official activity page](https://openapi.qoder.com.cn/growth-page/activity-iframe)
and its published `qbase/qoder/0.0.725` script, checked against Qoder CN 0.4.4.
The account's live list supplies campaign IDs; grants use `POST /sash/api/v1/me/campaigns/{id}/claim`
and uncertain results use the reward/list read APIs. Device proof is read from
the installed official CN helper, never generated or bypassed by this adapter.

The TRAE Work OAuth login shape (IDE client id, authorization-page parameters,
empty `x-cloudide-token` exchange header, IDE_PC DeviceInfo, callback-echo
identity) follows the packet-captured implementation in
[smart-open/TraeWorkAssistant](https://github.com/smart-open/TraeWorkAssistant)
(`src-tauri/src/commands/oauth.rs`); the Rust code here is an independent
implementation.

## TraeWorkAssistant license

MIT License

## cpa-multi-plugins license

MIT License

Copyright (c) 2026 mmqz

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## International adapters (experimental in v2.2.0)

WorkBuddy international configuration is pinned to official 5.9.4.42593613,
commit `fa0a0901906addaa1a4259f6df347388a5563ded`:
[official update manifest](https://www.workbuddy.ai/v2/update?platform=workbuddy-darwin-arm64).
Runtime field contracts were checked against official 5.9.4 `main/common.js`
and `main/workbuddy-auth-product-coordinator.js` (range-extracted official ZIP,
entry CRC verified), following earlier 5.7.6 contract checks.
`workbuddy_intl.json` uses invented identities and amounts. Account selection,
summary versus resource billing, and permitted agent models are kept separate.
Activity rewards follow the [official subscription rules](https://www.codebuddy.ai/docs/workbuddy/Subscription)
and are not submitted as manual claims.
Authenticated personal product configuration and billing passed a live read-only
check (20 catalog models). Personal accounts use `/v3/config`; enterprises use
`/console/enterprises/{enterpriseId}/config/models` and chat-tagged entries.
The legacy personal IDE catalog returned HTTP 500 and is no longer queried for
international accounts. Chat uses the validated official international endpoint
declared by product configuration. Inference has not been tested live.
`workbuddy_intl_catalog_observed.json` contains projected model/agent fields from
the authenticated product response, without credentials or account identifiers.

Qoder international production configuration follows official CLI 1.1.81:
`openapi.qoder.sh`, production OAuth client `e883ade2-e6e3-4d6d-adf7-f92ceff5fdcb`,
and the official international `api2.qoder.sh` model node. International events
follow the [official event page](https://docs.qoder.com/events/100credits)
and [published 0.0.746 activity script](https://g.alicdn.com/qbase/qoder/0.0.746/growth-page/activity-iframe/activity-iframe.js).
The international device helper receives environment `4`; CN remains `0`.
`qoder_intl.json` is synthetic contract data, not captured traffic. International
inference and claims have not been verified with a real account.

TRAE international configuration and encrypted local storage format follow the
verified official 3.6.13 / 2.3.99397 client, obtained via the
[official release service](https://icube-normal.trae.ai/icube/api/v1/native/version/trae/latest).
SG, US and USTTP account, billing and AI origins are distinct. The isolated
international desktop import, identity and billing paths passed a live read-only
check. `trae_intl_billing.json` contains synthetic billing values only.
`trae_intl_catalog_observed.json` and `trae_intl_stream_observed.json` are
projected, sanitized responses from that official isolated client. They retain
catalog-declared model versions and the tool-loop SSE shape; identifiers and
private paths were removed or replaced. No authentication headers or upload
request bodies were captured. Native requests bypass Electron fetch; NetLog
omits upload bodies and the production module has a static INFO trace limit.
The temporary diagnostic client, localhost port and RAM log volume were closed.
International model discovery and chat still return 501 until the complete
request contract and an AI Toolkit integration are verified. The adapter does
not reuse the CN chat protocol for this region.


## MiniMax Code

The independently implemented MiniMax Code adapter follows the official MIT-licensed
[MiniMax-AI/minimax-code](https://github.com/MiniMax-AI/minimax-code/tree/3c21163c85a9684030cb147bfef48c6284fa3c46)
contracts (checked 2026-11-05): `packages/oauth-core/src/{oauth-client,namespace}.ts`,
`packages/tui/src/{runtime/public-gateway,account/matrix-account-client,checkin/http-gateway}.ts`
and `packages/shared/src/daily-signin.ts`. The request attribution constants are
public protocol constants; Bearer credentials provide authorization. Balance fields
also match the installed official desktop membership response. The CN adapter's
identity, personal membership and signin status passed a live read-only check.
Model discovery additionally follows the installed official desktop's
`GET /archon/api/v1/config` contract. Managed inference follows the pinned official
`packages/config/src/config.ts`, `minimax-model-catalog.ts`, and model-system
`resolution/model-resolver-helpers.ts`: CN `agent.minimax.cn` and international
`agent.minimax.io`, `/mavis/api/v1/llm/v1/messages`, Bearer authorization and Mavis
session headers. The live account catalog controls model availability; the app defaults to its
highest advertised context tier. When a live M3/M3.1-Flash entry omits
context tiers, the exact model's official shared-registry 512K/1M tiers fill the
missing metadata; explicit account tier lists remain authoritative.
explicit shared-registry entries provide tool/image capabilities. CN live
verification passed with an isolated in-memory gateway: M3 text, tool-call
JSON and tool-result Messages SSE; M3.1-Flash low-effort Messages JSON. No executable
tool or credit claim was run, and no user database was modified. Global accounts,
image understanding and the complete browser login flow remain unverified.

MIT License

Copyright (c) 2026 MiniMax Code

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
