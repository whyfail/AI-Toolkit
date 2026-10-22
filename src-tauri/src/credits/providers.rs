//! Native CN provider adapters. Protocol references and notices are in providers/NOTICE.md.
#[path = "oauth.rs"]
mod oauth;
mod qoder;
mod qoder_identity;
mod stream;
mod trae_desktop;
mod traework;
mod workbuddy;

use super::types::*;
use futures::Stream;
pub use oauth::{OAuthPoll, OAuthStart};
use reqwest::{Client, RequestBuilder, Response};
use serde_json::{json, Value};
use std::{pin::Pin, sync::Arc, time::Duration};
pub(crate) use traework::local_credentials;
pub(super) const CHANNEL_VERIFICATION_ERROR: &str =
    "上游通道校验未通过（代码 11128），请使用官方客户端或平台授权的接口";
pub struct ProviderChat {
    pub stream: Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>,
}
#[derive(Debug)]
pub struct ChatChunk {
    pub data: Value,
    pub upstream_model: Option<String>,
}
#[derive(Clone, Debug)]
pub struct ProviderError {
    pub status: u16,
    pub message: String,
    pub rejected: bool,
}
impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for ProviderError {}
impl ProviderError {
    pub fn new(status: u16, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            rejected: false,
        }
    }
    fn transport(_: reqwest::Error) -> Self {
        Self::new(502, "平台连接失败或请求超时")
    }
}

#[derive(Clone)]
pub struct ProviderClient {
    http: Client,
    flows: Arc<tokio::sync::Mutex<std::collections::HashMap<String, oauth::LoginFlow>>>,
    #[cfg(test)]
    test_chats: Arc<std::sync::Mutex<TestChats>>,
}
#[cfg(test)]
#[derive(Default)]
struct TestChats {
    queues: std::collections::HashMap<
        String,
        std::collections::VecDeque<Vec<Result<ChatChunk, ProviderError>>>,
    >,
    calls: Vec<String>,
    bodies: Vec<Value>,
}
impl ProviderClient {
    pub fn new() -> Self {
        Self {
            http: Client::builder()
                .connect_timeout(Duration::from_secs(15))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("HTTP client"),
            flows: Arc::default(),
            #[cfg(test)]
            test_chats: Arc::default(),
        }
    }
    #[cfg(test)]
    pub fn queue_test_chat(&self, id: &str, chunks: Vec<Result<ChatChunk, ProviderError>>) {
        self.test_chats
            .lock()
            .unwrap()
            .queues
            .entry(id.into())
            .or_default()
            .push_back(chunks);
    }
    #[cfg(test)]
    pub fn test_chat_calls(&self) -> Vec<String> {
        self.test_chats.lock().unwrap().calls.clone()
    }
    #[cfg(test)]
    pub fn test_chat_bodies(&self) -> Vec<Value> {
        self.test_chats.lock().unwrap().bodies.clone()
    }
    pub async fn authenticate(
        &self,
        p: Platform,
        c: &Credentials,
    ) -> Result<(String, String, Credentials), ProviderError> {
        if c.access_token.trim().is_empty() {
            return Err(ProviderError::new(400, "凭证不能为空"));
        }
        match p {
            Platform::Workbuddy => workbuddy::authenticate(self, c).await,
            Platform::Traework => traework::authenticate(self, c).await,
            Platform::Qoder => qoder::authenticate(self, c).await,
        }
    }
    pub async fn refresh_credentials(
        &self,
        p: Platform,
        c: &Credentials,
    ) -> Result<Credentials, ProviderError> {
        match p {
            Platform::Workbuddy => workbuddy::refresh(self, c).await,
            Platform::Traework => traework::refresh(self, c).await,
            Platform::Qoder => qoder::refresh(self, c).await,
        }
    }
    pub async fn inspect(
        &self,
        a: &Account,
        c: &Credentials,
    ) -> Result<ProviderSnapshot, ProviderError> {
        match a.platform {
            Platform::Workbuddy => workbuddy::inspect(self, c).await,
            Platform::Traework => traework::inspect(self, c).await,
            Platform::Qoder => qoder::inspect(self, c).await,
        }
    }
    pub async fn claim(
        &self,
        a: &Account,
        c: &Credentials,
    ) -> Result<Vec<RewardOutcome>, ProviderError> {
        match a.platform {
            Platform::Workbuddy => workbuddy::claim(self, a, c).await,
            Platform::Traework => traework::claim(self, a, c).await,
            Platform::Qoder => qoder::claim(self, a, c).await,
        }
    }
    pub async fn chat(
        &self,
        a: &Account,
        c: &Credentials,
        m: &ModelInfo,
        body: Value,
    ) -> Result<ProviderChat, ProviderError> {
        #[cfg(test)]
        {
            let mut test = self.test_chats.lock().unwrap();
            if let Some(chunks) = test.queues.get_mut(&a.id).and_then(|q| q.pop_front()) {
                test.calls.push(a.id.clone());
                test.bodies.push(body);
                return Ok(ProviderChat {
                    stream: Box::pin(futures::stream::iter(chunks)),
                });
            }
            if std::env::var("AI_TOOLKIT_PROXY_DIAGNOSTIC").as_deref() != Ok("real-inference") {
                return Err(ProviderError::new(500, "测试上游响应队列未配置或已耗尽"));
            }
        }
        match a.platform {
            Platform::Workbuddy => workbuddy::chat(self, c, m, body).await,
            Platform::Traework => traework::chat(self, c, m, body).await,
            Platform::Qoder => qoder::chat(self, c, m, body).await,
        }
    }
    async fn json(&self, r: RequestBuilder) -> Result<Value, ProviderError> {
        json_response(
            r.timeout(Duration::from_secs(30))
                .send()
                .await
                .map_err(ProviderError::transport)?,
        )
        .await
    }
}

