//! MiniMax Code public OAuth and daily-signin contracts; see NOTICE.md.
use super::*;
use sha2::{Digest, Sha256};
use std::path::Path;

const CLIENT_ID: &str = "mcode-public";
const APP_VERSION: &str = "0.6.9";
const SIGNIN_STATUS: &str = "/minimax-cloud/api/v1/signin/status";
const SIGNIN_CLAIM: &str = "/minimax-cloud/api/v1/signin/claim";

pub(super) fn account_origin(region: CreditRegion) -> &'static str {
    match region {
        CreditRegion::Cn => "https://account.minimax.cn",
        CreditRegion::Intl => "https://account.minimax.io",
    }
}
fn api_origin(region: CreditRegion) -> &'static str {
    match region {
        CreditRegion::Cn => "https://agent.minimaxi.com",
        CreditRegion::Intl => "https://agent.minimax.io",
    }
}

// Match encodeURIComponent, including its treatment of ! ' ( ) *.
fn encode_component(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}
fn request(
    client: &ProviderClient,
    c: &Credentials,
    origin: &str,
    path: &str,
    body: Option<&str>,
    now: i64,
) -> RequestBuilder {
    let mut url = reqwest::Url::parse(&format!("{origin}{path}")).unwrap();
    let language = if c.region == CreditRegion::Cn {
        "zh"
    } else {
        "en"
    };
    let platform = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    };
    url.query_pairs_mut().extend_pairs([
        ("device_platform", "web"),
        ("biz_id", "3"),
        ("app_id", "3001"),
        ("version_code", "22201"),
        ("is_desktop", "1"),
        ("desktop_version", APP_VERSION),
        ("unix", &now.to_string()),
        (
            "timezone_offset",
            &chrono::Local::now().offset().local_minus_utc().to_string(),
        ),
        ("sys_language", language),
        ("lang", language),
        ("device_id", "0"),
        ("os_name", platform),
        ("browser_name", "mcode"),
        ("user_id", c.user_id.as_deref().unwrap_or("0")),
        ("client", "mcode"),
    ]);
    let signed_path = format!("{}?{}", url.path(), url.query().unwrap_or(""));
    let md5 = |value: &str| format!("{:x}", md5::compute(value));
    // Official public client attribution constants; authorization uses the Bearer token.
    let yy_body = body.unwrap_or("{}");
    let yy = md5(&format!(
        "{}_{yy_body}{}ooui",
        encode_component(&signed_path),
        md5(&now.to_string())
    ));
    let second = now / 1000;
    let signature = md5(&format!(
        "{second}I*7Cf%WZ#S&%1RlZJ&C2{}",
        body.unwrap_or("")
    ));
    let request = if let Some(body) = body {
        client.http.post(url).body(body.to_owned())
    } else {
        client.http.get(url)
    };
    request
        .bearer_auth(&c.access_token)
        .header("User-Agent", "MiniMaxCode")
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("yy", yy)
        .header("x-timestamp", second.to_string())
        .header("x-signature", signature)
}

fn check_response(value: &Value) -> Result<(), ProviderError> {
    for path in ["/statusInfo/code", "/base_resp/status_code"] {
        if let Some(code) = value
            .pointer(path)
            .and_then(Value::as_i64)
            .filter(|code| *code != 0)
        {
            let auth = code == 1_000_048;
            return Err(ProviderError {
                rejected: auth,
                upstream_code: Some(code.to_string()),
                ..ProviderError::new(
                    if auth { 401 } else { 502 },
                    if auth {
                        "MiniMax Code 登录已过期，请重新登录".into()
                    } else {
                        format!("MiniMax Code 请求被拒绝（代码 {code}）")
                    },
                )
            });
        }
    }
    Ok(())
}
async fn read(
    client: &ProviderClient,
    c: &Credentials,
    path: &str,
    body: Option<&Value>,
) -> Result<Value, ProviderError> {
    read_at(client, c, api_origin(c.region), path, body).await
}
async fn read_at(
    client: &ProviderClient,
    c: &Credentials,
    origin: &str,
    path: &str,
    body: Option<&Value>,
) -> Result<Value, ProviderError> {
    let serialized = body.map(Value::to_string);
    let value = client
        .json(request(
            client,
            c,
            origin,
            path,
            serialized.as_deref(),
            now_ms(),
        ))
        .await?;
    check_response(&value)?;
    Ok(value)
}

