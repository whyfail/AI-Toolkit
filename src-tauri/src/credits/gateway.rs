use super::providers::{ProviderChat, ProviderError};
use super::types::{now_ms, Account, GatewayStatus, ModelInfo, Platform, RequestSummary};
use super::CreditManager;
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures::StreamExt;
use parking_lot::Mutex;
use serde_json::{json, Map, Value};
use std::{collections::HashMap, sync::Arc, time::Instant};
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_util::sync::CancellationToken;

#[path = "anthropic.rs"]
mod anthropic;

const MAX_REQUEST_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Copy)]
enum Protocol {
    Openai,
    Messages,
}

impl Protocol {
    fn error(self, status: StatusCode, code: &str, message: &str) -> Response {
        match self {
            Self::Openai => api_error(status, code, message),
            Self::Messages => anthropic::error(status, message),
        }
    }
}

pub struct GatewayManager {
    running: Mutex<Option<RunningGateway>>,
    lifecycle: tokio::sync::Mutex<()>,
    last_error: Arc<Mutex<Option<String>>>,
}

struct RunningGateway {
    port: u16,
    cancellation: CancellationToken,
    task: JoinHandle<()>,
}

struct GatewayContext {
    manager: Arc<CreditManager>,
    key: String,
    routing: Mutex<RoutingState>,
    cancellation: CancellationToken,
}

#[derive(Default)]
struct RoutingState {
    loads: HashMap<String, usize>,
    assigned: HashMap<String, u64>,
    cooldowns: HashMap<String, i64>,
    sequence: u64,
}

#[derive(Clone)]
struct Candidate {
    account: Account,
    model: ModelInfo,
    expiry: i64,
}

impl Default for GatewayManager {
    fn default() -> Self {
        Self::new()
    }
}

impl GatewayManager {
    pub fn new() -> Self {
        Self {
            running: Mutex::new(None),
            lifecycle: tokio::sync::Mutex::new(()),
            last_error: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn start(
        &self,
        manager: Arc<CreditManager>,
        port: u16,
    ) -> Result<GatewayStatus, String> {
        let _lifecycle = self.lifecycle.lock().await;
        let already_running = self
            .running
            .lock()
            .as_ref()
            .is_some_and(|v| !v.task.is_finished());
        if already_running {
            return Ok(self.status(port));
        }
        let key = manager.gateway_key()?;
        if key.trim().is_empty() {
            return Err("本地代理密钥为空".into());
        }
        let listener = match TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await {
            Ok(listener) => listener,
            Err(error) => {
                let message = format!("无法启动本地代理端口 {port}：{error}");
                *self.last_error.lock() = Some(message.clone());
                return Err(message);
            }
        };
        let actual_port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let cancellation = CancellationToken::new();
        let context = Arc::new(GatewayContext {
            manager,
            key,
            routing: Mutex::new(RoutingState::default()),
            cancellation: cancellation.clone(),
        });
        let router = Router::new()
            .route("/health", get(|| async { Json(json!({"status": "ok"})) }))
            .route("/v1/models", get(models))
            .route("/v1/chat/completions", post(chat))
            .route("/v1/messages", post(messages))
            // Anthropic SDKs append /v1/messages to the configured base URL.
            // Accept the app's shared /v1 base URL without arbitrary path rewriting.
            .route("/v1/v1/messages", post(messages))
            .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
            .with_state(context);
        let shutdown = cancellation.clone();
        let last_error = self.last_error.clone();
        let task = tokio::spawn(async move {
            if let Err(error) = axum::serve(listener, router)
                .with_graceful_shutdown(shutdown.cancelled_owned())
                .await
            {
                *last_error.lock() = Some(format!("本地代理停止：{error}"));
                log::error!("Local credit gateway stopped: {error}");
            }
        });
        *self.running.lock() = Some(RunningGateway {
            port: actual_port,
            cancellation,
            task,
        });
        *self.last_error.lock() = None;
        Ok(self.status(actual_port))
    }

    pub async fn stop(&self) -> Result<GatewayStatus, String> {
        let _lifecycle = self.lifecycle.lock().await;
        let running = self.running.lock().take();
        let port = running.as_ref().map(|v| v.port).unwrap_or(17860);
        if let Some(mut running) = running {
            running.cancellation.cancel();
            if tokio::time::timeout(std::time::Duration::from_secs(3), &mut running.task)
                .await
                .is_err()
            {
                running.task.abort();
                let _ = running.task.await;
            }
        }
        Ok(self.status(port))
    }

    pub fn status(&self, port: u16) -> GatewayStatus {
        let running = self.running.lock();
        let active = running.as_ref().filter(|v| !v.task.is_finished());
        let port = active.map(|v| v.port).unwrap_or(port);
        GatewayStatus {
            running: active.is_some(),
            port,
            base_url: format!("http://127.0.0.1:{port}/v1"),
            last_error: self.last_error.lock().clone(),
        }
    }
}

impl Drop for GatewayManager {
    fn drop(&mut self) {
        if let Some(running) = self.running.get_mut().take() {
            running.cancellation.cancel();
            running.task.abort();
        }
    }
}

fn authorized(headers: &HeaderMap, key: &str) -> bool {
    let supplied = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    // Never log authentication headers. Compare every byte for equal-length keys.
    key_matches(supplied, key)
}

fn key_matches(supplied: &str, key: &str) -> bool {
    supplied.len() == key.len()
        && !key.is_empty()
        && supplied
            .as_bytes()
            .iter()
            .zip(key.as_bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

fn api_error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({"error": {"message": message, "type": "invalid_request_error", "code": code}})),
    )
        .into_response()
}

async fn models(State(context): State<Arc<GatewayContext>>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &context.key) {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "invalid_api_key",
            "本地代理密钥无效",
        );
    }
    let accounts = match context.manager.accounts() {
        Ok(accounts) => accounts,
        Err(_) => {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "account_store_error",
                "读取账号失败",
            )
        }
    };
    let mut entries = std::collections::BTreeMap::new();
    for account in accounts.into_iter().filter(account_eligible) {
        if let Some(snapshot) = &account.snapshot {
            for model in &snapshot.models {
                let alias = format!("{}/{}", account.platform.key(), model.id);
                entries.insert(alias.clone(), json!({"id": alias, "object": "model", "created": 0, "owned_by": account.platform.key()}));
                if let Some(canonical) = model.canonical_id.as_ref().filter(|_| model.verified) {
                    entries.insert(canonical.clone(), json!({"id": canonical, "object": "model", "created": 0, "owned_by": "auto"}));
                }
            }
        }
    }
    Json(json!({"object": "list", "data": entries.into_values().collect::<Vec<_>>()}))
        .into_response()
}

fn account_eligible(account: &Account) -> bool {
    account.enabled
        && account.status == "ready"
        && account
            .snapshot
            .as_ref()
            .is_some_and(|s| s.balance.is_none_or(|balance| balance > 0.0))
}

fn model_matches(account: &Account, model: &ModelInfo, requested: &str) -> bool {
    requested == format!("{}/{}", account.platform.key(), model.id)
        || (model.verified && model.canonical_id.as_deref() == Some(requested))
}

fn candidates(accounts: Vec<Account>, requested: &str, tools: bool, now: i64) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for account in accounts.into_iter().filter(account_eligible) {
        let Some(snapshot) = &account.snapshot else {
            continue;
        };
        let expiry = snapshot
            .packs
            .iter()
            .filter(|p| p.remaining.is_none_or(|v| v > 0.0))
            .filter_map(|p| p.expires_at)
            .filter(|expiry| *expiry > now)
            .min()
            .unwrap_or(i64::MAX);
        for model in &snapshot.models {
            if model_matches(&account, model, requested) && (!tools || model.supports_tools) {
                candidates.push(Candidate {
                    account: account.clone(),
                    model: model.clone(),
                    expiry,
                });
            }
        }
    }
    candidates
}

fn select_candidate(
    context: &GatewayContext,
    candidates: &[Candidate],
) -> Option<(Candidate, RequestGuard)> {
    let now = now_ms();
    let mut routing = context.routing.lock();
    let candidate = candidates
        .iter()
        .filter(|c| routing.cooldowns.get(&c.account.id).copied().unwrap_or(0) <= now)
        .min_by_key(|c| {
            (
                c.expiry,
                routing.loads.get(&c.account.id).copied().unwrap_or(0),
                routing.assigned.get(&c.account.id).copied().unwrap_or(0),
            )
        })?
        .clone();
    *routing
        .loads
        .entry(candidate.account.id.clone())
        .or_default() += 1;
    routing.sequence += 1;
    let sequence = routing.sequence;
    routing
        .assigned
        .insert(candidate.account.id.clone(), sequence);
    drop(routing);
    // The owning Arc is attached by the caller after selection.
    Some((candidate.clone(), RequestGuard::new(candidate.account)))
}

struct RequestGuard {
    context: Option<Arc<GatewayContext>>,
    account: Account,
    id: String,
    model: String,
    started: Instant,
    created_at: i64,
    attempted: bool,
    status: &'static str,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
}

impl RequestGuard {
    fn new(account: Account) -> Self {
        Self {
            context: None,
            account,
            id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
            model: String::new(),
            started: Instant::now(),
            created_at: now_ms(),
            attempted: false,
            status: "cancelled",
            input_tokens: None,
            output_tokens: None,
        }
    }

    fn observe(&mut self, data: &Value) {
        if let Some(usage) = data.get("usage") {
            self.input_tokens = usage
                .get("prompt_tokens")
                .and_then(Value::as_u64)
                .or(self.input_tokens);
            self.output_tokens = usage
                .get("completion_tokens")
                .and_then(Value::as_u64)
                .or(self.output_tokens);
        }
    }

