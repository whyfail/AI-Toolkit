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
        // TODO: remove once the per-platform upstream failures are verified live.
        let status = r.status();
        let body = r.text().await.unwrap_or_default();
        eprintln!(
            "credits: 上游 {:?} {} → {}",
            p,
            status,
            body.chars().take(300).collect::<String>()
        );
        return Err(http_error(status.as_u16()));
    }
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
                                Err(ProviderError::new(502, "平台 SSE 帧超过大小限制")),
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
                            return Some((Err(ProviderError::new(502, "平台流在完成前中断")), s));
                        }
                    }
                    _ => {
                        s.ended = true;
                        return Some((Err(ProviderError::new(502, "平台响应流中断或超时")), s));
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
    fn fail(&mut self, e: ProviderError) {
        self.queue.push_back(Err(e));
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
            self.finished = true;
            self.ended = true;
            return;
        }
        let Ok(mut v) = serde_json::from_str::<Value>(&raw) else {
            self.fail(ProviderError::new(502, "平台 SSE 数据无效"));
            return;
        };
        if self.platform == Platform::Qoder {
            if let Some(status) = v.get("statusCodeValue").and_then(Value::as_u64) {
                if status >= 400 {
                    // TODO: remove once the qoder chat failure is diagnosed live.
                    eprintln!(
                        "credits: Qoder 流错误 {status} → {}",
                        v.get("body").map(|b| b.to_string()).unwrap_or_default().chars().take(300).collect::<String>()
                    );
                    let mut e = http_error(status as u16);
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
            });
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
        shape_logged: false,}
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
    fn byte_boundaries_accept_crlf() {
        assert_eq!(boundary(b"data: x\r\n\r\n"), Some((7, 4)));
    }
}