pub(super) async fn authenticate(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<(String, String, Credentials), ProviderError> {
    let mut probe = c.clone();
    probe.user_id = None; // Resolve the server identity; never trust an imported ID.
    let value = read(client, &probe, "/v1/api/user/info", None).await?;
    let user = value
        .pointer("/data/userInfo")
        .or_else(|| value.pointer("/data/user_info"))
        .or_else(|| value.get("userInfo"))
        .or_else(|| value.get("user_info"))
        .ok_or_else(|| ProviderError::new(502, "MiniMax Code 未返回账号身份"))?;
    let id = text(user, &["realUserID", "real_user_id"])
        .ok_or_else(|| ProviderError::new(502, "MiniMax Code 未返回账号 ID"))?;
    if c.user_id.as_ref().is_some_and(|expected| expected != &id) {
        return Err(ProviderError::new(
            401,
            "MiniMax Code 账号已变化，请重新导入或登录",
        ));
    }
    let label = text(user, &["name", "userName", "user_name", "email"])
        .unwrap_or_else(|| "MiniMax Code".into());
    let mut credentials = c.clone();
    credentials.user_id = Some(id.clone());
    Ok((id, label, credentials))
}

pub(super) fn token_credentials(
    value: &Value,
    previous: &Credentials,
) -> Result<Credentials, ProviderError> {
    let token = text(value, &["access_token"]);
    let refresh = text(value, &["refresh_token"]).or_else(|| previous.refresh_token.clone());
    let expires = num(value, &["expires_in"])
        .filter(|value| value.is_finite() && *value > 0.0 && *value < 31_536_000.0);
    let claims = token.as_deref().map(jwt_claims).unwrap_or(Value::Null);
    let scopes = value
        .get("scope")
        .or_else(|| claims.get("scope"))
        .or_else(|| claims.get("scp"));
    let allowed_scope = scopes.is_some_and(|scope| {
        scope
            .as_str()
            .is_some_and(|scope| scope.split_whitespace().any(|s| s == "agent.default"))
            || scope
                .as_array()
                .is_some_and(|scopes| scopes.iter().any(|s| s == "agent.default"))
    });
    if token.is_none()
        || refresh.is_none()
        || expires.is_none()
        || !allowed_scope
        || !text(value, &["token_type"]).is_some_and(|kind| kind.eq_ignore_ascii_case("bearer"))
    {
        return Err(ProviderError::new(502, "MiniMax Code 授权响应无效"));
    }
    let mut credentials = previous.clone();
    credentials.access_token = token.unwrap();
    credentials.refresh_token = refresh;
    credentials.expires_at = Some(now_ms() + (expires.unwrap() * 1000.0) as i64);
    Ok(credentials)
}
pub(super) async fn refresh(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<Credentials, ProviderError> {
    if c.extra["localImport"].as_bool() == Some(true) {
        // The official client owns its rotating refresh token. Re-read its current
        // lease instead of consuming that refresh token in a second process.
        let local = local_credentials_for_region(c.region)
            .map_err(|message| ProviderError::new(401, message))?
            .filter(|local| {
                local
                    .expires_at
                    .is_some_and(|expires| expires > now_ms() + 60_000)
            })
            .ok_or_else(|| {
                ProviderError::new(401, "请先在 MiniMax Code 刷新登录，或在此使用网页登录")
            })?;
        let (id, _, mut local) = authenticate(client, &local).await?;
        if c.user_id.as_ref().is_some_and(|expected| expected != &id) {
            return Err(ProviderError::new(
                401,
                "本机 MiniMax Code 已切换账号，请重新导入",
            ));
        }
        local.user_id = Some(id);
        return Ok(local);
    }
    let refresh = c
        .refresh_token
        .as_deref()
        .ok_or_else(|| ProviderError::new(401, "MiniMax Code 登录已过期，请重新登录"))?;
    let value = client
        .json(
            client
                .http
                .post(format!("{}/oauth2/token", account_origin(c.region)))
                .form(&[
                    ("grant_type", "refresh_token"),
                    ("refresh_token", refresh),
                    ("client_id", CLIENT_ID),
                    ("scope", "agent.default"),
                    ("audience", "agent-backend"),
                ]),
        )
        .await?;
    if value.get("error").is_some() {
        return Err(ProviderError::new(
            401,
            "MiniMax Code 凭据刷新失败，请重新登录",
        ));
    }
    token_credentials(&value, c)
}

fn balance_snapshot(value: &Value) -> Result<ProviderSnapshot, ProviderError> {
    let membership = data(value);
    let remaining = membership
        .get("op_credit_summary")
        .and_then(|summary| num(summary, &["total_remaining_amount"]))
        .or_else(|| num(membership, &["opcredit_balance", "total_remains_credit"]))
        .filter(|amount| amount.is_finite() && *amount >= 0.0);
    let mut result = snapshot(vec![], vec![], vec![], None);
    result.balance = remaining;
    if remaining.is_none() {
        return Err(ProviderError::new(
            502,
            "MiniMax Code 未返回可识别的积分余额",
        ));
    }
    Ok(result)
}
pub(super) async fn balance(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<ProviderSnapshot, ProviderError> {
    // Select the personal workspace so team balances are not mixed with check-in credits.
    let workspaces = read(
        client,
        c,
        "/matrix/api/v1/user/get_user_extra_info",
        Some(&json!({})),
    )
    .await?;
    let workspace = data(&workspaces)["workspaces"]
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .find(|item| item["workspace_type"].as_i64() == Some(0))
        });
    let body = workspace
        .and_then(|workspace| workspace.get("workspace_id"))
        .map(|id| json!({"workspace_id":id}))
        .unwrap_or_else(|| json!({}));
    let membership = read(
        client,
        c,
        "/matrix/api/v1/commerce/get_membership_info",
        Some(&body),
    )
    .await?;
    balance_snapshot(&membership)
}

fn signin_reward(panel: &Value) -> Result<Reward, ProviderError> {
    let invalid = || ProviderError::new(502, "MiniMax Code 签到状态格式无效");
    let days = panel["days"]
        .as_array()
        .filter(|days| days.len() == 7)
        .ok_or_else(invalid)?;
    if !panel["scene"].as_u64().is_some_and(|scene| scene <= 4) {
        return Err(invalid());
    }
    let mut numbers = std::collections::HashSet::new();
    for day in days {
        let Some(number @ 1..=7) = day["day_no"].as_u64() else {
            return Err(invalid());
        };
        if !numbers.insert(number)
            || !day["status"]
                .as_u64()
                .is_some_and(|status| (1..=4).contains(&status))
            || !day["points"]
                .as_f64()
                .is_some_and(|points| points.is_finite() && points >= 0.0)
            || !day["is_today"].is_boolean()
        {
            return Err(invalid());
        }
    }
    if days.iter().filter(|day| day["is_today"] == true).count() > 1
        || days.iter().filter(|day| day["status"] == 2).count() > 1
    {
        return Err(invalid());
    }
    let claimed = days
        .iter()
        .find(|day| day["is_today"] == true && day["status"] == 3);
    let current = claimed
        .or_else(|| days.iter().find(|day| day["status"] == 2))
        .or_else(|| days.iter().find(|day| day["is_today"] == true));
    Ok(Reward {
        id: format!(
            "daily-signin-{}",
            (now_ms() + 8 * 3_600_000).div_euclid(86_400_000)
        ),
        name: current
            .map(|day| format!("每日签到 · 第 {} 天", day["day_no"]))
            .unwrap_or_else(|| "每日签到".into()),
        kind: "credits".into(),
        status: if claimed.is_some() {
            "claimed"
        } else if current.is_some_and(|day| day["status"] == 2) {
            "claimable"
        } else {
            "unavailable"
        }
        .into(),
        amount: current.and_then(|day| day["points"].as_f64()), // bonus_points is already included.
        next_available_at: None,
        action_url: None,
    })
}
pub(super) async fn rewards(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<Vec<Reward>, ProviderError> {
    rewards_at(client, c, api_origin(c.region)).await
}
async fn rewards_at(
    client: &ProviderClient,
    c: &Credentials,
    origin: &str,
) -> Result<Vec<Reward>, ProviderError> {
    let value = read_at(client, c, origin, SIGNIN_STATUS, None).await?;
    Ok(vec![signin_reward(value.get("data").ok_or_else(
        || ProviderError::new(502, "MiniMax Code 未返回签到数据"),
    )?)?])
}
pub(super) async fn models(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<Vec<ModelInfo>, ProviderError> {
    parse_models(&read(client, c, "/archon/api/v1/config", None).await?)
}
fn parse_models(value: &Value) -> Result<Vec<ModelInfo>, ProviderError> {
    let entries = data(value)["models"].as_array().ok_or_else(|| ProviderError::new(502, "MiniMax Code 未返回模型目录"))?;
    let mut seen = std::collections::HashSet::new();
    let mut result = vec![];
    for entry in entries {
        if entry["provider_id"] != "minimax" { continue; }
        let Some(id) = text(entry, &["model_id"]).filter(|id| id.len() <= 128
            && id.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))) else { continue; };
        if !seen.insert(id.clone()) { continue; }
        let name = text(entry, &["display_name"]).unwrap_or_else(|| id.clone());
        let mut item = model(id, name, token_limit(entry, &["context_limit"]));
        // Tool/image capabilities are declared by the pinned official shared
        // registry; the live account catalog alone controls which models exist.
        let registered = matches!(item.id.as_str(), "MiniMax-M3" | "MiniMax-M3.1-Flash" | "MiniMax-M2.7" | "MiniMax-M2.7-highspeed");
        item.supports_tools = flag(entry, &["tool_call"]).unwrap_or(registered);
        item.supports_images = flag(entry, &["attachment"]).or_else(|| registered.then_some(item.id.starts_with("MiniMax-M3")));
        item.max_output_tokens = token_limit(entry, &["max_output_tokens", "output_limit"]);
        item.context_window_options = entry["context_window_options"].as_array().into_iter().flatten()
            .filter_map(Value::as_u64).filter(|limit| *limit > 0).collect();
        // Older cloud entries omit M3's selectable tiers and still report 400K.
        // The official shared registry declares 512K/1M for both exact models.
        // Keep explicit account tier lists authoritative, including empty lists.
        if entry.get("context_window_options").is_none()
            && matches!(item.id.as_str(), "MiniMax-M3" | "MiniMax-M3.1-Flash") {
            item.context_window_options = vec![512000, 1000000];
        }
        item.context_window = item.context_window_options.iter().copied().chain(item.context_window).max();
        let mode = entry.pointer("/thinking_config/mode").and_then(Value::as_str);
        item.supports_reasoning = mode.map(|mode| mode != "forced_off");
        item.reasoning_effort_options = effort_options(&entry["effort_options"]);
        item.default_reasoning_effort = text(entry, &["default_effort"]);
        if mode == Some("switchable") {
            item.reasoning_effort_options.extend(["off".into(), "on".into()]);
            item.default_reasoning_effort = Some(if entry.pointer("/thinking_config/default_value").and_then(Value::as_str) == Some("false") { "off" } else { "on" }.into());
        }
        item.verification_note = Some("模型来自账号实时目录；缺失的上下文档位及工具、图片能力参考官方共享目录，尚未验证推理身份".into());
        result.push(item);
    }
    Ok(result)
}
pub(super) async fn inspect(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<ProviderSnapshot, ProviderError> {
    let (balance, rewards, models) = tokio::join!(balance(client, c), rewards(client, c), models(client, c));
    let mut result = balance?;
    result.rewards = rewards?;
    result.models = models?;
    Ok(result)
}
pub(super) async fn chat(
    client: &ProviderClient,
    c: &Credentials,
    model: &ModelInfo,
    body: Value,
) -> Result<ProviderChat, ProviderError> {
    let body = super::minimax_chat::request_body(&body, model)?;
    let origin = match c.region { CreditRegion::Cn => "https://agent.minimax.cn", CreditRegion::Intl => "https://agent.minimax.io" };
    let request = client.http.post(format!("{origin}/mavis/api/v1/llm/v1/messages"))
        .bearer_auth(&c.access_token).header("anthropic-version", "2023-06-01")
        .header("User-Agent", "MiniMaxAgent").header("X-Mavis-Session-Id", uuid::Uuid::new_v4().to_string())
        .header("X-Mavis-Agent-Id", "main")
        .header("X-Mavis-Timezone-Offset", chrono::Local::now().offset().local_minus_utc().to_string())
        .json(&body);
    super::stream::response(request, Platform::MinimaxCode).await
}
fn claim_outcome(
    account: &Account,
    reward: &Reward,
    value: &Value,
) -> Result<RewardOutcome, ProviderError> {
    let result = data(value);
    let points = result["points"]
        .as_f64()
        .filter(|points| points.is_finite() && *points >= 0.0);
    if !matches!(result["claim_result"].as_u64(), Some(1 | 2))
        || points.is_none()
        || text(result, &["claim_id"]).is_none()
        || !result["day_no"]
            .as_u64()
            .is_some_and(|day| (1..=7).contains(&day))
        || !result["expire_at_ms"].is_number()
        || signin_reward(&result["panel"]).is_err()
    {
        return Err(ProviderError::new(
            502,
            "MiniMax Code 领取响应无效，需回查状态",
        ));
    }
    let already = result["claim_result"] == 2;
    Ok(outcome(
        account,
        reward,
        if already {
            "already_claimed"
        } else {
            "claimed"
        },
        if already {
            "今日签到积分已领取"
        } else {
            "签到积分已领取"
        },
        if already { None } else { points },
    ))
}
pub(super) async fn claim(
    client: &ProviderClient,
    account: &Account,
    c: &Credentials,
) -> Result<Vec<RewardOutcome>, ProviderError> {
    claim_at(client, account, c, api_origin(c.region)).await
}
async fn claim_at(
    client: &ProviderClient,
    account: &Account,
    c: &Credentials,
    origin: &str,
) -> Result<Vec<RewardOutcome>, ProviderError> {
    let current = rewards_at(client, c, origin).await?;
    let reward = &current[0];
    if reward.status != "claimable" {
        return Ok(vec![outcome(
            account,
            reward,
            if reward.status == "claimed" {
                "already_claimed"
            } else {
                "unavailable"
            },
            "当前没有可领取的签到积分",
            None,
        )]);
    }
    let submitted = read_at(client, c, origin, SIGNIN_CLAIM, Some(&json!({}))).await;
    if let Ok(value) = &submitted {
        if let Ok(result) = claim_outcome(account, reward, value) {
            return Ok(vec![result]);
        }
    }
    // Ambiguous/timeout results are queried once, never resubmitted here.
    Ok(vec![reconcile_claim(
        account,
        reward,
        &submitted,
        rewards_at(client, c, origin).await,
    )])
}

fn parse_local(
    value: &Value,
    data_dir: &Path,
    region: CreditRegion,
) -> Result<Credentials, String> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let region_key = if region == CreditRegion::Cn {
        "cn"
    } else {
        "en"
    };
    let auth_home = data_dir.join("auth");
    let account_key = URL_SAFE_NO_PAD.encode(Sha256::digest(
        format!("{}\0{CLIENT_ID}", auth_home.display()).as_bytes(),
    ));
    let record_key = format!("com.minimax.mcode.oauth.prod.{region_key}\0{account_key}");
    let record = &value["records"][record_key];
    if value["schemaVersion"] != 1
        || record["schemaVersion"] != 1
        || record["clientId"] != CLIENT_ID
        || record["audience"] != "agent-backend"
        || record["tokenType"] != "Bearer"
        || !record["scopes"]
            .as_array()
            .is_some_and(|scopes| scopes.iter().any(|scope| scope == "agent.default"))
    {
        return Err("本机 MiniMax Code 凭据格式不受支持，请使用网页登录".into());
    }
    Ok(Credentials {
        region,
        access_token: text(record, &["accessToken"]).ok_or("本机 MiniMax Code 缺少访问凭据")?,
        refresh_token: None,
        expires_at: Some(
            record["expiresAtMs"]
                .as_i64()
                .ok_or("本机 MiniMax Code 缺少凭据有效期")?,
        ),
        user_id: None,
        device_id: None,
        extra: json!({"localImport":true}),
    })
}
pub(crate) fn local_credentials_for_region(
    region: CreditRegion,
) -> Result<Option<Credentials>, String> {
    let data_dir = std::env::var_os("MINIMAX_DATA_DIR")
        .filter(|path| !path.is_empty())
        .or_else(|| std::env::var_os("MAVIS_DATA_DIR").filter(|path| !path.is_empty()))
        .map(std::path::PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".minimax")))
        .ok_or("无法定位 MiniMax Code 数据目录")?;
    let data_dir = if data_dir.is_absolute() {
        data_dir
    } else {
        std::env::current_dir()
            .map_err(|_| "无法解析 MiniMax Code 数据目录")?
            .join(data_dir)
    };
    let path = data_dir
        .join("auth/prod")
        .join(if region == CreditRegion::Cn {
            "cn"
        } else {
            "en"
        })
        .join(CLIENT_ID)
        .join("auth.json");
    let raw = match std::fs::read(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("无法读取本机 MiniMax Code 凭据".into()),
    };
    let value: Value =
        serde_json::from_slice(&raw).map_err(|_| "本机 MiniMax Code 凭据文件无效")?;
    parse_local(&value, &data_dir, region).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credentials() -> Credentials {
        Credentials {
            region: CreditRegion::Cn,
            access_token: "synthetic-access".into(),
            refresh_token: None,
            user_id: Some("synthetic-user".into()),
            device_id: None,
            expires_at: None,
            extra: json!({}),
        }
    }
    fn panel(status: u64) -> Value {
        json!({"scene":2,"days":(1..=7).map(|day| json!({"day_no":day,"points":1234,
            "bonus_points":234,"is_today":day == 4,"status":if day == 4 { status } else { 1 }})).collect::<Vec<_>>()})
    }
    fn account() -> Account {
        Account {
            id: "fixture".into(),
            platform: Platform::MinimaxCode,
            region: CreditRegion::Cn,
            user_id: "synthetic-user".into(),
            label: "MiniMax Code".into(),
            enabled: true,
            status: "ready".into(),
            last_error: None,
            snapshot: None,
        }
    }

    #[test]
    fn live_catalog_selects_the_highest_official_context_tiers() {
        let value = json!({"models":[
            {"provider_id":"minimax","model_id":"MiniMax-M3.1-Flash","display_name":"M3.1-Flash","context_limit":512000,
                "context_window_options":[512000,1000000],"thinking_config":{"mode":"forced_on"},
                "effort_options":["default","low","high"],"default_effort":"default"},
            {"provider_id":"minimax","model_id":"MiniMax-M3","context_limit":400000,"thinking_config":{"mode":"switchable","default_value":"true"}},
            {"provider_id":"other","model_id":"not-a-minimax-route"},
            {"provider_id":"minimax","model_id":"MiniMax-M3"}
        ]});
        let items = parse_models(&value).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].context_window, Some(1000000));
        assert_eq!(items[0].context_window_options, vec![512000,1000000]);
        assert_eq!(items[0].max_output_tokens, None);
        assert!(items[0].supports_tools && items[0].supports_images == Some(true));
        assert_eq!(items[1].context_window, Some(1000000));
        assert_eq!(items[1].context_window_options, vec![512000,1000000]);
        assert_eq!(items[1].reasoning_effort_options, vec!["off","on"]);
        assert!(items.iter().all(|item| !item.verified));
        assert!(parse_models(&json!({})).is_err());
        assert!(parse_models(&json!({"models":[]})).unwrap().is_empty());
    }

    #[test]
    fn official_m3_tiers_fill_missing_metadata_without_overriding_explicit_account_limits() {
        for id in ["MiniMax-M3", "MiniMax-M3.1-Flash"] {
            let mut entry = json!({"provider_id":"minimax","model_id":id,"context_limit":400000});
            let read = |entry: &Value| parse_models(&json!({"models":[entry]})).unwrap().remove(0);
            assert_eq!(read(&entry).context_window, Some(1000000));
            entry["context_window_options"] = json!([128000,400000]);
            let explicit = read(&entry);
            assert_eq!(explicit.context_window, Some(400000));
            assert_eq!(explicit.context_window_options, vec![128000,400000]);
            entry["context_window_options"] = json!([]);
            assert_eq!(read(&entry).context_window, Some(400000));
            assert!(read(&entry).context_window_options.is_empty());
        }
        let unknown = parse_models(&json!({"models":[{"provider_id":"minimax","model_id":"future-model","context_limit":400000}]})).unwrap();
        assert_eq!(unknown[0].context_window, Some(400000));
        assert!(unknown[0].context_window_options.is_empty());
    }

    #[test]
    fn balances_use_the_summary_once_and_keep_missing_values_unknown() {
        let value = json!({"op_credit_summary":{"total_remaining_amount":"123.5","free_remaining_amount":"100","purchased_remaining_amount":"23.5"},"opcredit_balance":999,"total_remains_credit":9999});
        assert_eq!(balance_snapshot(&value).unwrap().balance, Some(123.5));
        assert_eq!(
            balance_snapshot(&json!({"data":{"total_remains_credit":0}}))
                .unwrap()
                .balance,
            Some(0.0)
        );
        for invalid in [
            json!({}),
            json!({"opcredit_balance":-1}),
            json!({"opcredit_balance":"NaN"}),
        ] {
            assert!(balance_snapshot(&invalid).is_err());
        }
    }

    #[test]
    fn signin_uses_live_points_without_adding_bonus_and_requires_a_valid_panel() {
        for (status, expected) in [
            (1, "unavailable"),
            (2, "claimable"),
            (3, "claimed"),
            (4, "unavailable"),
        ] {
            let reward = signin_reward(&panel(status)).unwrap();
            assert_eq!(reward.status, expected);
            assert_eq!(reward.amount, Some(1234.0));
        }
        let mut invalid = panel(2);
        invalid["days"][0]["day_no"] = json!(4);
        assert!(signin_reward(&invalid).is_err());
        invalid = panel(2);
        invalid["days"][0]["status"] = json!(2);
        assert!(signin_reward(&invalid).is_err());
        invalid = panel(2);
        invalid["days"][3]["points"] = Value::Null;
        assert!(signin_reward(&invalid).is_err());
        invalid = panel(2);
        invalid["days"][0]["is_today"] = json!(true);
        assert!(signin_reward(&invalid).is_err());
    }

    #[test]
    fn oauth_grants_require_bearer_scope_and_expiry_and_preserve_region() {
        let mut c = credentials();
        c.region = CreditRegion::Intl;
        let value = json!({"access_token":"synthetic-next","refresh_token":"synthetic-refresh","token_type":"Bearer","expires_in":3600,"scope":"agent.default"});
        let token = token_credentials(&value, &c).unwrap();
        assert_eq!(token.region, CreditRegion::Intl);
        assert_eq!(token.user_id, c.user_id);
        assert!(token.expires_at.unwrap() > now_ms());
        for key in [
            "access_token",
            "refresh_token",
            "token_type",
            "expires_in",
            "scope",
        ] {
            let mut invalid = value.clone();
            invalid.as_object_mut().unwrap().remove(key);
            assert!(
                token_credentials(&invalid, &c).is_err(),
                "accepted missing {key}"
            );
        }
    }

    #[test]
    fn native_business_errors_never_expose_the_response_message() {
        let error =
            check_response(&json!({"statusInfo":{"code":1000048,"message":"fixture-secret"}}))
                .unwrap_err();
        assert_eq!(error.status, 401);
        assert!(error.rejected);
        assert!(!error.message.contains("fixture-secret"));
        assert!(check_response(
            &json!({"base_resp":{"status_code":1001,"status_msg":"fixture-secret"}})
        )
        .is_err());
    }

    #[test]
    fn local_import_matches_the_exact_region_and_namespace_without_copying_refresh_tokens() {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
        let root = Path::new("/synthetic/minimax");
        let digest = URL_SAFE_NO_PAD.encode(Sha256::digest(
            format!("{}\0mcode-public", root.join("auth").display()).as_bytes(),
        ));
        let key = format!("com.minimax.mcode.oauth.prod.cn\0{digest}");
        let value = json!({"schemaVersion":1,"records":{key:{"schemaVersion":1,
            "clientId":"mcode-public","audience":"agent-backend","tokenType":"Bearer","scopes":["agent.default"],
            "accessToken":"synthetic-access","refreshToken":"must-not-be-consumed","expiresAtMs":123456789}}});
        let c = parse_local(&value, root, CreditRegion::Cn).unwrap();
        assert_eq!(c.access_token, "synthetic-access");
        assert!(c.refresh_token.is_none());
        assert_eq!(c.extra["localImport"], true);
        assert!(parse_local(&value, root, CreditRegion::Intl).is_err());
        assert!(parse_local(&value, Path::new("/other"), CreditRegion::Cn).is_err());
    }

    #[test]
    fn signing_keeps_the_exact_body_and_regional_origin() {
        let client = ProviderClient::new();
        for region in [CreditRegion::Cn, CreditRegion::Intl] {
            let mut c = credentials();
            c.region = region;
            for body in [None, Some("{}"), Some("{\"workspace_id\":42}")] {
                let request = request(
                    &client,
                    &c,
                    api_origin(region),
                    SIGNIN_STATUS,
                    body,
                    1_700_000_000_123,
                )
                .build()
                .unwrap();
                assert_eq!(
                    request.url().origin().ascii_serialization(),
                    api_origin(region)
                );
                assert_eq!(
                    request.headers()["authorization"],
                    "Bearer synthetic-access"
                );
                assert!(!request.url().as_str().contains("synthetic-access"));
                assert_eq!(
                    request.method(),
                    if body.is_some() {
                        reqwest::Method::POST
                    } else {
                        reqwest::Method::GET
                    }
                );
                let path = format!(
                    "{}?{}",
                    request.url().path(),
                    request.url().query().unwrap()
                );
                let yy = format!(
                    "{:x}",
                    md5::compute(format!(
                        "{}_{}{:x}ooui",
                        encode_component(&path),
                        body.unwrap_or("{}"),
                        md5::compute("1700000000123")
                    ))
                );
                assert_eq!(request.headers()["yy"], yy);
                let expected = format!(
                    "{:x}",
                    md5::compute(format!(
                        "1700000000I*7Cf%WZ#S&%1RlZJ&C2{}",
                        body.unwrap_or("")
                    ))
                );
                assert_eq!(request.headers()["x-signature"], expected);
            }
        }
        assert_eq!(
            encode_component("/a?b=x&c=!~*'()"),
            "%2Fa%3Fb%3Dx%26c%3D!~*'()"
        );
        assert_eq!(
            serde_json::to_string(&Platform::MinimaxCode).unwrap(),
            "\"minimax-code\""
        );
        assert_ne!(
            Platform::MinimaxCode.route_key(CreditRegion::Cn),
            Platform::MinimaxCode.route_key(CreditRegion::Intl)
        );
    }

    #[tokio::test]
    async fn claims_skip_completed_rewards_and_reconcile_ambiguous_results_without_resubmitting() {
        use axum::{
            extract::State,
            http::{HeaderMap, StatusCode},
            routing::{get, post},
            Json, Router,
        };
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct Fixture {
            mode: u8,
            claims: AtomicUsize,
        }
        async fn status(State(f): State<Arc<Fixture>>) -> Json<Value> {
            let claimed = f.mode == 5 || (f.mode == 3 && f.claims.load(Ordering::SeqCst) > 0);
            Json(json!({"base_resp":{"status_code":0},"data":panel(if claimed { 3 } else { 2 })}))
        }
        async fn submit(
            State(f): State<Arc<Fixture>>,
            headers: HeaderMap,
            Json(body): Json<Value>,
        ) -> (StatusCode, Json<Value>) {
            assert_eq!(body, json!({}));
            assert_eq!(headers["authorization"], "Bearer synthetic-access");
            f.claims.fetch_add(1, Ordering::SeqCst);
            if f.mode == 2 {
                return (StatusCode::OK, Json(json!({"data":{"claim_result":1}})));
            }
            if f.mode >= 3 {
                return (StatusCode::BAD_GATEWAY, Json(json!({})));
            }
            (
                StatusCode::OK,
                Json(json!({"base_resp":{"status_code":0},"data":{
                "claim_id":"synthetic-claim","claim_result":if f.mode == 1 { 2 } else { 1 },
                "day_no":4,"points":1234,"expire_at_ms":1_900_000_000_000_i64,"panel":panel(3)}})),
            )
        }
        for (mode, expected) in [
            (0, "claimed"),
            (1, "already_claimed"),
            (2, "pending"),
            (3, "claimed"),
            (4, "pending"),
            (5, "already_claimed"),
        ] {
            let fixture = Arc::new(Fixture {
                mode,
                claims: AtomicUsize::new(0),
            });
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let origin = format!("http://{}", listener.local_addr().unwrap());
            let router = Router::new()
                .route(SIGNIN_STATUS, get(status))
                .route(SIGNIN_CLAIM, post(submit))
                .with_state(fixture.clone());
            let server = tokio::spawn(async move {
                axum::serve(listener, router).await.unwrap();
            });
            let mut client = ProviderClient::new();
            client.http = Client::builder().no_proxy().build().unwrap();
            let result = claim_at(&client, &account(), &credentials(), &origin)
                .await
                .unwrap();
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].status, expected, "mode {mode}");
            assert_eq!(
                result[0].amount,
                if expected == "claimed" {
                    Some(1234.0)
                } else {
                    None
                }
            );
            assert_eq!(
                fixture.claims.load(Ordering::SeqCst),
                if mode == 5 { 0 } else { 1 }
            );
            server.abort();
        }
    }

    #[tokio::test]
    #[ignore = "explicit read-only diagnostic using the local official client lease"]
    async fn minimax_local_read_only() {
        assert_eq!(
            std::env::var("AI_TOOLKIT_MINIMAX_DIAGNOSTIC").as_deref(),
            Ok("read-only")
        );
        let c = local_credentials_for_region(CreditRegion::Cn)
            .unwrap()
            .unwrap();
        let client = ProviderClient::new();
        let (_, _, c) = authenticate(&client, &c).await.unwrap();
        let result = inspect(&client, &c).await.unwrap();
        assert!(result.balance.is_some());
        assert_eq!(result.rewards.len(), 1);
        assert!(!result.models.is_empty());
        println!("MiniMax Code identity, personal credits, daily signin status and live model catalog verified; no claim or inference submitted.");
    }

    #[tokio::test]
    #[ignore = "explicit short inference diagnostic using an isolated in-memory gateway"]
    async fn minimax_local_gateway_inference() {
        use super::super::super::CreditManager;
        assert_eq!(std::env::var("AI_TOOLKIT_PROXY_DIAGNOSTIC").as_deref(), Ok("real-inference"));
        let c = local_credentials_for_region(CreditRegion::Cn).unwrap().unwrap();
        let client = ProviderClient::new();
        let (_, _, c) = authenticate(&client, &c).await.unwrap();
        let snapshot = inspect(&client, &c).await.unwrap();
        let mut account = account(); account.user_id = c.user_id.clone().unwrap(); account.snapshot = Some(snapshot);
        let manager = CreditManager::test();
        manager.store.save_account(&account).unwrap();
        manager.save_credentials(&account.id, &c).unwrap();
        let gateway = manager.gateway.start(manager.clone(), 0).await.unwrap();
        let http = Client::builder().no_proxy().timeout(Duration::from_secs(90)).build().unwrap();
        let key = manager.gateway_key().unwrap();
        let catalog: Value = http.get(format!("{}/models", gateway.base_url)).bearer_auth(&key).send().await.unwrap().json().await.unwrap();
        assert!(catalog["data"].as_array().unwrap().iter().any(|model| model["id"] == "minimax-code/MiniMax-M3"));
        let response = http.post(format!("{}/chat/completions", gateway.base_url)).bearer_auth(&key)
            .json(&json!({"model":"minimax-code/MiniMax-M3","max_tokens":128,"reasoning_effort":"off","stream":false,
                "messages":[{"role":"user","content":"Call toolkit_echo with value toolkit-minimax-verified."}],
                "tools":[{"type":"function","function":{"name":"toolkit_echo","description":"Return the provided fixed string without side effects.",
                    "parameters":{"type":"object","properties":{"value":{"type":"string"}},"required":["value"]}}}],"tool_choice":"required"}))
            .send().await.unwrap();
        let status = response.status(); let result: Value = response.json().await.unwrap();
        assert!(status.is_success(), "tool request failed: {}", result["error"]);
        assert_eq!(result["choices"][0]["finish_reason"], "tool_calls");
        let call = &result["choices"][0]["message"]["tool_calls"][0];
        assert_eq!(call["function"]["name"], "toolkit_echo");
        let input: Value = serde_json::from_str(call["function"]["arguments"].as_str().unwrap()).unwrap();
        assert_eq!(input["value"], "toolkit-minimax-verified");
        let response = http.post(format!("{}/messages", gateway.base_url)).header("x-api-key", &key)
            .json(&json!({"model":"minimax-code/MiniMax-M3","max_tokens":128,"thinking":{"type":"disabled"},"stream":true,
                "messages":[{"role":"user","content":"Call toolkit_echo, then repeat its result exactly."},
                    {"role":"assistant","content":[{"type":"tool_use","id":call["id"],"name":"toolkit_echo","input":input}]},
                    {"role":"user","content":[{"type":"tool_result","tool_use_id":call["id"],"content":"toolkit-minimax-verified"}]}],
                "tools":[{"name":"toolkit_echo","input_schema":{"type":"object","properties":{"value":{"type":"string"}},"required":["value"]}}]}))
            .send().await.unwrap();
        assert!(response.status().is_success());
        let text = response.text().await.unwrap();
        assert!(text.contains("event: message_stop"), "missing stream completion");
        let reply: String = text.lines().filter_map(|line| line.strip_prefix("data: "))
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter_map(|event| event.pointer("/delta/text").and_then(Value::as_str).map(str::to_owned)).collect();
        assert!(reply.contains("toolkit-minimax-verified"));
        let response = http.post(format!("{}/messages", gateway.base_url)).header("x-api-key", &key)
            .json(&json!({"model":"minimax-code/MiniMax-M3.1-Flash","max_tokens":256,"thinking":{"type":"adaptive"},
                "output_config":{"effort":"low"},"stream":false,"messages":[{"role":"user","content":"Reply exactly OK."}]}))
            .send().await.unwrap();
        let status = response.status(); let result: Value = response.json().await.unwrap();
        manager.gateway.stop().await.unwrap();
        assert!(status.is_success(), "Flash request failed: {}", result["error"]);
        assert_eq!(result["model"], "minimax-code/MiniMax-M3.1-Flash");
        assert!(result["content"].as_array().unwrap().iter().any(|block| block["text"].as_str().is_some_and(|text| text.contains("OK"))));
        println!("MiniMax Code live gateway passed: catalog, M3 Chat Completions tool call, Messages tool result/SSE, M3.1-Flash Messages JSON. No executable tool or credit claim was run; gateway keys/history stayed in memory.");
    }
}