    // Drop reads `status` for the request summary, but rustc's generator dataflow
    // misses that read across yield points in async-stream bodies, so writes go
    // through this method instead of triggering `unused_assignments`.
    fn set_status(&mut self, status: &'static str) {
        self.status = status;
    }
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        let Some(context) = &self.context else { return };
        let mut routing = context.routing.lock();
        if let Some(load) = routing.loads.get_mut(&self.account.id) {
            *load = load.saturating_sub(1);
        }
        drop(routing);
        if self.attempted {
            context.manager.mark_stale(&self.account.id);
            let _ = context.manager.record_request(RequestSummary {
                id: self.id.clone(),
                account_id: self.account.id.clone(),
                platform: self.account.platform,
                model: self.model.clone(),
                elapsed_ms: self.started.elapsed().as_millis() as u64,
                input_tokens: self.input_tokens,
                output_tokens: self.output_tokens,
                status: self.status.into(),
                created_at: self.created_at,
            });
        }
    }
}

fn validate_image_url(image: &Value) -> Result<(), &'static str> {
    use base64::Engine;
    let image = image.as_object().ok_or("image_url 必须是对象")?;
    let url = image
        .get("url")
        .and_then(Value::as_str)
        .ok_or("图片缺少有效 URL")?;
    if let Some(data) = url.strip_prefix("data:") {
        let (media, data) = data.split_once(',').ok_or("图片 data URL 格式无效")?;
        if !matches!(
            media,
            "image/jpeg;base64" | "image/png;base64" | "image/gif;base64" | "image/webp;base64"
        ) || data.is_empty()
            || base64::engine::general_purpose::STANDARD
                .decode(data)
                .is_err()
        {
            return Err("图片必须是 JPEG、PNG、GIF 或 WebP 的有效 base64 数据");
        }
    } else if reqwest::Url::parse(url)
        .ok()
        .is_none_or(|url| !matches!(url.scheme(), "http" | "https") || url.host_str().is_none())
    {
        return Err("图片 URL 必须使用 http 或 https");
    }
    if image
        .get("detail")
        .is_some_and(|detail| !matches!(detail.as_str(), Some("auto" | "low" | "high")))
    {
        return Err("图片 detail 仅支持 auto、low 或 high");
    }
    Ok(())
}

fn validate_request(data: &Value) -> Result<(&str, bool, bool), &'static str> {
    let object = data.as_object().ok_or("请求必须是 JSON 对象")?;
    const FIELDS: &[&str] = &[
        "model",
        "messages",
        "stream",
        "stream_options",
        "tools",
        "tool_choice",
        "parallel_tool_calls",
        "temperature",
        "top_p",
        "max_tokens",
        "max_completion_tokens",
        "stop",
        "seed",
        "presence_penalty",
        "frequency_penalty",
        "reasoning_effort",
        "response_format",
        "n",
        "user",
    ];
    if object.keys().any(|k| !FIELDS.contains(&k.as_str())) {
        return Err("请求包含不支持的字段");
    }
    let model = object
        .get("model")
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty() && v.len() <= 256)
        .ok_or("缺少有效的 model")?;
    if object.get("stream").is_some_and(|v| !v.is_boolean()) {
        return Err("stream 必须是布尔值");
    }
    if object.get("n").is_some_and(|v| v.as_u64() != Some(1)) {
        return Err("代理仅支持 n=1");
    }
    if object.contains_key("max_tokens") && object.contains_key("max_completion_tokens") {
        return Err("输出 token 上限参数不能同时设置两种格式");
    }
    for field in ["max_tokens", "max_completion_tokens"] {
        if object
            .get(field)
            .is_some_and(|v| v.as_u64().is_none_or(|n| n == 0))
        {
            return Err("输出 token 上限必须是正整数");
        }
    }
    for field in [
        "temperature",
        "top_p",
        "presence_penalty",
        "frequency_penalty",
    ] {
        if object.get(field).is_some_and(|v| !v.is_number()) {
            return Err("采样参数必须是数字");
        }
    }
    if object
        .get("parallel_tool_calls")
        .is_some_and(|v| !v.is_boolean())
    {
        return Err("parallel_tool_calls 必须是布尔值");
    }
    if object.get("stream_options").is_some_and(|v| !v.is_object()) {
        return Err("stream_options 必须是对象");
    }
    let messages = object
        .get("messages")
        .and_then(Value::as_array)
        .filter(|v| !v.is_empty() && v.len() <= 4096)
        .ok_or("messages 必须是非空数组")?;
    let mut has_tools = false;
    for message in messages {
        let message = message.as_object().ok_or("message 必须是对象")?;
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .ok_or("message 缺少 role")?;
        if !matches!(role, "system" | "developer" | "user" | "assistant" | "tool") {
            return Err("不支持的消息 role");
        }
        if role == "tool" {
            has_tools = true;
            if message
                .get("tool_call_id")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
            {
                return Err("工具结果缺少 tool_call_id");
            }
        }
        if message
            .get("reasoning_content")
            .is_some_and(|reasoning| role != "assistant" || !reasoning.is_string())
        {
            return Err("reasoning_content 仅支持 assistant 消息中的字符串");
        }
        if let Some(content) = message.get("content") {
            if !content.is_string() && !content.is_null() {
                let parts = content.as_array().ok_or("content 必须是文本或内容块数组")?;
                for part in parts {
                    match part.get("type").and_then(Value::as_str) {
                        Some("text") if part.get("text").and_then(Value::as_str).is_some() => {}
                        Some("image_url") if matches!(role, "user" | "tool") => {
                            validate_image_url(&part["image_url"])?
                        }
                        _ => return Err("内容块仅支持文本，以及 user 或 tool 消息中的图片"),
                    }
                }
            }
        } else if message.get("tool_calls").is_none() {
            return Err("message 缺少 content");
        }
        if let Some(calls) = message.get("tool_calls") {
            has_tools = true;
            let calls = calls.as_array().ok_or("tool_calls 必须是数组")?;
            if calls.iter().any(|v| {
                v.get("type").and_then(Value::as_str) != Some("function")
                    || v.get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(Value::as_str)
                        .is_none()
                    || v.get("function")
                        .and_then(|f| f.get("arguments"))
                        .and_then(Value::as_str)
                        .is_none()
            }) {
                return Err("工具调用格式无效");
            }
        }
    }
    if let Some(tools) = object.get("tools") {
        let tools = tools.as_array().ok_or("tools 必须是数组")?;
        has_tools |= !tools.is_empty();
        if tools.iter().any(|v| {
            v.get("type").and_then(Value::as_str) != Some("function")
                || v.get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty)
                || v.get("function")
                    .and_then(|f| f.get("parameters"))
                    .is_some_and(|p| !p.is_object())
        }) {
            return Err("tools 仅支持 function 工具");
        }
    }
    if let Some(choice) = object.get("tool_choice") {
        if let Some(choice) = choice.as_str() {
            if !matches!(choice, "auto" | "required" | "none") {
                return Err("tool_choice 字符串仅支持 auto、required 或 none");
            }
        } else if let Some(choice) = choice.as_object() {
            match choice.get("type").and_then(Value::as_str) {
                Some("auto" | "required" | "none") => {}
                Some("function") => {
                    let name = choice
                        .get("function")
                        .and_then(|function| function.get("name"))
                        .and_then(Value::as_str)
                        .filter(|name| !name.is_empty())
                        .ok_or("tool_choice 缺少函数名称")?;
                    if !object
                        .get("tools")
                        .and_then(Value::as_array)
                        .is_some_and(|tools| {
                            tools.iter().any(|tool| {
                                tool.pointer("/function/name").and_then(Value::as_str) == Some(name)
                            })
                        })
                    {
                        return Err("tool_choice 指定的函数不在 tools 中");
                    }
                }
                _ => return Err("tool_choice 格式无效"),
            }
        } else {
            return Err("tool_choice 必须是字符串或对象");
        }
    }
    Ok((
        model,
        object
            .get("stream")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        has_tools,
    ))
}

async fn chat(
    State(context): State<Arc<GatewayContext>>,
    headers: HeaderMap,
    body: Result<Json<Value>, axum::extract::rejection::JsonRejection>,
) -> Response {
    protocol_request(context, headers, body, Protocol::Openai).await
}

async fn messages(
    State(context): State<Arc<GatewayContext>>,
    headers: HeaderMap,
    body: Result<Json<Value>, axum::extract::rejection::JsonRejection>,
) -> Response {
    protocol_request(context, headers, body, Protocol::Messages).await
}

