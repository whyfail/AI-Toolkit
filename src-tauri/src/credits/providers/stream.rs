use super::*;
use futures::StreamExt;
use std::collections::VecDeque;

struct State {
    input: Pin<Box<dyn Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>>,
    buffer: Vec<u8>,
    queue: VecDeque<Result<ChatChunk, ProviderError>>,
    platform: Platform,
    identity: Option<String>,
    id: String,
    ended: bool,
    finished: bool,
    answered: bool,
    shape_logged: bool,
    upstream_request_id: Option<String>,
    minimax_tools: std::collections::HashMap<u64, (usize, bool)>,
    minimax_tool_count: usize,
    minimax_usage: Value,
    minimax_finish: Option<String>,
}
pub(super) async fn response(
    r: RequestBuilder,
    p: Platform,
) -> Result<ProviderChat, ProviderError> {
    let r = tokio::time::timeout(std::time::Duration::from_secs(60), r.send())
        .await
        .map_err(|_| ProviderError::new(504, "等待平台响应超时"))?
        .map_err(ProviderError::transport)?;
    if !r.status().is_success() {
        let status = r.status();
        let request_id = upstream_request_id(r.headers());
        let body = r.text().await.unwrap_or_default();
        let mut error = upstream_body_error(status.as_u16(), &body);
        error.upstream_request_id = request_id.or(error.upstream_request_id);
        return Err(error);
    }
    let request_id = upstream_request_id(r.headers());
    let state = State {
        input: Box::pin(r.bytes_stream()),
        buffer: vec![],
        queue: VecDeque::new(),
        platform: p,
        identity: None,
        id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        ended: false,
        finished: false,
        answered: false,
        shape_logged: false,
        upstream_request_id: request_id,
        minimax_tools: Default::default(),
        minimax_tool_count: 0,
        minimax_usage: json!({}),
        minimax_finish: None,
    };
    Ok(ProviderChat {
        stream: Box::pin(futures::stream::unfold(state, |mut s| async move {
            loop {
                if let Some(c) = s.queue.pop_front() {
                    return Some((c, s));
                }
                if s.ended {
                    return None;
                }
                let next =
                    tokio::time::timeout(std::time::Duration::from_secs(180), s.input.next()).await;
                match next {
                    Ok(Some(Ok(bytes))) => {
                        s.buffer.extend_from_slice(&bytes);
                        if s.buffer.len() > 4 * 1024 * 1024 {
                            s.ended = true;
                            return Some((
                                Err(s.error(ProviderError::new(502, "平台 SSE 帧超过大小限制"))),
                                s,
                            ));
                        }
                        while let Some((n, sep)) = boundary(&s.buffer) {
                            let frame = s.buffer[..n].to_vec();
                            s.buffer.drain(..n + sep);
                            s.parse(&frame);
                            if s.ended {
                                break;
                            }
                        }
                    }
                    Ok(None) => {
                        s.ended = true;
                        if !s.finished {
                            return Some((Err(s.error(ProviderError::new(502, "平台流在完成前中断"))), s));
                        }
                    }
                    _ => {
                        s.ended = true;
                        return Some((Err(s.error(ProviderError::new(502, "平台响应流中断或超时"))), s));
                    }
                }
            }
        })),
    })
}
fn boundary(b: &[u8]) -> Option<(usize, usize)> {
    b.windows(2)
        .position(|w| w == b"\n\n")
        .map(|n| (n, 2))
        .or_else(|| b.windows(4).position(|w| w == b"\r\n\r\n").map(|n| (n, 4)))
}
impl State {
    fn error(&self, mut error: ProviderError) -> ProviderError {
        error.stage = "stream";
        if error.upstream_request_id.is_none() {
            error.upstream_request_id = self.upstream_request_id.clone();
        }
        error
    }