pub(super) async fn json_response(r: Response) -> Result<Value, ProviderError> {
    let status = r.status().as_u16();
    let text = r
        .text()
        .await
        .map_err(|_| ProviderError::new(502, "平台响应读取失败"))?;
    if !(200..300).contains(&status) {
        return Err(upstream_body_error(status, &text));
    }
    let v: Value = serde_json::from_str(&text)
        .map_err(|_| ProviderError::new(502, "平台返回了无法识别的数据"))?;
    check_business(&v)?;
    Ok(v)
}
// Volcengine-style rejections (ResponseMetadata.Error) carry the account- or
// request-state reason; the HTTP status alone only says "4xx".
fn upstream_error_detail(v: &Value) -> Option<(String, String)> {
    let error = v.pointer("/ResponseMetadata/Error")?;
    if error.is_null() {
        return None;
    }
    Some((
        text(error, &["Code"]).unwrap_or_else(|| "unknown".into()),
        error
            .get("Message")
            .and_then(Value::as_str)
            .unwrap_or("无详细信息")
            .to_string(),
    ))
}
fn upstream_body_error(status: u16, text: &str) -> ProviderError {
    let detail = serde_json::from_str::<Value>(text).ok();
    // 11128 is also used for malformed requests; match the channel rejection text too.
    if status == 400
        && detail.as_ref().is_some_and(|value| {
            value["code"].as_i64() == Some(11128)
                && value["msg"].as_str() == Some("Illegal API invocation from an unapproved channel")
        })
    {
        return ProviderError::new(status, CHANNEL_VERIFICATION_ERROR);
    }
    let mut error = http_error(status);
    if let Some((code, message)) = detail.as_ref().and_then(upstream_error_detail) {
        error.message = format!("平台拒绝（代码 {code}）：{message}");
    }
    error
}
pub(super) fn http_error(status: u16) -> ProviderError {
    ProviderError {
        status,
        message: match status {
            401 | 403 => "平台认证被拒绝，请重新登录".into(),
            402 => "平台额度不足".into(),
            429 => "平台请求限流".into(),
            _ => format!("平台请求失败（HTTP {status}）"),
        },
        rejected: matches!(status, 401 | 402 | 403 | 429),
    }
}
pub(super) fn check_business(v: &Value) -> Result<(), ProviderError> {
    if let Some(code) = num(v, &["code", "biz_code", "bizCode"]) {
        if code != 0.0 && code != 200.0 {
            let rejected = matches!(code as i64, 1001 | 1005 | 4008 | 11202 | 6004);
            return Err(ProviderError {
                status: if rejected { 429 } else { 502 },
                message: format!("平台业务请求被拒绝（代码 {}）", code as i64),
                rejected,
            });
        }
    }
    if let Some((code, message)) = upstream_error_detail(v) {
        // These session-invalid codes are handled by TRAE's packaged CheckLogin
        // client. Unknown business errors must not trigger a token refresh.
        let authentication = code.parse::<u32>().is_ok_and(|code| {
            matches!(code, 20324 | 20101 | 20315 | 20125 | 20126 | 20401 | 20403)
        });
        return Err(ProviderError {
            status: if authentication { 401 } else { 502 },
            message: if authentication {
                format!("平台认证失败（代码 {code}）：{message}")
            } else {
                format!("平台业务请求被拒绝（代码 {code}）：{message}")
            },
            rejected: authentication,
        });
    }
    if v.pointer("/Response/Error").is_some_and(|e| !e.is_null())
        || v.pointer("/data/Response/Error")
            .is_some_and(|e| !e.is_null())
    {
        return Err(ProviderError::new(502, "平台账单查询返回错误"));
    }
    Ok(())
}
pub(super) fn data(v: &Value) -> &Value {
    v.get("data").or_else(|| v.get("Result")).unwrap_or(v)
}
pub(super) fn text(v: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|k| {
            v.get(*k).and_then(|x| {
                x.as_str()
                    .map(str::to_owned)
                    .or_else(|| x.as_i64().map(|n| n.to_string()))
            })
        })
        .filter(|s| !s.is_empty())
}
pub(super) fn num(v: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|k| {
        v.get(*k).and_then(|x| {
            x.as_f64()
                .or_else(|| x.as_str().and_then(|s| s.parse().ok()))
        })
    })
}
pub(super) fn flag(v: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter().find_map(|k| {
        v.get(*k).and_then(|x| {
            x.as_bool()
                .or_else(|| x.as_str().and_then(|s| s.parse().ok()))
        })
    })
}
pub(super) fn token_limit(v: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        v.get(*key)
            .and_then(|value| {
                value
                    .as_u64()
                    .or_else(|| value.as_str().and_then(|text| text.parse::<u64>().ok()))
            })
            .filter(|limit| *limit > 0)
    })
}
pub(super) fn underlying_model_id(v: &Value, route: &str) -> Option<String> {
    let id = v.as_str()?.trim();
    // Deployment endpoints and TRAE's internal __dev names do not identify model versions.
    (!id.is_empty() && id != route && !id.starts_with("ep-") && !id.ends_with("__dev"))
        .then(|| id.to_owned())
}
pub(super) fn effort_options(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|effort| !effort.is_empty())
        .fold(Vec::new(), |mut options, effort| {
            if !options.iter().any(|known| known == effort) {
                options.push(effort.to_owned());
            }
            options
        })
}
pub(super) fn epoch(v: &Value) -> Option<i64> {
    if let Some(n) = v
        .as_i64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
    {
        return Some(if n < 100_000_000_000 { n * 1000 } else { n });
    }
    v.as_str().and_then(|s| {
        chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|d| d.timestamp_millis())
            .or_else(|| {
                chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                    .ok()
                    .and_then(|d| {
                        d.and_local_timezone(chrono::FixedOffset::east_opt(8 * 3600)?)
                            .single()
                    })
                    .map(|d| d.timestamp_millis())
            })
    })
}
pub(super) fn jwt_claims(token: &str) -> Value {
    use base64::Engine;
    token
        .split('.')
        .nth(1)
        .and_then(|s| {
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(s)
                .ok()
        })
        .and_then(|s| serde_json::from_slice(&s).ok())
        .unwrap_or(Value::Null)
}
pub(super) fn snapshot(
    packs: Vec<CreditPack>,
    rewards: Vec<Reward>,
    models: Vec<ModelInfo>,
    diagnostic: Option<String>,
) -> ProviderSnapshot {
    let balance = if packs.is_empty() || packs.iter().any(|p| p.remaining.is_none()) {
        None
    } else {
        Some(
            packs
                .iter()
                .filter(|p| p.expires_at.is_none_or(|t| t > now_ms()))
                .filter_map(|p| p.remaining)
                .sum(),
        )
    };
    ProviderSnapshot {
        balance,
        unit: "credits".into(),
        quota: None,
        packs,
        rewards,
        models,
        fetched_at: now_ms(),
        stale: false,
        diagnostic,
    }
}
pub(super) fn model(id: String, name: String, context: Option<u64>) -> ModelInfo {
    ModelInfo {
        canonical_id: canonical_glm(&name).or_else(|| canonical_glm(&id)),
        upstream_id: None,
        upstream_model_ids: vec![],
        id,
        name,
        supports_tools: true,
        context_window: context,
        context_window_options: vec![],
        max_output_tokens: None,
        supports_images: None,
        supports_reasoning: None,
        reasoning_effort_options: vec![],
        default_reasoning_effort: None,
        verified: false,
        verification_note: Some("已从账号实时目录发现，尚未验证实际模型身份".into()),
    }
}
fn canonical_glm(s: &str) -> Option<String> {
    let s = s.trim().to_ascii_lowercase().replace(' ', "-");
    if !s.starts_with("glm-") {
        return None;
    }
    let suffix = &s[4..];
    if suffix.is_empty()
        || !suffix.as_bytes()[0].is_ascii_digit()
        || suffix
            .chars()
            .any(|c| !c.is_ascii_alphanumeric() && c != '.' && c != '-')
    {
        return None;
    }
    Some(s)
}
pub fn identity_matches(m: &ModelInfo, identity: &str) -> bool {
    let id = identity.trim();
    if let Some(c) = &m.canonical_id {
        canonical_glm(id).as_ref() == Some(c)
    } else {
        id.eq_ignore_ascii_case(&m.id)
    }
}
pub(super) fn outcome(
    a: &Account,
    r: &Reward,
    status: &str,
    message: impl Into<String>,
    amount: Option<f64>,
) -> RewardOutcome {
    RewardOutcome {
        account_id: a.id.clone(),
        reward_id: r.id.clone(),
        name: r.name.clone(),
        status: status.into(),
        amount,
        kind: r.kind.clone(),
        message: message.into(),
        created_at: now_ms(),
    }
}
pub(super) fn reconcile_claim(
    a: &Account,
    r: &Reward,
    submitted: &Result<Value, ProviderError>,
    after: Result<Vec<Reward>, ProviderError>,
) -> RewardOutcome {
    let confirmed = after
        .as_ref()
        .is_ok_and(|rs| rs.iter().any(|x| x.id == r.id && x.status == "claimed"));
    if confirmed {
        return outcome(a, r, "claimed", "回查上游确认权益已领取", r.amount);
    }
    match submitted {
        Err(e) if e.rejected => outcome(a, r, "failed", e.message.clone(), None),
        _ => outcome(
            a,
            r,
            "pending",
            "领取结果尚未确认，已回查状态；不会自动重发领取请求",
            None,
        ),
    }
}
pub(super) fn credentials(
    v: &Value,
    previous: &Credentials,
    token_keys: &[&str],
    refresh_keys: &[&str],
) -> Result<Credentials, ProviderError> {
    let mut c = previous.clone();
    c.access_token =
        text(v, token_keys).ok_or_else(|| ProviderError::new(502, "平台未返回访问凭证"))?;
    c.refresh_token = text(v, refresh_keys).or(c.refresh_token);
    c.expires_at = v
        .get("TokenExpireAt")
        .or_else(|| v.get("expires_at"))
        .and_then(epoch)
        .or_else(|| num(v, &["expiresIn", "expires_in"]).map(|n| now_ms() + n as i64 * 1000));
    Ok(c)
}
// Dev diagnostics: object paths with value lengths only — never the values.
pub(super) fn json_shape(value: &Value, depth: usize) -> String {
    match value {
        Value::Object(map) if depth < 4 => map
            .iter()
            .map(|(k, v)| match v {
                Value::Object(_) | Value::Array(_) => {
                    format!("{k}{{{}}}", json_shape(v, depth + 1))
                }
                Value::String(s) => format!("{k}({})", s.len()),
                other => format!("{k}={other}"),
            })
            .collect::<Vec<_>>()
            .join(" "),
        Value::Array(items) => format!("[{} items]", items.len()),
        other => format!("{}", other),
    }
}
pub(super) fn empty_credentials() -> Credentials {
    Credentials {
        access_token: String::new(),
        refresh_token: None,
        user_id: None,
        device_id: None,
        expires_at: None,
        extra: json!({}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_identity_never_merges_versions() {
        let m = model("gmodel".into(), "GLM-5.3".into(), None);
        assert!(identity_matches(&m, "GLM-5.3"));
        assert!(!identity_matches(&m, "gmodel"));
        assert!(!identity_matches(&m, "glm-5.2"));
        assert!(!identity_matches(&m, "auto"));
        assert!(!identity_matches(&m, "glm-5.3-flash"));
    }
    fn claim_account() -> Account {
        Account {
            id: "test".into(),
            platform: Platform::Workbuddy,
            user_id: "uid".into(),
            label: "test".into(),
            enabled: true,
            status: "ready".into(),
            last_error: None,
            snapshot: None,
        }
    }
    fn claim_reward(status: &str) -> Reward {
        Reward {
            id: "daily".into(),
            name: "daily".into(),
            kind: "credits".into(),
            status: status.into(),
            amount: Some(100.0),
            next_available_at: None,
            action_url: None,
        }
    }
    #[test]
    fn timed_out_claim_is_confirmed_by_readback() {
        let a = claim_account();
        let r = claim_reward("claimable");
        let uncertain = Err(ProviderError::new(502, "timeout"));
        assert_eq!(
            reconcile_claim(&a, &r, &uncertain, Ok(vec![claim_reward("claimed")])).status,
            "claimed"
        );
    }
    #[test]
    fn ambiguous_claim_never_reports_success_or_retries() {
        let a = claim_account();
        let r = claim_reward("claimable");
        let uncertain = Err(ProviderError::new(502, "timeout"));
        let o = reconcile_claim(
            &a,
            &r,
            &uncertain,
            Err(ProviderError::new(502, "read timeout")),
        );
        assert_eq!(o.status, "pending");
        assert_eq!(o.amount, None);
    }
    #[test]
    fn unknown_balance_is_not_zero() {
        assert_eq!(snapshot(vec![], vec![], vec![], None).balance, None);
    }
    #[test]
    fn embedded_billing_error_is_rejected_before_auth_success() {
        assert!(check_business(
            &json!({"code":0,"data":{"Response":{"Error":{"Code":"AuthFailure"}}}})
        )
        .is_err());
        assert!(check_business(&json!({"ResponseMetadata":{"Error":null}})).is_ok());
    }

    #[test]
    fn metadata_authentication_failure_is_refreshable_and_preserves_details() {
        for code in [json!("20101"), json!(20101)] {
            let error = check_business(&json!({"ResponseMetadata":{"Error": {
                "Code": code,
                "Message": "session rejected by issuing realm"
            }}}))
            .expect_err("reject authentication failure");
            assert_eq!(error.status, 401);
            assert!(error.rejected);
            assert_eq!(
                error.message,
                "平台认证失败（代码 20101）：session rejected by issuing realm"
            );
        }
    }

    #[test]
    fn non_authentication_business_failure_keeps_its_existing_classification() {
        let error =
            check_business(&json!({"code": 9001})).expect_err("reject ordinary business error");
        assert_eq!(error.status, 502);
        assert!(!error.rejected);
        assert_eq!(error.message, "平台业务请求被拒绝（代码 9001）");
        let error = check_business(&json!({"ResponseMetadata":{"Error":{
            "Code":30011,"Message":"unclassified upstream error"
        }}}))
        .expect_err("unknown metadata error must not be treated as expired enterprise credentials");
        assert_eq!(error.status, 502);
        assert!(!error.rejected);
        assert_eq!(
            error.message,
            "平台业务请求被拒绝（代码 30011）：unclassified upstream error"
        );
    }
}