async fn protocol_request(
    context: Arc<GatewayContext>,
    headers: HeaderMap,
    body: Result<Json<Value>, axum::extract::rejection::JsonRejection>,
    protocol: Protocol,
) -> Response {
    let authenticated = authorized(&headers, &context.key)
        || (matches!(protocol, Protocol::Messages)
            && headers
                .get("x-api-key")
                .and_then(|key| key.to_str().ok())
                .is_some_and(|key| key_matches(key, &context.key)));
    if !authenticated {
        return protocol.error(
            StatusCode::UNAUTHORIZED,
            "invalid_api_key",
            "本地代理密钥无效",
        );
    }
    let data = match body {
        Ok(Json(data)) => data,
        Err(error) => {
            let (status, code, message) = match error.status() {
                StatusCode::PAYLOAD_TOO_LARGE => (
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "request_too_large",
                    "请求正文超过 32 MiB 上限；请压缩图片或精简历史消息",
                ),
                StatusCode::UNSUPPORTED_MEDIA_TYPE => (
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "invalid_content_type",
                    "Content-Type 必须为 application/json",
                ),
                StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => (
                    StatusCode::BAD_REQUEST,
                    "invalid_json",
                    "请求 JSON 无效",
                ),
                status => (status, "request_body_error", "读取请求正文失败"),
            };
            return protocol.error(status, code, message);
        }
    };
    let data = if matches!(protocol, Protocol::Messages) {
        match anthropic::request(data) {
            Ok(data) => data,
            Err(message) => {
                return protocol.error(StatusCode::BAD_REQUEST, "invalid_request", message)
            }
        }
    } else {
        data
    };
    let (requested, stream, tools) = match validate_request(&data) {
        Ok(v) => v,
        Err(message) => return protocol.error(StatusCode::BAD_REQUEST, "invalid_request", message),
    };
    let requested = requested.to_owned();
    let accounts = match context.manager.accounts() {
        Ok(v) => v,
        Err(_) => {
            return protocol.error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "account_store_error",
                "读取账号失败",
            )
        }
    };
    let all = candidates(accounts.clone(), &requested, false, now_ms());
    if all.is_empty() {
        return protocol.error(
            StatusCode::NOT_FOUND,
            "model_not_available",
            "请求的精确模型没有可用账号",
        );
    }
    let mut remaining = candidates(accounts, &requested, tools, now_ms());
    if remaining.is_empty() {
        return protocol.error(
            StatusCode::BAD_REQUEST,
            "tools_not_supported",
            "该模型当前可用通道不支持工具调用",
        );
    }
    let requested_output = data
        .get("max_tokens")
        .and_then(Value::as_u64)
        .or_else(|| data.get("max_completion_tokens").and_then(Value::as_u64));
    remaining.retain(|candidate| {
        requested_output.is_none_or(|limit| {
            candidate
                .model
                .context_window
                .is_none_or(|context| limit <= context)
        })
    });
    if remaining.is_empty() {
        return protocol.error(
            StatusCode::BAD_REQUEST,
            "context_limit_exceeded",
            "输出 token 上限超过该模型已知的上下文窗口",
        );
    }
    let mut last_status = StatusCode::SERVICE_UNAVAILABLE;
    while let Some((candidate, mut guard)) = select_candidate(&context, &remaining) {
        remaining.retain(|v| v.account.id != candidate.account.id);
        guard.context = Some(context.clone());
        guard.model = requested.clone();
        let credentials = tokio::select! {
            _ = context.cancellation.cancelled() => return protocol.error(StatusCode::SERVICE_UNAVAILABLE, "gateway_stopped", "本地代理已停止"),
            result = context.manager.credentials_for(&candidate.account) => match result { Ok(v) => v, Err(_) => { guard.set_status("error"); continue; } },
        };
        let mut request = data.clone();
        request["model"] = Value::String(candidate.model.id.clone());
        request["stream"] = Value::Bool(true);
        guard.attempted = true;
        let mut upstream = tokio::select! {
            _ = context.cancellation.cancelled() => return protocol.error(StatusCode::SERVICE_UNAVAILABLE, "gateway_stopped", "本地代理已停止"),
            result = context.manager.provider.chat(&candidate.account, &credentials, &candidate.model, request.clone()) => result,
        };
        if let Ok(response) = upstream {
            upstream = prime_response(response, &context, &candidate).await;
        }
        // A confirmed authentication rejection happened before generation, so one refresh
        // and retry is safe. Timeouts, 5xx and stream failures never take this path.
        if upstream
            .as_ref()
            .err()
            .is_some_and(|error| error.status == 401 && error.rejected)
        {
            let refreshed = tokio::select! {
                _ = context.cancellation.cancelled() => return protocol.error(StatusCode::SERVICE_UNAVAILABLE, "gateway_stopped", "本地代理已停止"),
                result = context.manager.force_refresh_credentials(&candidate.account) => result,
            };
            if let Ok(refreshed) = refreshed {
                upstream = tokio::select! {
                    _ = context.cancellation.cancelled() => return protocol.error(StatusCode::SERVICE_UNAVAILABLE, "gateway_stopped", "本地代理已停止"),
                    result = context.manager.provider.chat(&candidate.account, &refreshed, &candidate.model, request) => result,
                };
                if let Ok(response) = upstream {
                    upstream = prime_response(response, &context, &candidate).await;
                }
            }
        }
        match upstream {
            Ok(upstream) => {
                return if stream {
                    stream_response_for(context.clone(), candidate.model, upstream, guard, protocol)
                } else {
                    collect_response_for(
                        context.clone(),
                        candidate.model,
                        upstream,
                        guard,
                        protocol,
                    )
                    .await
                }
            }
            Err(error) => {
                guard.set_status("error");
                last_status = StatusCode::from_u16(error.status).unwrap_or(StatusCode::BAD_GATEWAY);
                if error.rejected && matches!(error.status, 401 | 402 | 403 | 429) {
                    context.routing.lock().cooldowns.insert(
                        candidate.account.id,
                        now_ms() + if error.status == 429 { 30_000 } else { 60_000 },
                    );
                    continue;
                }
                return protocol.error(
                    StatusCode::from_u16(error.status).unwrap_or(StatusCode::BAD_GATEWAY),
                    "upstream_error",
                    "上游请求失败；未重试可能已被计费的请求",
                );
            }
        }
    }
    protocol.error(
        last_status,
        "no_available_account",
        "该精确模型的账号暂时不可用，请刷新余额或重新登录",
    )
}

async fn prime_response(
    mut upstream: ProviderChat,
    context: &GatewayContext,
    candidate: &Candidate,
) -> Result<ProviderChat, ProviderError> {
    let first = tokio::select! {
        _ = context.cancellation.cancelled() => return Err(ProviderError::new(503, "本地代理已停止")),
        next = tokio::time::timeout(std::time::Duration::from_secs(180), upstream.stream.next()) => next.map_err(|_| ProviderError::new(504, "等待平台首个有效响应超时"))?,
    };
    match first {
        Some(Ok(chunk)) => {
            if !matching_upstream(&candidate.model, chunk.upstream_model.as_deref(), candidate.account.platform) {
                let _ = context
                    .manager
                    .invalidate_model(
                        &candidate.account.id,
                        &candidate.model.id,
                        "上游返回的模型身份与所选模型不一致",
                    )
                    .await;
                return Err(ProviderError::new(502, "上游模型身份不一致"));
            }
            if chunk.data.get("error").is_some() {
                return Err(ProviderError::new(502, "上游返回错误"));
            }
            Ok(ProviderChat {
                stream: Box::pin(
                    futures::stream::once(async move { Ok(chunk) }).chain(upstream.stream),
                ),
            })
        }
        Some(Err(error)) => Err(error),
        None => Err(ProviderError::new(502, "上游未返回有效响应")),
    }
}

fn matching_upstream(model: &ModelInfo, actual: Option<&str>, platform: Platform) -> bool {
    // Qoder maps model keys server-side and always reports "auto" upstream, so
    // identity confirmation is impossible by design; its catalog is trusted.
    platform == Platform::Qoder
        || actual.is_none_or(|actual| super::providers::identity_matches(model, actual))
}

fn normalized_chunk(data: Value, guard: &RequestGuard) -> Value {
    let mut output = json!({"id": guard.id, "object": "chat.completion.chunk", "created": guard.created_at / 1000, "model": guard.model, "choices": data.get("choices").cloned().unwrap_or_else(|| json!([]))});
    if let Some(usage) = data.get("usage") {
        output["usage"] = usage.clone();
    }
    output
}

fn stream_response_for(
    context: Arc<GatewayContext>,
    model: ModelInfo,
    mut upstream: ProviderChat,
    mut guard: RequestGuard,
    protocol: Protocol,
) -> Response {
    let stream = async_stream::stream! {
        let mut messages = anthropic::MessageStream::default();
        if matches!(protocol, Protocol::Messages) {
            yield Ok::<_, std::convert::Infallible>(messages.start(&guard));
        }
        loop {
            let next = tokio::select! {
                _ = context.cancellation.cancelled() => {
                    if matches!(protocol, Protocol::Messages) {
                        yield Ok(anthropic::stream_error("本地代理已停止"));
                    }
                    break;
                },
                next = upstream.stream.next() => next,
            };
            match next {
                Some(Ok(chunk)) if matching_upstream(&model, chunk.upstream_model.as_deref(), guard.account.platform) && chunk.data.get("error").is_none() => {
                    guard.observe(&chunk.data);
                    match protocol {
                        Protocol::Openai => {
                            let output = normalized_chunk(chunk.data, &guard);
                            yield Ok(format!("data: {output}\n\n"));
                        }
                        Protocol::Messages => match messages.push(&chunk.data) {
                            Ok(events) => for event in events { yield Ok(event); },
                            Err(message) => {
                                guard.set_status("error");
                                yield Ok(anthropic::stream_error(message));
                                break;
                            }
                        },
                    }
                }
                Some(Ok(chunk)) => {
                    if !matching_upstream(&model, chunk.upstream_model.as_deref(), guard.account.platform) {
                        let _ = context.manager.invalidate_model(&guard.account.id, &model.id, "上游返回的模型身份与所选模型不一致").await;
                    }
                    guard.set_status("error");
                    yield Ok(match protocol {
                        Protocol::Openai => "data: {\"error\":{\"message\":\"上游流中断或模型不匹配\",\"code\":\"upstream_stream_error\"}}\n\n".to_string(),
                        Protocol::Messages => anthropic::stream_error("上游流中断或模型不匹配"),
                    });
                    break;
                }
                Some(Err(_)) => {
                    guard.set_status("error");
                    yield Ok(match protocol {
                        Protocol::Openai => "data: {\"error\":{\"message\":\"上游流中断或模型不匹配\",\"code\":\"upstream_stream_error\"}}\n\n".to_string(),
                        Protocol::Messages => anthropic::stream_error("上游流中断或模型不匹配"),
                    });
                    break;
                }
                None => {
                    match protocol {
                        Protocol::Openai => {
                            guard.set_status("success");
                            yield Ok("data: [DONE]\n\n".to_string());
                        }
                        Protocol::Messages => match messages.finish() {
                            Ok(events) => {
                                guard.set_status("success");
                                for event in events { yield Ok(event); }
                            }
                            Err(message) => {
                                guard.set_status("error");
                                yield Ok(anthropic::stream_error(message));
                            }
                        },
                    }
                    break;
                }
            }
        }
    };
    (
        [
            (header::CONTENT_TYPE, "text/event-stream"),
            (header::CACHE_CONTROL, "no-cache"),
            (header::CONNECTION, "keep-alive"),
        ],
        Body::from_stream(stream),
    )
        .into_response()
}