    fn fail(&mut self, e: ProviderError) {
        self.queue.push_back(Err(self.error(e)));
        self.ended = true;
    }
    fn parse(&mut self, b: &[u8]) {
        let Ok(s) = std::str::from_utf8(b) else {
            self.fail(ProviderError::new(502, "平台 SSE 编码无效"));
            return;
        };
        let event = s
            .lines()
            .find_map(|l| l.strip_prefix("event:"))
            .unwrap_or("")
            .trim();
        let raw = s
            .lines()
            .filter_map(|l| l.strip_prefix("data:").map(str::trim_start))
            .collect::<Vec<_>>()
            .join("\n");
        if raw.is_empty() {
            return;
        }
        if raw.trim() == "[DONE]" {
            if self.platform == Platform::MinimaxCode {
                self.fail(ProviderError::new(502, "MiniMax Code 响应缺少 message_stop"));
                return;
            }
            self.finished = true;
            self.ended = true;
            return;
        }
        let Ok(mut v) = serde_json::from_str::<Value>(&raw) else {
            self.fail(ProviderError::new(502, "平台 SSE 数据无效"));
            return;
        };
        if self.platform == Platform::MinimaxCode {
            self.parse_minimax(event, &v);
            return;
        }
        if self.platform == Platform::Qoder {
            if let Some(status) = v.get("statusCodeValue").and_then(Value::as_u64) {
                if status >= 400 {
                    let body = v.get("body").unwrap_or(&v);
                    let raw = body.as_str().map(str::to_owned).unwrap_or_else(|| body.to_string());
                    let mut e = upstream_body_error(status as u16, &raw);
                    e.rejected &= !self.answered;
                    self.fail(e);
                    return;
                }
            }
            if let Some(body) = v.get("body") {
                v = if let Some(s) = body.as_str() {
                    if s.trim() == "[DONE]" {
                        self.finished = true;
                        self.ended = true;
                        return;
                    }
                    match serde_json::from_str(s.trim_start_matches("data:").trim()) {
                        Ok(v) => v,
                        Err(_) => {
                            self.fail(ProviderError::new(502, "Qoder SSE 信封数据无效"));
                            return;
                        }
                    }
                } else {
                    body.clone()
                };
            }
            // TODO: remove once the qoder identity key is confirmed live.
            if !self.shape_logged && !self.answered {
                self.shape_logged = true;
                eprintln!("credits: Qoder SSE 事件 {}", super::json_shape(&v, 0));
            }
        }
        if event == "error" || v.get("error").is_some() {
            let code = num(&v, &["code", "biz_code"]).unwrap_or(0.0) as i64;
            self.fail(ProviderError {
                status: if matches!(code, 1005 | 4008) {
                    429
                } else {
                    502
                },
                message: format!("平台流内错误（代码 {code}）"),
                rejected: !self.answered && matches!(code, 1005 | 4008),
                retry_after_ms: None,
                pause_auto: false,
                ..ProviderError::new(502, "")
            }.with_upstream_details(&v, None));
            return;
        }
        // TRAE's extra_info.model identifies a deployment endpoint (ep-...),
        // not the selected model reported by its metadata event.
        if self.platform != Platform::Traework || event != "extra_info" {
            if let Some(actual) = text(&v, &["model", "model_name", "selected_model"]) {
                self.identity = Some(actual);
            }
        }
        if self.platform == Platform::Traework {
            match event {
                "output" => {
                    let mut delta = json!({});
                    if let Some(c) = v.get("response").filter(|v| v.is_string()) {
                        delta["content"] = c.clone()
                    };
                    if let Some(c) = v.get("reasoning_content").filter(|v| v.is_string()) {
                        delta["reasoning_content"] = c.clone()
                    };
                    if let Some(t) = v.get("tool_calls").filter(|v| !v.is_null()) {
                        delta["tool_calls"] = normalize_tools(t.clone());
                    }
                    self.emit(delta, None, None);
                }
                "token_usage" => self.emit(json!({}), None, Some(v)),
                "done" => {
                    self.emit(
                        json!({}),
                        Some(text(&v, &["finish_reason"]).unwrap_or("stop".into())),
                        None,
                    );
                    self.finished = true;
                    self.ended = true;
                }
                "metadata" => {
                    if self.identity.is_some() {
                        self.emit(json!({"role":"assistant"}), None, None)
                    }
                }
                _ => {}
            }
        } else if v.get("choices").is_some() {
            self.answered |= v["choices"]
                .as_array()
                .is_some_and(|cs| cs.iter().any(|c| has_answer(&c["delta"])));
            if v["choices"]
                .as_array()
                .is_some_and(|cs| cs.iter().any(|c| c["finish_reason"].is_string()))
            {
                self.finished = true;
            }
            self.queue.push_back(Ok(ChatChunk {
                data: v,
                upstream_model: self.identity.clone(),
            }));
        }
    }
    fn parse_minimax(&mut self, event: &str, value: &Value) {
        let event = value["type"].as_str().unwrap_or(event);
        if self.identity.is_none() && !matches!(event, "message_start" | "error" | "ping") {
            self.fail(ProviderError::new(502, "MiniMax Code 在模型身份确认前返回了内容"));
            return;
        }
        match event {
            "message_start" => {
                self.identity = text(&value["message"], &["model"]);
                if self.identity.is_none() {
                    self.fail(ProviderError::new(502, "MiniMax Code 未返回模型身份"));
                    return;
                }
                self.update_minimax_usage(&value["message"]["usage"]);
                self.emit(json!({"role":"assistant"}), None, Some(self.minimax_usage()));
            }
            "content_block_start" => {
                let block = &value["content_block"];
                match block["type"].as_str() {
                    Some("tool_use") => {
                        let Some(block_index) = value["index"].as_u64() else {
                            self.fail(ProviderError::new(502, "MiniMax Code 工具块缺少序号")); return;
                        };
                        if self.minimax_tools.contains_key(&block_index) || !block["input"].is_object()
                            || text(block, &["id"]).is_none() || text(block, &["name"]).is_none() {
                            self.fail(ProviderError::new(502, "MiniMax Code 工具块格式无效")); return;
                        }
                        let index = self.minimax_tool_count;
                        self.minimax_tool_count += 1;
                        let has_arguments = block["input"].as_object().is_some_and(|input| !input.is_empty());
                        self.minimax_tools.insert(block_index, (index, has_arguments));
                        self.emit(json!({"tool_calls":[{"index":index,"id":block["id"],"type":"function",
                            "function":{"name":block["name"],"arguments":if has_arguments { block["input"].to_string() } else { String::new() }}}]}), None, None);
                    }
                    Some("text") => {
                        if let Some(text) = block["text"].as_str().filter(|text| !text.is_empty()) {
                            self.emit(json!({"content":text}), None, None);
                        }
                    }
                    Some("thinking") => {
                        if let Some(text) = block["thinking"].as_str().filter(|text| !text.is_empty()) {
                            self.emit(json!({"reasoning_content":text}), None, None);
                        }
                    }
                    _ => self.fail(ProviderError::new(502, "MiniMax Code 返回了不支持的内容块")),
                }
            }
            "content_block_delta" => {
                let delta = &value["delta"];
                match delta["type"].as_str() {
                    Some("text_delta") => self.emit(json!({"content":delta["text"]}), None, None),
                    Some("thinking_delta") => self.emit(json!({"reasoning_content":delta["thinking"]}), None, None),
                    Some("input_json_delta") => {
                        let tool = value["index"].as_u64().and_then(|index| self.minimax_tools.get_mut(&index));
                        let (Some((index, has_arguments)), Some(arguments)) = (tool, delta["partial_json"].as_str()) else {
                            self.fail(ProviderError::new(502, "MiniMax Code 工具参数分片缺少关联块")); return;
                        };
                        *has_arguments |= !arguments.is_empty();
                        let index = *index;
                        self.emit(json!({"tool_calls":[{"index":index,"function":{"arguments":arguments}}]}), None, None);
                    }
                    Some("signature_delta") => {}, // No Anthropic signature proof is advertised by this proxy.
                    _ => self.fail(ProviderError::new(502, "MiniMax Code 返回了不支持的内容分片")),
                }
            }
            "content_block_stop" => {
                if let Some((index, false)) = value["index"].as_u64().and_then(|index| self.minimax_tools.remove(&index)) {
                    self.emit(json!({"tool_calls":[{"index":index,"function":{"arguments":"{}"}}]}), None, None);
                }
            }
            "message_delta" => {
                self.update_minimax_usage(&value["usage"]);
                if let Some(reason) = value["delta"]["stop_reason"].as_str() {
                    self.minimax_finish = Some(match reason {
                        "tool_use" => "tool_calls",
                        "max_tokens" | "model_context_window_exceeded" => "length",
                        "end_turn" | "stop_sequence" | "refusal" => "stop",
                        _ => { self.fail(ProviderError::new(502, "MiniMax Code 返回了不支持的结束原因")); return; }
                    }.into());
                }
            }
            "message_stop" => {
                let Some(finish) = self.minimax_finish.take() else {
                    self.fail(ProviderError::new(502, "MiniMax Code 响应缺少结束原因")); return;
                };
                self.emit(json!({}), Some(finish), Some(self.minimax_usage()));
                self.finished = true;
                self.ended = true;
            }
            "error" => {
                let status = match value["error"]["type"].as_str() {
                    Some("authentication_error") => 401,
                    Some("permission_error") => 403,
                    Some("rate_limit_error") => 429,
                    Some("invalid_request_error") => 400,
                    _ => 502,
                };
                let mut error = upstream_body_error(status, &value.to_string());
                error.rejected &= !self.answered;
                self.fail(error);
            }
            "ping" => {},
            _ => self.fail(ProviderError::new(502, "MiniMax Code 返回了无法识别的流事件")),
        }
    }
    fn update_minimax_usage(&mut self, usage: &Value) {
        for field in ["input_tokens", "output_tokens", "cache_read_input_tokens", "cache_creation_input_tokens"] {
            if let Some(value) = usage[field].as_u64() { self.minimax_usage[field] = json!(value); }
        }
    }
    fn minimax_usage(&self) -> Value {
        let input = ["input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens"].iter()
            .map(|field| self.minimax_usage[*field].as_u64().unwrap_or(0)).fold(0_u64, u64::saturating_add);
        let output = self.minimax_usage["output_tokens"].as_u64().unwrap_or(0);
        json!({"prompt_tokens":input,"completion_tokens":output,"total_tokens":input.saturating_add(output)})
    }
    fn emit(&mut self, delta: Value, finish: Option<String>, usage: Option<Value>) {
        self.answered |= has_answer(&delta);
        let mut v = json!({"id":self.id,"object":"chat.completion.chunk","created":now_ms()/1000,"model":"","choices":[{"index":0,"delta":delta,"finish_reason":finish}]});
        if let Some(u) = usage {
            v["usage"] = u;
        }
        self.queue.push_back(Ok(ChatChunk {
            data: v,
            upstream_model: self.identity.clone(),
        }));
    }
}
fn has_answer(d: &Value) -> bool {
    d.get("content")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty())
        || d.get("reasoning_content")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty())
        || d.get("tool_calls")
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty())
}
fn normalize_tools(v: Value) -> Value {
    let mut calls = if let Value::Array(a) = v { a } else { vec![v] };
    for (i, c) in calls.iter_mut().enumerate() {
        if let Some(o) = c.as_object_mut() {
            if let Some(f) = o.remove("function_call") {
                o.insert("function".into(), f);
            }
            if o.get("id")
                .is_some_and(|id| id.is_null() || id.as_str() == Some(""))
            {
                o.remove("id");
            }
            o.entry("index").or_insert(json!(i));
            if let Some(f) = o.get_mut("function").and_then(Value::as_object_mut) {
                f.remove("namespace");
                f.remove("partial_arguments");
            }
        }
    }
    json!(calls)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn state(p: Platform) -> State {
        State {
            input: Box::pin(futures::stream::empty()),
            buffer: vec![],
            queue: VecDeque::new(),
            platform: p,
            identity: None,
            id: "test".into(),
            ended: false,
            finished: false,
            answered: false,
            shape_logged: false,
            upstream_request_id: None,
            minimax_tools: Default::default(),
            minimax_tool_count: 0,
            minimax_usage: json!({}),
            minimax_finish: None,
        }
    }
    #[test]
    fn trae_tools_and_identity_are_preserved() {
        let mut s = state(Platform::Traework);
        s.parse(
            br#"event: metadata
data: {"model":"glm-5.3"}"#,
        );
        s.parse(
            br#"event: output
data: {"tool_calls":[{"index":0,"id":"c","function_call":{"name":"read","arguments":"{}"}}]}"#,
        );
        let c = s.queue.pop_back().unwrap().unwrap();
        assert_eq!(c.upstream_model.as_deref(), Some("glm-5.3"));
        assert_eq!(
            c.data["choices"][0]["delta"]["tool_calls"][0]["function"]["name"],
            "read"
        );
        assert_eq!(c.data["choices"][0]["delta"]["tool_calls"][0]["id"], "c");
        for id in [json!(""), Value::Null] {
            let frame = format!(
                "event: output\ndata: {}",
                json!({"tool_calls":[{"index":0,"id":id,"function_call":{"name":"","arguments":"partial"}}]})
            );
            s.parse(frame.as_bytes());
            let c = s.queue.pop_back().unwrap().unwrap();
            let call = &c.data["choices"][0]["delta"]["tool_calls"][0];
            assert!(call.get("id").is_none());
            assert_eq!(call["index"], 0);
            assert_eq!(call["function"]["arguments"], "partial");
        }
    }

    #[test]
    fn trae_deployment_statistics_do_not_replace_the_model_identity() {
        for identity in ["", "deepseek-v4.1-flash", "different-model"] {
            let mut s = state(Platform::Traework);
            for (event, data) in [
                ("metadata", json!({"model":identity})),
                (
                    "timing_cost",
                    json!({"provider_model_name":"ep-fixture-deployment"}),
                ),
                (
                    "output",
                    json!({"response":"OK","reasoning_content":null,"tool_calls":null}),
                ),
                (
                    "extra_info",
                    json!({"model":"ep-fixture-deployment","input_token":34,"version":1}),
                ),
                (
                    "token_usage",
                    json!({"prompt_tokens":34,"completion_tokens":76}),
                ),
                ("done", json!({"finish_reason":"stop"})),
            ] {
                s.parse(format!("event: {event}\ndata: {data}").as_bytes());
            }
            assert!(s.finished && s.ended);
            let chunks: Vec<_> = s.queue.into_iter().map(Result::unwrap).collect();
            for chunk in &chunks {
                assert_eq!(
                    chunk.upstream_model.as_deref(),
                    (!identity.is_empty()).then_some(identity)
                );
            }
            assert!(chunks
                .iter()
                .any(|chunk| chunk.data["choices"][0]["delta"]["content"] == "OK"));
            assert!(chunks
                .iter()
                .any(|chunk| chunk.data["usage"]["completion_tokens"] == 76));
            assert_eq!(
                chunks.last().unwrap().data["choices"][0]["finish_reason"],
                "stop"
            );
        }
    }

    #[test]
    fn qoder_envelope_rejection_is_not_a_success() {
        let mut s = state(Platform::Qoder);
        s.parse(br#"data: {"statusCodeValue":429,"body":"{}"}"#);
        assert!(s.queue.pop_front().unwrap().unwrap_err().rejected);
    }

    #[test]
    fn stream_errors_preserve_safe_metadata_and_do_not_become_retryable_after_output() {
        let mut s = state(Platform::Workbuddy);
        s.upstream_request_id = Some("req_header_123".into());
        s.answered = true;
        s.parse(br#"data: {"code":1005,"error":{"message":"fixture-secret-body"}}"#);
        let error = s.queue.pop_front().unwrap().unwrap_err();
        assert_eq!(error.upstream_code.as_deref(), Some("1005"));
        assert_eq!(error.upstream_request_id.as_deref(), Some("req_header_123"));
        assert_eq!(error.stage, "stream");
        assert!(!error.rejected);
        assert!(!serde_json::to_string(&error.diagnostic()).unwrap().contains("fixture-secret"));

        let mut s = state(Platform::Qoder);
        s.parse(br#"data: {"statusCodeValue":400,"body":{"error":{"code":"invalid_schema","message":"fixture-secret"},"request_id":"req_body_456"}}"#);
        let error = s.queue.pop_front().unwrap().unwrap_err();
        assert_eq!(error.upstream_code.as_deref(), Some("invalid_schema"));
        assert_eq!(error.upstream_request_id.as_deref(), Some("req_body_456"));
        assert!(!error.rejected);
    }
    #[test]
    fn byte_boundaries_accept_crlf() {
        assert_eq!(boundary(b"data: x\r\n\r\n"), Some((7, 4)));
    }

    #[test]
    fn minimax_anthropic_stream_preserves_thinking_tool_indices_usage_and_completion() {
        let mut s = state(Platform::MinimaxCode);
        for value in [
            json!({"type":"message_start","message":{"model":"MiniMax-M3","usage":{"input_tokens":7,"cache_read_input_tokens":10,"cache_creation_input_tokens":3}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"plan"}}),
            json!({"type":"content_block_start","index":1,"content_block":{"type":"text","text":"hello"}}),
            json!({"type":"content_block_start","index":2,"content_block":{"type":"tool_use","id":"first","name":"echo","input":{}}}),
            json!({"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"{\"value\":"}}),
            json!({"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"42}"}}),
            json!({"type":"content_block_stop","index":2}),
            json!({"type":"content_block_start","index":4,"content_block":{"type":"tool_use","id":"second","name":"noop","input":{}}}),
            json!({"type":"content_block_stop","index":4}),
            json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"input_tokens":7,"output_tokens":11}}),
        ] { s.parse(format!("data: {value}").as_bytes()); }
        assert!(!s.finished);
        s.parse(b"event: message_stop\ndata: {\"type\":\"message_stop\"}");
        assert!(s.finished && s.ended);
        let chunks: Vec<_> = s.queue.into_iter().map(Result::unwrap).collect();
        assert!(chunks.iter().all(|chunk| chunk.upstream_model.as_deref() == Some("MiniMax-M3")));
        assert!(chunks.iter().any(|chunk| chunk.data["choices"][0]["delta"]["reasoning_content"] == "plan"));
        let calls: Vec<_> = chunks.iter().filter_map(|chunk| chunk.data["choices"][0]["delta"]["tool_calls"].as_array()).flatten().collect();
        let args = |index| calls.iter().filter(|call| call["index"] == index)
            .filter_map(|call| call["function"]["arguments"].as_str()).collect::<String>();
        assert_eq!(args(0), "{\"value\":42}"); assert_eq!(args(1), "{}");
        let last = &chunks.last().unwrap().data;
        assert_eq!(last["choices"][0]["finish_reason"], "tool_calls");
        assert_eq!(last["usage"], json!({"prompt_tokens":20,"completion_tokens":11,"total_tokens":31}));
    }
    #[test]
    fn minimax_stream_errors_do_not_retry_after_output_or_accept_incomplete_endings() {
        let mut s = state(Platform::MinimaxCode);
        s.answered = true;
        s.parse(br#"data: {"type":"error","error":{"type":"rate_limit_error","message":"fixture-secret"}}"#);
        let error = s.queue.pop_front().unwrap().unwrap_err();
        assert_eq!(error.status, 429); assert!(!error.rejected); assert!(!error.message.contains("fixture-secret"));
        let mut s = state(Platform::MinimaxCode);
        s.parse(br#"data: {"type":"message_stop"}"#);
        assert!(s.queue.pop_front().unwrap().is_err()); assert!(!s.finished);
        let mut s = state(Platform::MinimaxCode); s.parse(b"data: [DONE]");
        assert!(s.queue.pop_front().unwrap().is_err()); assert!(!s.finished);
    }

    #[tokio::test]
    async fn channel_verification_rejections_report_a_safe_specific_reason() {
        use axum::{http::StatusCode, routing::post, Json, Router};
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route(
                    "/",
                    post(|Json(body): Json<Value>| async {
                        (StatusCode::BAD_REQUEST, [("x-request-id", "req_upstream_header")], Json(body))
                    }),
                ),
            )
            .await
            .unwrap();
        });
        let http = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();
        for (body, expected) in [
            (
                json!({"code":11128,"msg":"Illegal API invocation from an unapproved channel","token":"fixture-secret"}),
                "上游通道校验未通过（代码 11128），请使用官方客户端或平台授权的接口",
            ),
            (
                json!({"code":11128,"msg":"request illegal","token":"fixture-secret"}),
                "上游通道校验未通过（代码 11128），请使用官方客户端或平台授权的接口",
            ),
            (
                json!({"code":11128,"msg":"first message is not system prompt","token":"fixture-secret"}),
                "平台请求失败（HTTP 400）",
            ),
            (
                json!({"code":11158,"msg":"model [gemini-3.8-flash] is not allowed for user type [personal_free_user]","token":"fixture-secret"}),
                "当前账号类型不支持该模型，请在会话中切换其他模型",
            ),
            (
                json!({"code":11133,"msg":"fixture-secret"}),
                "平台请求失败（HTTP 400）",
            ),
        ] {
            let error = response(http.post(&url).json(&body), Platform::Workbuddy)
                .await
                .err()
                .unwrap();
            assert_eq!(error.status, 400);
            assert_eq!(error.message, expected);
            assert!(!error.rejected);
            assert!(!error.message.contains("fixture-secret"));
            assert_eq!(error.upstream_code.as_deref(), body["code"].as_i64().map(|v| v.to_string()).as_deref());
            assert_eq!(error.upstream_request_id.as_deref(), Some("req_upstream_header"));
        }
        server.abort();
    }
}
