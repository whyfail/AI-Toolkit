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
official CN platform origins. The reference's community Trae reverse proxy,
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