#[derive(Default)]
struct Completion {
    content: String,
    reasoning: String,
    tools: std::collections::BTreeMap<u64, Value>,
    finish: Option<Value>,
    usage: Option<Value>,
}

impl Completion {
    fn push(&mut self, data: &Value) {
        if let Some(usage) = data.get("usage") {
            self.usage = Some(usage.clone());
        }
        let Some(choice) = data
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|v| v.first())
        else {
            return;
        };
        if let Some(finish) = choice.get("finish_reason").filter(|v| !v.is_null()) {
            self.finish = Some(finish.clone());
        }
        let Some(delta) = choice.get("delta").or_else(|| choice.get("message")) else {
            return;
        };
        if let Some(text) = delta.get("content").and_then(Value::as_str) {
            self.content.push_str(text);
        }
        if let Some(text) = delta.get("reasoning_content").and_then(Value::as_str) {
            self.reasoning.push_str(text);
        }
        if let Some(calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for call in calls {
                let index = call.get("index").and_then(Value::as_u64).unwrap_or(0);
                let target = self.tools.entry(index).or_insert_with(|| json!({"id": "", "type": "function", "function": {"name": "", "arguments": ""}}));
                for field in ["id", "type"] {
                    if let Some(value) = call.get(field) {
                        // TRAE continuation chunks can explicitly carry an empty ID.
                        // It omits metadata for this fragment, not the existing call ID.
                        if field == "id" && (value.is_null() || value.as_str() == Some("")) {
                            continue;
                        }
                        target[field] = value.clone();
                    }
                }
                if let Some(function) = call.get("function") {
                    for field in ["name", "arguments"] {
                        if let Some(part) = function.get(field).and_then(Value::as_str) {
                            let current = target["function"][field].as_str().unwrap_or("");
                            target["function"][field] = Value::String(format!("{current}{part}"));
                        }
                    }
                }
            }
        }
    }

    fn response(self, guard: &RequestGuard) -> Value {
        let mut message = Map::new();
        message.insert("role".into(), json!("assistant"));
        message.insert(
            "content".into(),
            if self.content.is_empty() && !self.tools.is_empty() {
                Value::Null
            } else {
                json!(self.content)
            },
        );
        if !self.reasoning.is_empty() {
            message.insert("reasoning_content".into(), json!(self.reasoning));
        }
        if !self.tools.is_empty() {
            message.insert(
                "tool_calls".into(),
                json!(self.tools.into_values().collect::<Vec<_>>()),
            );
        }
        let mut response = json!({"id": guard.id, "object": "chat.completion", "created": guard.created_at / 1000, "model": guard.model, "choices": [{"index": 0, "message": message, "finish_reason": self.finish.unwrap_or_else(|| json!("stop"))}]});
        if let Some(usage) = self.usage {
            response["usage"] = usage;
        }
        response
    }
}

#[cfg(test)]
async fn collect_response(
    context: Arc<GatewayContext>,
    model: ModelInfo,
    upstream: ProviderChat,
    guard: RequestGuard,
) -> Response {
    collect_response_for(context, model, upstream, guard, Protocol::Openai).await
}

async fn collect_response_for(
    context: Arc<GatewayContext>,
    model: ModelInfo,
    mut upstream: ProviderChat,
    mut guard: RequestGuard,
    protocol: Protocol,
) -> Response {
    let mut completion = Completion::default();
    let mut received_bytes = 0usize;
    loop {
        let next = tokio::select! { _ = context.cancellation.cancelled() => return protocol.error(StatusCode::SERVICE_UNAVAILABLE, "gateway_stopped", "本地代理已停止"), next = upstream.stream.next() => next };
        match next {
            Some(Ok(chunk))
                if matching_upstream(&model, chunk.upstream_model.as_deref(), guard.account.platform)
                    && chunk.data.get("error").is_none() =>
            {
                received_bytes += chunk.data.to_string().len();
                if received_bytes > 16 * 1024 * 1024 {
                    guard.set_status("error");
                    return protocol.error(
                        StatusCode::BAD_GATEWAY,
                        "response_too_large",
                        "上游非流式响应超过内存保护上限",
                    );
                }
                guard.observe(&chunk.data);
                completion.push(&chunk.data);
            }
            Some(Ok(chunk)) => {
                if !matching_upstream(&model, chunk.upstream_model.as_deref(), guard.account.platform) {
                    let _ = context
                        .manager
                        .invalidate_model(
                            &guard.account.id,
                            &model.id,
                            "上游返回的模型身份与所选模型不一致",
                        )
                        .await;
                }
                guard.set_status("error");
                return protocol.error(
                    StatusCode::BAD_GATEWAY,
                    "upstream_model_mismatch",
                    "上游返回错误或模型版本不匹配",
                );
            }
            Some(Err(error)) => {
                guard.set_status("error");
                return protocol.error(
                    StatusCode::from_u16(error.status).unwrap_or(StatusCode::BAD_GATEWAY),
                    "upstream_error",
                    "上游请求失败；未重试可能已被计费的请求",
                );
            }
            None => {
                let output = match protocol {
                    Protocol::Openai => completion.response(&guard),
                    Protocol::Messages => match anthropic::response(completion, &guard) {
                        Ok(response) => response,
                        Err(message) => {
                            guard.set_status("error");
                            return protocol.error(
                                StatusCode::BAD_GATEWAY,
                                "invalid_upstream_response",
                                message,
                            );
                        }
                    },
                };
                guard.set_status("success");
                return Json(output).into_response();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::{CreditPack, Platform, ProviderSnapshot};
    use super::*;

    fn account(id: &str, canonical: &str, expiry: i64) -> Account {
        Account {
            id: id.into(),
            platform: Platform::Workbuddy,
            user_id: id.into(),
            label: id.into(),
            enabled: true,
            status: "ready".into(),
            last_error: None,
            snapshot: Some(ProviderSnapshot {
                balance: Some(100.0),
                unit: "credits".into(),
                quota: None,
                packs: vec![CreditPack {
                    id: "pack".into(),
                    name: "pack".into(),
                    remaining: Some(100.0),
                    expires_at: Some(expiry),
                }],
                rewards: vec![],
                models: vec![ModelInfo {
                    upstream_id: None,
                    id: canonical.into(),
                    name: canonical.into(),
                    canonical_id: Some(canonical.into()),
                    supports_tools: true,
                    context_window: None,
                    verified: true,
                    verification_note: None,
                }],
                fetched_at: now_ms(),
                stale: false,
                diagnostic: None,
            }),
        }
    }

    #[test]
    fn authorization_requires_exact_bearer_key() {
        let mut headers = HeaderMap::new();
        assert!(!authorized(&headers, "secret"));
        headers.insert(header::AUTHORIZATION, "Bearer secret".parse().unwrap());
        assert!(authorized(&headers, "secret"));
        assert!(!authorized(&headers, "secreu"));
        assert!(!authorized(&headers, ""));
    }

    #[test]
    fn validation_rejects_endpoint_injection_and_validates_image_inputs() {
        let valid = json!({"model": "glm-5.3", "messages": [{"role": "user", "content": "hello"}], "stream": true});
        assert_eq!(validate_request(&valid).unwrap(), ("glm-5.3", true, false));
        let mut injected = valid.clone();
        injected["base_url"] = json!("https://attacker.invalid");
        assert!(validate_request(&injected).is_err());
        let mut image = valid;
        image["messages"][0]["content"] =
            json!([{"type": "image_url", "image_url": {"url": "https://example.invalid/image"}}]);
        assert!(validate_request(&image).is_ok());
        image["messages"][0]["content"][0]["image_url"]["url"] = json!("file:///tmp/image.png");
        assert!(validate_request(&image).is_err());
        image["messages"][0]["content"][0]["image_url"]["url"] =
            json!("data:image/png;base64,invalid base64");
        assert!(validate_request(&image).is_err());
    }

    #[test]
    fn tool_choice_requires_an_available_named_function() {
        let mut request = json!({"model":"workbuddy/glm-5.3","messages":[{"role":"user","content":"hello"}],
            "tools":[{"type":"function","function":{"name":"read_file","parameters":{"type":"object"}}}],
            "tool_choice":{"type":"function","function":{"name":"read_file"}}});
        assert!(validate_request(&request).is_ok());
        request["tool_choice"]["function"]["name"] = json!("missing");
        assert!(validate_request(&request).is_err());
        request["tool_choice"] = json!({"type":"unsupported"});
        assert!(validate_request(&request).is_err());
        request["tool_choice"] = json!("none");
        assert!(validate_request(&request).is_ok());
    }

    #[test]
    fn platform_models_are_callable_while_cross_platform_aliases_require_verification() {
        let first = account("first", "glm-5.3", now_ms() + 10000);
        let mut second = account("second", "glm-5.2", now_ms() + 5000);
        assert_eq!(
            candidates(
                vec![first.clone(), second.clone()],
                "glm-5.3",
                false,
                now_ms()
            )
            .len(),
            1
        );
        second.snapshot.as_mut().unwrap().models[0].verified = false;
        assert!(candidates(vec![second.clone()], "glm-5.2", false, now_ms()).is_empty());
        assert_eq!(
            candidates(vec![second], "workbuddy/glm-5.2", false, now_ms()).len(),
            1
        );
        assert_eq!(
            candidates(vec![first], "workbuddy/glm-5.3", true, now_ms()).len(),
            1
        );
    }

    #[test]
    fn aggregates_fragmented_tool_calls_and_usage() {
        let mut completion = Completion::default();
        completion.push(&json!({"choices": [{"delta": {"tool_calls": [{"index": 0, "id": "call_1", "type": "function", "function": {"name": "read_file", "arguments": "{\"path\":"}}]}, "finish_reason": null}]}));
        completion.push(&json!({"choices": [{"delta": {"tool_calls": [{"index": 0, "id":"", "function": {"arguments": "\"test.txt\"}"}}]}, "finish_reason": "tool_calls"}], "usage": {"prompt_tokens": 10, "completion_tokens": 3}}));
        let mut guard = RequestGuard::new(account("first", "glm-5.3", now_ms()));
        guard.model = "glm-5.3".into();
        let response = completion.response(&guard);
        assert_eq!(
            response["choices"][0]["message"]["tool_calls"][0]["id"],
            "call_1"
        );
        assert_eq!(
            response["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"],
            "{\"path\":\"test.txt\"}"
        );
        assert_eq!(response["choices"][0]["finish_reason"], "tool_calls");
        assert_eq!(response["usage"]["prompt_tokens"], 10);
    }

    #[test]
    fn empty_tool_ids_do_not_erase_parallel_or_late_ids() {
        let mut completion = Completion::default();
        completion.push(&json!({"choices":[{"delta":{"tool_calls":[
            {"index":0,"id":"call_a","function":{"name":"read","arguments":"{}"}},
            {"index":1,"function":{"name":"read","arguments":"{}"}}
        ]}}]}));
        completion.push(&json!({"choices":[{"delta":{"tool_calls":[
            {"index":0,"id":null}, {"index":1,"id":"call_b"}
        ]}}]}));
        completion.push(&json!({"choices":[{"delta":{"tool_calls":[
            {"index":0,"id":""}, {"index":1,"id":null}
        ]},"finish_reason":"tool_calls"}]}));
        assert_eq!(completion.tools.len(), 2);
        assert_eq!(completion.tools[&0]["id"], "call_a");
        assert_eq!(completion.tools[&1]["id"], "call_b");
        assert_eq!(completion.tools[&0]["function"]["arguments"], "{}");
        assert_eq!(completion.tools[&1]["function"]["arguments"], "{}");
    }

    fn test_context() -> Arc<GatewayContext> {
        Arc::new(GatewayContext {
            manager: CreditManager::test(),
            key: "test-local-key".into(),
            routing: Mutex::new(RoutingState::default()),
            cancellation: CancellationToken::new(),
        })
    }

    #[test]
    fn routing_prioritizes_expiry_then_load_and_releases_leases() {
        let context = test_context();
        let now = now_ms();
        let entries = candidates(
            vec![
                account("first", "glm-5.3", now + 10000),
                account("second", "glm-5.3", now + 10000),
            ],
            "glm-5.3",
            false,
            now,
        );
        let (first, mut lease) = select_candidate(&context, &entries).unwrap();
        lease.context = Some(context.clone());
        let (second, mut second_lease) = select_candidate(&context, &entries).unwrap();
        second_lease.context = Some(context.clone());
        assert_ne!(first.account.id, second.account.id);
        drop(lease);
        assert_eq!(context.routing.lock().loads[&first.account.id], 0);
        let mut earlier = entries;
        earlier[0].expiry = now + 1000;
        assert_eq!(
            select_candidate(&context, &earlier).unwrap().0.account.id,
            "first"
        );
    }

    #[tokio::test]
    async fn localhost_lifecycle_authentication_and_schema_errors() {
        let manager = CreditManager::test();
        let gateway = GatewayManager::new();
        let occupied = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let occupied_port = occupied.local_addr().unwrap().port();
        assert!(gateway.start(manager.clone(), occupied_port).await.is_err());
        assert!(!gateway.status(occupied_port).running);
        assert!(gateway.status(occupied_port).last_error.is_some());
        drop(occupied);
        let status = gateway.start(manager.clone(), 0).await.unwrap();
        assert!(status.running);
        assert_ne!(status.port, 0);
        let http = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .unwrap();
        let root = format!("http://127.0.0.1:{}", status.port);
        assert_eq!(
            http.get(format!("{root}/health"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            http.get(format!("{root}/v1/models"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
        let key = manager.gateway_key().unwrap();
        let models = http
            .get(format!("{root}/v1/models"))
            .bearer_auth(&key)
            .send()
            .await
            .unwrap();
        assert_eq!(models.status(), StatusCode::OK);
        assert_eq!(models.json::<Value>().await.unwrap()["data"], json!([]));
        let unknown = http.post(format!("{root}/v1/chat/completions")).bearer_auth(&key)
            .json(&json!({"model": "glm-unknown", "messages": [{"role": "user", "content": "hello"}]})).send().await.unwrap();
        assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
        let invalid = http.post(format!("{root}/v1/chat/completions")).bearer_auth(&key)
            .json(&json!({"model": "glm-unknown", "messages": [], "base_url": "https://invalid.test"})).send().await.unwrap();
        assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
        assert!(!gateway.stop().await.unwrap().running);
        assert!(http.get(format!("{root}/health")).send().await.is_err());
    }

    #[tokio::test]
    async fn dropping_stream_response_cancels_upstream_and_releases_account() {
        struct PendingUpstream(Arc<std::sync::atomic::AtomicBool>);
        impl futures::Stream for PendingUpstream {
            type Item = Result<super::super::providers::ChatChunk, ProviderError>;
            fn poll_next(
                self: std::pin::Pin<&mut Self>,
                _: &mut std::task::Context<'_>,
            ) -> std::task::Poll<Option<Self::Item>> {
                std::task::Poll::Pending
            }
        }
        impl Drop for PendingUpstream {
            fn drop(&mut self) {
                self.0.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
        for protocol in [Protocol::Openai, Protocol::Messages] {
            let context = test_context();
            let candidate = candidates(
                vec![account("first", "glm-5.3", now_ms() + 10000)],
                "glm-5.3",
                false,
                now_ms(),
            );
            let (candidate, mut guard) = select_candidate(&context, &candidate).unwrap();
            guard.context = Some(context.clone());
            guard.model = "glm-5.3".into();
            let dropped = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let response = stream_response_for(
                context.clone(),
                candidate.model,
                ProviderChat {
                    stream: Box::pin(PendingUpstream(dropped.clone())),
                },
                guard,
                protocol,
            );
            drop(response);
            assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
            assert_eq!(context.routing.lock().loads["first"], 0);
        }
    }

    #[tokio::test]
    async fn changed_upstream_model_revokes_verification_and_returns_error() {
        let context = test_context();
        let account = account("first", "glm-5.3", now_ms() + 10000);
        context.manager.store.save_account(&account).unwrap();
        let model = account.snapshot.as_ref().unwrap().models[0].clone();
        let mut guard = RequestGuard::new(account);
        guard.context = Some(context.clone());
        guard.model = "glm-5.3".into();
        let upstream = ProviderChat {
            stream: Box::pin(futures::stream::iter(vec![Ok(
                super::super::providers::ChatChunk {
                    data: json!({"choices": [{"delta": {"content": "wrong-model-output"}, "finish_reason": "stop"}]}),
                    upstream_model: Some("glm-5.2".into()),
                },
            )])),
        };
        let response = collect_response(context.clone(), model, upstream, guard).await;
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert!(
            !context
                .manager
                .store
                .account("first")
                .unwrap()
                .snapshot
                .unwrap()
                .models[0]
                .verified
        );
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("wrong-model-output"));
    }

    #[tokio::test]
    async fn first_stream_error_preserves_safe_rejection_without_assuming_transport_safety() {
        let context = test_context();
        let candidate = candidates(
            vec![account("first", "glm-5.3", now_ms() + 10000)],
            "glm-5.3",
            false,
            now_ms(),
        )
        .remove(0);
        for rejected in [true, false] {
            let upstream = ProviderChat {
                stream: Box::pin(futures::stream::iter(vec![Err(ProviderError {
                    status: 429,
                    message: "test rejection".into(),
                    rejected,
                })])),
            };
            let error = match prime_response(upstream, &context, &candidate).await {
                Ok(_) => panic!("error frame must be propagated before committing HTTP response"),
                Err(error) => error,
            };
            assert_eq!(error.status, 429);
            assert_eq!(error.rejected, rejected);
        }
    }

    #[tokio::test]
    async fn first_stream_content_is_preserved_in_order() {
        let context = test_context();
        let candidate = candidates(
            vec![account("first", "glm-5.3", now_ms() + 10000)],
            "glm-5.3",
            false,
            now_ms(),
        )
        .remove(0);
        let upstream = ProviderChat {
            stream: Box::pin(futures::stream::iter(["first", "second"].into_iter().map(
                |text| {
                    Ok(super::super::providers::ChatChunk {
                        data: json!({"choices": [{"delta": {"content": text}}]}),
                        upstream_model: Some("glm-5.3".into()),
                    })
                },
            ))),
        };
        let mut primed = prime_response(upstream, &context, &candidate)
            .await
            .unwrap();
        assert_eq!(
            primed.stream.next().await.unwrap().unwrap().data["choices"][0]["delta"]["content"],
            "first"
        );
        assert_eq!(
            primed.stream.next().await.unwrap().unwrap().data["choices"][0]["delta"]["content"],
            "second"
        );
    }

    fn routed_test_manager() -> Arc<CreditManager> {
        let manager = CreditManager::test();
        for (id, expiry) in [("first", 5000), ("second", 10000)] {
            let account = account(id, "glm-5.3", now_ms() + expiry);
            manager.store.save_account(&account).unwrap();
            let credentials = super::super::types::Credentials {
                access_token: "fixture-secret-only-in-memory".into(),
                refresh_token: None,
                user_id: Some(id.into()),
                device_id: None,
                expires_at: None,
                extra: json!({}),
            };
            manager
                .secrets
                .set(id, &serde_json::to_string(&credentials).unwrap())
                .unwrap();
        }
        manager.provider.queue_test_chat("second", vec![Ok(super::super::providers::ChatChunk {
            data: json!({"choices": [{"delta": {"content": "fallback reply"}, "finish_reason": "stop"}], "usage": {"prompt_tokens": 10, "completion_tokens": 3}}),
            upstream_model: Some("glm-5.3".into()),
        })]);
        manager
    }

    fn fixture_chunks(
        values: Vec<Value>,
    ) -> Vec<Result<super::super::providers::ChatChunk, ProviderError>> {
        values
            .into_iter()
            .map(|data| {
                Ok(super::super::providers::ChatChunk {
                    data,
                    upstream_model: Some("glm-5.3".into()),
                })
            })
            .collect()
    }

    fn messages_body(stream: bool) -> Value {
        json!({"model":"workbuddy/glm-5.3","max_tokens":64,"messages":[{"role":"user","content":"hello"}],"stream":stream})
    }

    fn local_http() -> reqwest::Client {
        reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .unwrap()
    }

    fn sse_events(body: &str) -> Vec<Value> {
        body.split("\n\n")
            .filter(|frame| !frame.is_empty())
            .map(|frame| {
                let mut lines = frame.lines();
                let name = lines.next().unwrap().strip_prefix("event: ").unwrap();
                let event: Value =
                    serde_json::from_str(lines.next().unwrap().strip_prefix("data: ").unwrap())
                        .unwrap();
                assert_eq!(name, event["type"].as_str().unwrap());
                event
            })
            .collect()
    }

    #[test]
    fn reasoning_history_requires_an_assistant_string() {
        let mut body = json!({"model":"glm-5.3","messages":[{"role":"assistant","content":"","reasoning_content":"history"}]});
        assert!(validate_request(&body).is_ok());
        body["messages"][0]["reasoning_content"] = json!(null);
        assert!(validate_request(&body).is_err());
        body["messages"][0]["reasoning_content"] = json!("history");
        for role in ["user", "system", "tool"] {
            body["messages"][0]["role"] = json!(role);
            body["messages"][0]["tool_call_id"] = json!("shot");
            assert!(validate_request(&body).is_err());
        }
    }

    #[tokio::test]
    async fn zcode_long_history_over_four_mib_reaches_each_provider() {
        let image_data = "AAAA".repeat(40_000);
        let mut messages = vec![json!({"role":"user","content":"review the screenshots"})];
        let mut openai = messages.clone();
        for index in 0..35 {
            let id = format!("shot_{index}");
            let reasoning = format!("inspect screenshot {index}");
            messages.push(json!({"role":"assistant","content":[
                {"type":"thinking","thinking":reasoning,"signature":""},
                {"type":"tool_use","id":id,"name":"Read","input":{}}
            ]}));
            messages.push(json!({"role":"user","content":[{"type":"tool_result","tool_use_id":id,"content":[
                {"type":"image","source":{"type":"base64","media_type":"image/png","data":image_data}}
            ]}]}));
            openai.push(
                json!({"role":"assistant","content":"","reasoning_content":reasoning,"tool_calls":[
                    {"id":id,"type":"function","function":{"name":"Read","arguments":"{}"}}
                ]}),
            );
            openai.push(json!({"role":"tool","tool_call_id":id,"content":[
                {"type":"image_url","image_url":{"url":format!("data:image/png;base64,{image_data}")}}
            ]}));
        }
        for platform in [Platform::Workbuddy, Platform::Traework, Platform::Qoder] {
            let manager = routed_test_manager();
            for mut account in manager.accounts().unwrap() {
                account.platform = platform;
                manager.store.save_account(&account).unwrap();
            }
            let gateway = GatewayManager::new();
            let status = gateway.start(manager.clone(), 0).await.unwrap();
            let http = reqwest::Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap();
            for messages_protocol in [true, false] {
                manager.provider.queue_test_chat("first", fixture_chunks(vec![
                    json!({"choices":[{"delta":{"content":"screenshots received"},"finish_reason":"stop"}]})
                ]));
                let mut body = json!({"model":format!("{}/glm-5.3",platform.key()),"max_tokens":64,"messages":if messages_protocol { &messages } else { &openai }});
                if messages_protocol {
                    body["stream"] = json!(true);
                }
                let bytes = serde_json::to_vec(&body).unwrap();
                assert!(bytes.len() > 4 * 1024 * 1024);
                let response = http
                    .post(format!(
                        "{}/{}",
                        status.base_url,
                        if messages_protocol {
                            "messages"
                        } else {
                            "chat/completions"
                        }
                    ))
                    .bearer_auth(manager.gateway_key().unwrap())
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(bytes)
                    .send()
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::OK);
                if messages_protocol {
                    assert_eq!(
                        sse_events(&response.text().await.unwrap()).last().unwrap()["type"],
                        "message_stop"
                    );
                } else {
                    assert_eq!(
                        response.json::<Value>().await.unwrap()["choices"][0]["message"]["content"],
                        "screenshots received"
                    );
                }
            }
            for body in manager.provider.test_chat_bodies() {
                assert_eq!(body["messages"].as_array().unwrap().len(), 71);
                for index in 0..35 {
                    let assistant = &body["messages"][1 + 2 * index];
                    let result = &body["messages"][2 + 2 * index];
                    assert_eq!(
                        assistant["reasoning_content"],
                        format!("inspect screenshot {index}")
                    );
                    assert_eq!(assistant["content"], "");
                    assert_eq!(result["tool_call_id"], format!("shot_{index}"));
                    let url = result["content"][0]["image_url"]["url"].as_str().unwrap();
                    assert!(url.starts_with("data:image/png;base64,"));
                    assert_eq!(url.len(), "data:image/png;base64,".len() + image_data.len());
                }
            }
            gateway.stop().await.unwrap();
        }
    }

    #[tokio::test]
    async fn request_body_rejections_distinguish_size_json_and_content_type() {
        let manager = routed_test_manager();
        let gateway = GatewayManager::new();
        let status = gateway.start(manager.clone(), 0).await.unwrap();
        let key = manager.gateway_key().unwrap();
        let http = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap();
        let mut oversized = vec![b' '; 32 * 1024 * 1024 + 1];
        oversized[..2].copy_from_slice(b"{}");
        let oversized = bytes::Bytes::from(oversized);
        for (path, messages_protocol) in [
            ("messages", true),
            ("v1/messages", true),
            ("chat/completions", false),
        ] {
            let url = format!("{}/{path}", status.base_url);
            for chunked in [false, true] {
                let body = if chunked {
                    reqwest::Body::wrap_stream(futures::stream::iter([Ok::<_, std::io::Error>(
                        oversized.clone(),
                    )]))
                } else {
                    oversized.clone().into()
                };
                let response = http
                    .post(&url)
                    .bearer_auth(&key)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(body)
                    .send()
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
                let error = response.json::<Value>().await.unwrap();
                assert_eq!(
                    error["error"][if messages_protocol { "type" } else { "code" }],
                    "request_too_large"
                );
                assert!(error["error"]["message"]
                    .as_str()
                    .unwrap()
                    .contains("32 MiB"));
            }
            let response = http
                .post(&url)
                .bearer_auth(&key)
                .header(header::CONTENT_TYPE, "application/json")
                .body("{")
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let error = response.json::<Value>().await.unwrap();
            assert_eq!(error["error"]["message"], "请求 JSON 无效");
            let response = http
                .post(&url)
                .bearer_auth(&key)
                .body("{}")
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
            assert!(response.json::<Value>().await.unwrap()["error"]["message"]
                .as_str()
                .unwrap()
                .contains("application/json"));
        }
        assert!(manager.provider.test_chat_calls().is_empty());
        gateway.stop().await.unwrap();
    }

    #[tokio::test]
    async fn messages_images_reach_the_provider_in_user_messages_and_tool_results() {
        for platform in [Platform::Workbuddy, Platform::Traework, Platform::Qoder] {
            let manager = routed_test_manager();
            for mut account in manager.accounts().unwrap() {
                account.platform = platform;
                manager.store.save_account(&account).unwrap();
            }
            let requested = format!("{}/glm-5.3", platform.key());
            for _ in 0..2 {
                manager.provider.queue_test_chat(
                    "first",
                    fixture_chunks(vec![
                        json!({"choices":[{"delta":{"content":"image reply"},"finish_reason":"stop"}]}),
                    ]),
                );
            }
            let gateway = GatewayManager::new();
            let status = gateway.start(manager.clone(), 0).await.unwrap();
            let http = local_http();
            let key = manager.gateway_key().unwrap();
            let image = json!({"type":"image","source":{"type":"base64","media_type":"image/jpeg","data":"aW1hZ2U="}});
            for (stream, messages) in [false, true].into_iter().zip([
                json!([{"role":"user","content":[{"type":"text","text":"check this image"},image.clone()]}]),
                json!([
                    {"role":"user","content":"read the file"},
                    {"role":"assistant","content":[{"type":"tool_use","id":"call_1","name":"Read","input":{"path":"image.jpg"}}]},
                    {"role":"user","content":[{"type":"tool_result","tool_use_id":"call_1","content":[image]}]},
                ]),
            ]) {
                let response = http
                    .post(format!("{}/messages", status.base_url))
                    .header("x-api-key", &key)
                    .json(&json!({"model":requested,"max_tokens":128000,"messages":messages,"thinking":{"type":"enabled"},"output_config":{"effort":"max"},"stream":stream}))
                    .send()
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::OK);
                if stream {
                    let events = sse_events(&response.text().await.unwrap());
                    assert_eq!(events.last().unwrap()["type"], "message_stop");
                    assert!(events.iter().any(|event| event["delta"]["text"] == "image reply"));
                } else {
                    let message = response.json::<Value>().await.unwrap();
                    assert_eq!(message["content"][0]["text"], "image reply");
                }
            }
            assert_eq!(manager.provider.test_chat_calls(), vec!["first", "first"]);
            let bodies = manager.provider.test_chat_bodies();
            let expected =
                json!({"type":"image_url","image_url":{"url":"data:image/jpeg;base64,aW1hZ2U="}});
            assert_eq!(bodies[0]["messages"][0]["content"][1], expected);
            assert_eq!(bodies[1]["messages"][2]["content"][0], expected);
            assert_eq!(bodies[1]["messages"][2]["tool_call_id"], "call_1");
            gateway.stop().await.unwrap();
        }
    }

    #[tokio::test]
    async fn messages_http_auth_validation_and_sdk_base_url_alias() {
        let manager = routed_test_manager();
        manager.provider.queue_test_chat("first", fixture_chunks(vec![json!({"choices":[{"delta":{"content":"Messages reply","reasoning_content":"provider-only reasoning"},"finish_reason":"length"}],"usage":{"prompt_tokens":12,"completion_tokens":7}})]));
        manager.provider.queue_test_chat(
            "first",
            fixture_chunks(vec![
                json!({"choices":[{"delta":{"content":"Bearer reply"},"finish_reason":"stop"}]} ),
            ]),
        );
        let gateway = GatewayManager::new();
        let status = gateway.start(manager.clone(), 0).await.unwrap();
        let http = local_http();
        let key = manager.gateway_key().unwrap();
        let url = format!("{}/messages", status.base_url);
        let malformed = http
            .post(&url)
            .header("x-api-key", &key)
            .header(header::CONTENT_TYPE, "application/json")
            .body("{")
            .send()
            .await
            .unwrap();
        assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            malformed.json::<Value>().await.unwrap()["error"]["type"],
            "invalid_request_error"
        );
        let unauthorized = http
            .post(&url)
            .header("x-api-key", "wrong")
            .json(&messages_body(false))
            .send()
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            unauthorized.json::<Value>().await.unwrap()["error"]["type"],
            "authentication_error"
        );
        let mut invalid = messages_body(false);
        for (field, value) in [
            ("top_k", json!(5)),
            ("stop_sequences", json!(["END"])),
        ] {
            invalid[field] = value;
            let response = http
                .post(&url)
                .header("x-api-key", &key)
                .json(&invalid)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let error = response.json::<Value>().await.unwrap();
            assert_eq!(error["type"], "error");
            assert_eq!(error["error"]["type"], "invalid_request_error");
            invalid.as_object_mut().unwrap().remove(field);
        }
        invalid["messages"][0]["content"] =
            json!([{"type":"image","source":{"type":"url","url":"file:///tmp/a.png"}}]);
        let response = http
            .post(&url)
            .header("x-api-key", &key)
            .json(&invalid)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(manager.provider.test_chat_calls().is_empty());
        let response = http
            .post(format!("{}/v1/messages", status.base_url))
            .header("x-api-key", &key)
            .header("anthropic-version", "2023-06-01")
            .json(&messages_body(false))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let message = response.json::<Value>().await.unwrap();
        assert_eq!(message["type"], "message");
        assert!(message["id"].as_str().unwrap().starts_with("msg_"));
        assert_eq!(message["model"], "workbuddy/glm-5.3");
        assert_eq!(
            message["content"],
            json!([{"type":"text","text":"Messages reply"}])
        );
        assert_eq!(message["stop_reason"], "max_tokens");
        assert_eq!(
            message["usage"],
            json!({"input_tokens":12,"output_tokens":7})
        );
        let response = http
            .post(&url)
            .bearer_auth(&key)
            .json(&messages_body(false))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.json::<Value>().await.unwrap()["content"][0]["text"],
            "Bearer reply"
        );
        // Claude-Code-style clients always send a reasoning configuration; the
        // hint must be accepted (and dropped) instead of failing the request.
        manager.provider.queue_test_chat("first", fixture_chunks(vec![json!({"choices":[{"delta":{"content":"thinking reply"},"finish_reason":"stop"}]})]));
        let response = http
            .post(&url)
            .header("x-api-key", &key)
            .json(&json!({"model":"workbuddy/glm-5.3","max_tokens":64,"messages":[{"role":"user","content":"hello"}],"thinking":{"type":"enabled"},"output_config":{"effort":"max"}}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.json::<Value>().await.unwrap()["content"][0]["text"],
            "thinking reply"
        );
        // Existing OpenAI callers continue to authenticate with Bearer only.
        let response = http.post(format!("{}/chat/completions", status.base_url)).header("x-api-key", &key).json(&json!({"model":"workbuddy/glm-5.3","messages":[{"role":"user","content":"hello"}]})).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        gateway.stop().await.unwrap();
    }

    #[tokio::test]
    async fn messages_http_tool_roundtrip_and_sse_event_contract() {
        let manager = routed_test_manager();
        let chunks = vec![
            json!({"choices":[{"delta":{"content":"I will check. "}}]}),
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"read_","arguments":"{\"path\":"}}]}}]}),
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"","function":{"name":"file","arguments":"\"a.txt\"}"}}]},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":6}}),
        ];
        manager
            .provider
            .queue_test_chat("first", fixture_chunks(chunks.clone()));
        manager.provider.queue_test_chat("first", fixture_chunks(vec![json!({"choices":[{"delta":{"content":"after tool result"},"finish_reason":"stop"}]} )]));
        let gateway = GatewayManager::new();
        let status = gateway.start(manager.clone(), 0).await.unwrap();
        let http = local_http();
        let key = manager.gateway_key().unwrap();
        let url = format!("{}/messages", status.base_url);
        let mut body = messages_body(true);
        body["tools"] = json!([{"name":"read_file","input_schema":{"type":"object","properties":{"path":{"type":"string"}}},"cache_control":{"type":"ephemeral"}}]);
        let response = http
            .post(&url)
            .header("x-api-key", &key)
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/event-stream"
        );
        let stream = response.text().await.unwrap();
        assert!(!stream.contains("[DONE]"));
        assert!(!stream.contains("signature"));
        let events = sse_events(&stream);
        let kinds: Vec<_> = events
            .iter()
            .map(|event| event["type"].as_str().unwrap())
            .collect();
        assert_eq!(
            kinds,
            vec![
                "message_start",
                "content_block_start",
                "content_block_delta",
                "content_block_stop",
                "content_block_start",
                "content_block_delta",
                "content_block_stop",
                "message_delta",
                "message_stop"
            ]
        );
        assert_eq!(
            events[2]["delta"],
            json!({"type":"text_delta","text":"I will check. "})
        );
        assert_eq!(events[4]["index"], 1);
        assert_eq!(
            events[4]["content_block"],
            json!({"type":"tool_use","id":"call_1","name":"read_file","input":{}})
        );
        let input: Value =
            serde_json::from_str(events[5]["delta"]["partial_json"].as_str().unwrap()).unwrap();
        assert_eq!(input, json!({"path":"a.txt"}));
        assert_eq!(events[7]["delta"]["stop_reason"], "tool_use");
        assert_eq!(
            events[7]["usage"],
            json!({"input_tokens":10,"output_tokens":6})
        );
        body["stream"] = json!(false);
        body["messages"] = json!([
            {"role":"user","content":"hello"},
            {"role":"assistant","content":[{"type":"text","text":"I will check. "},{"type":"tool_use","id":"call_1","name":"read_file","input":input}]},
            {"role":"user","content":[{"type":"tool_result","tool_use_id":"call_1","content":"file contents"}]},
        ]);
        let response = http
            .post(&url)
            .bearer_auth(&key)
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.json::<Value>().await.unwrap()["stop_reason"],
            "end_turn"
        );
        manager
            .provider
            .queue_test_chat("first", fixture_chunks(chunks));
        body["messages"] = messages_body(false)["messages"].clone();
        let response = http
            .post(&url)
            .header("x-api-key", &key)
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let message = response.json::<Value>().await.unwrap();
        assert_eq!(message["stop_reason"], "tool_use");
        assert_eq!(
            message["content"][1],
            json!({"type":"tool_use","id":"call_1","name":"read_file","input":{"path":"a.txt"}})
        );
        gateway.stop().await.unwrap();
    }

    #[tokio::test]
    async fn messages_stream_errors_do_not_finish_or_retry_committed_content() {
        let manager = routed_test_manager();
        let mut chunks = fixture_chunks(vec![json!({"choices":[{"delta":{"content":"partial"}}]})]);
        chunks.push(Err(ProviderError {
            status: 429,
            message: "fixture-only error".into(),
            rejected: true,
        }));
        manager.provider.queue_test_chat("first", chunks);
        let gateway = GatewayManager::new();
        let status = gateway.start(manager.clone(), 0).await.unwrap();
        let response = local_http()
            .post(format!("{}/messages", status.base_url))
            .header("x-api-key", manager.gateway_key().unwrap())
            .json(&messages_body(true))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let events = sse_events(&response.text().await.unwrap());
        assert_eq!(events.last().unwrap()["type"], "error");
        assert_eq!(events.last().unwrap()["error"]["type"], "api_error");
        assert!(!events.iter().any(|event| event["type"] == "message_stop"));
        assert_eq!(manager.provider.test_chat_calls(), vec!["first"]);
        gateway.stop().await.unwrap();
    }

    #[tokio::test]
    async fn messages_preserves_safe_first_frame_failover_and_model_identity_check() {
        for (rejected, wrong_model) in [(true, false), (false, false), (false, true)] {
            let manager = routed_test_manager();
            let chunks = if wrong_model {
                vec![Ok(super::super::providers::ChatChunk {
                    data: json!({"choices":[{"delta":{"content":"wrong-model-content"}}]}),
                    upstream_model: Some("glm-5.2".into()),
                })]
            } else {
                vec![Err(ProviderError {
                    status: 429,
                    message: "test".into(),
                    rejected,
                })]
            };
            manager.provider.queue_test_chat("first", chunks);
            let gateway = GatewayManager::new();
            let status = gateway.start(manager.clone(), 0).await.unwrap();
            let response = local_http()
                .post(format!("{}/messages", status.base_url))
                .header("x-api-key", manager.gateway_key().unwrap())
                .json(&messages_body(true))
                .send()
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                if wrong_model {
                    StatusCode::BAD_GATEWAY
                } else if rejected {
                    StatusCode::OK
                } else {
                    StatusCode::TOO_MANY_REQUESTS
                }
            );
            let body = response.text().await.unwrap();
            assert!(!body.contains("wrong-model-content"));
            assert_eq!(
                manager.provider.test_chat_calls(),
                if rejected {
                    vec!["first", "second"]
                } else {
                    vec!["first"]
                }
            );
            if !rejected {
                let error: Value = serde_json::from_str(&body).unwrap();
                assert_eq!(error["type"], "error");
            }
            gateway.stop().await.unwrap();
        }
    }

    #[tokio::test]
    async fn messages_invalid_tool_arguments_return_an_error_in_both_modes() {
        let manager = routed_test_manager();
        for _ in 0..2 {
            manager.provider.queue_test_chat("first", fixture_chunks(vec![json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"read","arguments":"{invalid"}}]},"finish_reason":"stop"}]} )]));
        }
        let gateway = GatewayManager::new();
        let status = gateway.start(manager.clone(), 0).await.unwrap();
        for stream in [false, true] {
            let response = local_http()
                .post(format!("{}/messages", status.base_url))
                .header("x-api-key", manager.gateway_key().unwrap())
                .json(&messages_body(stream))
                .send()
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                if stream {
                    StatusCode::OK
                } else {
                    StatusCode::BAD_GATEWAY
                }
            );
            let body = response.text().await.unwrap();
            if stream {
                let events = sse_events(&body);
                assert_eq!(events.last().unwrap()["type"], "error");
                assert!(!events.iter().any(|event| event["type"] == "message_stop"
                    || event["type"] == "content_block_start"));
            } else {
                assert_eq!(
                    serde_json::from_str::<Value>(&body).unwrap()["error"]["type"],
                    "api_error"
                );
            }
        }
        assert!(manager
            .store
            .requests()
            .unwrap()
            .iter()
            .all(|row| row.status == "error"));
        gateway.stop().await.unwrap();
    }

    #[tokio::test]
    async fn messages_gateway_cancellation_sends_error_and_releases_account() {
        let context = test_context();
        let entries = candidates(
            vec![account("first", "glm-5.3", now_ms() + 10000)],
            "glm-5.3",
            false,
            now_ms(),
        );
        let (candidate, mut guard) = select_candidate(&context, &entries).unwrap();
        guard.context = Some(context.clone());
        guard.model = "glm-5.3".into();
        let response = stream_response_for(
            context.clone(),
            candidate.model,
            ProviderChat {
                stream: Box::pin(futures::stream::pending()),
            },
            guard,
            Protocol::Messages,
        );
        context.cancellation.cancel();
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let events = sse_events(std::str::from_utf8(&bytes).unwrap());
        assert_eq!(events.last().unwrap()["type"], "error");
        assert!(!events.iter().any(|event| event["type"] == "message_stop"));
        assert_eq!(context.routing.lock().loads["first"], 0);
    }

    #[tokio::test]
    async fn fresh_catalog_model_is_listed_and_can_be_called_by_platform_id() {
        let manager = routed_test_manager();
        for mut account in manager.accounts().unwrap() {
            account.snapshot.as_mut().unwrap().models[0].verified = false;
            manager.store.save_account(&account).unwrap();
        }
        manager.provider.queue_test_chat("first", vec![Ok(super::super::providers::ChatChunk {
            data: json!({"choices":[{"delta":{"content":"catalog model reply"},"finish_reason":"stop"}]}),
            upstream_model: Some("glm-5.3".into()),
        })]);
        let gateway = GatewayManager::new();
        let status = gateway.start(manager.clone(), 0).await.unwrap();
        let http = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .unwrap();
        let key = manager.gateway_key().unwrap();
        let models = http
            .get(format!("{}/models", status.base_url))
            .bearer_auth(&key)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap();
        let ids: Vec<_> = models["data"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|model| model["id"].as_str())
            .collect();
        assert_eq!(ids, vec!["workbuddy/glm-5.3"]);
        let body = json!({"model":"workbuddy/glm-5.3","messages":[{"role":"user","content":"hello"}],"stream":false});
        let response = http
            .post(format!("{}/chat/completions", status.base_url))
            .bearer_auth(&key)
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.json::<Value>().await.unwrap()["choices"][0]["message"]["content"],
            "catalog model reply"
        );
        assert_eq!(manager.provider.test_chat_calls(), vec!["first"]);
        let mut alias = body;
        alias["model"] = json!("glm-5.3");
        let response = http
            .post(format!("{}/chat/completions", status.base_url))
            .bearer_auth(&key)
            .json(&alias)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(manager.provider.test_chat_calls(), vec!["first"]);
        gateway.stop().await.unwrap();
    }

    #[tokio::test]
    async fn first_frame_rejection_fails_over_but_ambiguous_failure_does_not() {
        for rejected in [true, false] {
            let manager = routed_test_manager();
            manager.provider.queue_test_chat(
                "first",
                vec![Err(ProviderError {
                    status: 429,
                    message: "fixture rejection".into(),
                    rejected,
                })],
            );
            let gateway = GatewayManager::new();
            let status = gateway.start(manager.clone(), 0).await.unwrap();
            let response = reqwest::Client::builder().no_proxy().timeout(std::time::Duration::from_secs(3)).build().unwrap()
                .post(format!("{}/chat/completions", status.base_url)).bearer_auth(manager.gateway_key().unwrap())
                .json(&json!({"model": "glm-5.3", "messages": [{"role": "user", "content": "hello"}], "stream": false})).send().await.unwrap();
            assert_eq!(
                response.status(),
                if rejected {
                    StatusCode::OK
                } else {
                    StatusCode::TOO_MANY_REQUESTS
                }
            );
            let body = response.text().await.unwrap();
            assert!(!body.contains("fixture-secret"));
            assert_eq!(body.contains("fallback reply"), rejected);
            assert_eq!(
                manager.provider.test_chat_calls(),
                if rejected {
                    vec!["first", "second"]
                } else {
                    vec!["first"]
                }
            );
            if rejected {
                let history = manager.store.requests().unwrap();
                let success = history.iter().find(|row| row.status == "success").unwrap();
                assert_eq!(success.account_id, "second");
                assert_eq!(success.input_tokens, Some(10));
                assert_eq!(success.output_tokens, Some(3));
            }
            gateway.stop().await.unwrap();
        }
    }

    #[tokio::test]
    async fn never_switches_accounts_after_content_has_been_committed() {
        let manager = routed_test_manager();
        manager.provider.queue_test_chat(
            "first",
            vec![
                Ok(super::super::providers::ChatChunk {
                    data: json!({"choices": [{"delta": {"content": "first account reply"}}]}),
                    upstream_model: Some("glm-5.3".into()),
                }),
                Err(ProviderError {
                    status: 429,
                    message: "late rejection".into(),
                    rejected: true,
                }),
            ],
        );
        let gateway = GatewayManager::new();
        let status = gateway.start(manager.clone(), 0).await.unwrap();
        let response = reqwest::Client::builder().no_proxy().timeout(std::time::Duration::from_secs(3)).build().unwrap()
            .post(format!("{}/chat/completions", status.base_url)).bearer_auth(manager.gateway_key().unwrap())
            .json(&json!({"model": "glm-5.3", "messages": [{"role": "user", "content": "hello"}], "stream": true})).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.text().await.unwrap();
        assert!(body.contains("first account reply"));
        assert!(body.contains("upstream_stream_error"));
        assert!(!body.contains("fallback reply"));
        assert!(!body.contains("[DONE]"));
        assert_eq!(manager.provider.test_chat_calls(), vec!["first"]);
        gateway.stop().await.unwrap();
    }
}
