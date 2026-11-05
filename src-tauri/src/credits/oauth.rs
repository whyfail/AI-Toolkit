use super::*;
use super::{minimax, qoder, traework, workbuddy};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthStart {
    pub flow_id: String,
    pub authorization_url: String,
    pub expires_at: i64,
}
// Completed is consumed by the command layer; credentials never go to WebView.
pub enum OAuthPoll {
    Pending,
    Completed {
        platform: Platform,
        user_id: String,
        label: String,
        credentials: Credentials,
    },
    Failed {
        message: String,
    },
}
#[derive(Clone)]
pub(super) struct LoginFlow {
    platform: Platform,
    client: reqwest::Client,
    expires_at: i64,
    nonce: String,
    verifier: String,
    base: Credentials,
    callback: Arc<tokio::sync::Mutex<Option<Result<Value, String>>>>,
    poll_lock: Arc<tokio::sync::Mutex<()>>,
    callback_done: tokio_util::sync::CancellationToken,
}
fn pkce() -> (String, String) {
    let verifier = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}
// One stable local identity: starting a new login does not rotate device IDs.
fn local_identity() -> (String, String) {
    let digest = Sha256::digest(format!(
        "AI-Toolkit trae device v2:{:?}:{}",
        dirs::home_dir(),
        std::env::consts::OS
    ));
    let machine = hex::encode(digest);
    let mut n = [0u8; 8];
    n.copy_from_slice(&digest[..8]);
    (
        machine,
        format!("{:016}", u64::from_be_bytes(n) % 10_000_000_000_000_000),
    )
}
impl ProviderClient {
    pub async fn start_login(&self, platform: Platform, region: CreditRegion) -> Result<OAuthStart, ProviderError> {
        let flow_id = uuid::Uuid::new_v4().to_string();
        let mut expires_at = now_ms() + 5 * 60 * 1000;
        let client = reqwest::Client::builder()
            .cookie_store(true)
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| ProviderError::new(500, "无法初始化登录客户端"))?;
        let (verifier, challenge) = pkce();
        let (machine, device) = local_identity();
        let callback = Arc::new(tokio::sync::Mutex::new(None));
        let callback_done = tokio_util::sync::CancellationToken::new();
        let mut base = empty_credentials();
        base.region = region;
        base.device_id = Some(device);
        base.extra = json!({"machineId":machine});
        let mut nonce = flow_id.clone();
        let authorization_url = match platform {
            Platform::MinimaxCode => {
                let value = json_response(client.post(format!("{}/oauth2/device/code", minimax::account_origin(region)))
                    .form(&[("client_id", "mcode-public"), ("scope", "agent.default"), ("audience", "agent-backend"),
                        ("code_challenge", challenge.as_str()), ("code_challenge_method", "S256")])
                    .send().await.map_err(ProviderError::transport)?).await?;
                let user_code = text(&value, &["user_code"])
                    .ok_or_else(|| ProviderError::new(502, "MiniMax Code 未返回授权码"))?;
                let standard = text(&value, &["device_code"]);
                let account_polling = standard.is_none() && num(&value, &["expired_in"]).is_some();
                if standard.is_none() && !account_polling {
                    return Err(ProviderError::new(502, "MiniMax Code 未返回设备授权信息"));
                }
                nonce = standard.unwrap_or_else(|| user_code.clone());
                let expires = num(&value, &["expires_in"]).or_else(|| num(&value, &["expired_in"]).map(|expiry|
                    if expiry >= 1_000_000_000_000.0 { (expiry - now_ms() as f64) / 1000.0 } else { expiry }))
                    .filter(|expiry| expiry.is_finite() && *expiry > 0.0)
                    .ok_or_else(|| ProviderError::new(502, "MiniMax Code 授权有效期无效"))?;
                expires_at = now_ms() + (expires.min(1800.0) * 1000.0) as i64;
                let interval = num(&value, &["interval"]).unwrap_or(if account_polling { 5000.0 } else { 5.0 });
                let interval = if account_polling { interval } else { interval * 1000.0 };
                base.extra = json!({"pollIntervalMs":interval.clamp(1000.0, 60_000.0) as i64,
                    "nextPollAt":now_ms() + interval.clamp(1000.0, 60_000.0) as i64,
                    "codeParameter":if account_polling { "user_code" } else { "device_code" }});
                let complete = text(&value, &["verification_uri_complete"]);
                let url = complete.clone().or_else(|| text(&value, &["verification_uri", "verification_url"]))
                    .ok_or_else(|| ProviderError::new(502, "MiniMax Code 未返回授权地址"))?;
                validate_browser_url(&url, if region == CreditRegion::Cn { &["minimax.cn", "minimaxi.com"] } else { &["minimax.io"] })?;
                let mut url = reqwest::Url::parse(&url).map_err(|_| ProviderError::new(502, "MiniMax Code 授权地址无效"))?;
                if !account_polling && complete.is_none() { url.query_pairs_mut().append_pair("user_code", &user_code); }
                url.query_pairs_mut().append_pair("client_surface", "tui").append_pair("download_source", "mcode-internal");
                url.to_string()
            }
            Platform::Workbuddy => {
                let v = json_response(
                    workbuddy::headers(
                        client
                            .post(format!("{}/v2/plugin/auth/state", workbuddy::api_base(&base)))
                            .query(&[("platform", if region == CreditRegion::Intl { "workbuddy-ai" } else { "CLI" })]),
                        &base,
                    )
                    .json(&json!({}))
                    .send()
                    .await
                    .map_err(ProviderError::transport)?,
                )
                .await?;
                nonce = text(data(&v), &["state"])
                    .ok_or_else(|| ProviderError::new(502, "WorkBuddy 登录未返回状态"))?;
                let url = text(data(&v), &["authUrl", "authURL"])
                    .ok_or_else(|| ProviderError::new(502, "WorkBuddy 登录未返回授权地址"))?;
                validate_browser_url(&url, if region == CreditRegion::Intl { &["workbuddy.ai", "codebuddy.ai"] } else { &["codebuddy.cn", "copilot.tencent.com"] })?;
                url
            }
            Platform::Qoder => {
                let mut inner =
                    reqwest::Url::parse(&format!("{}/device/selectAccounts", qoder::auth_base(region))).unwrap();
                inner
                    .query_pairs_mut()
                    .append_pair("challenge", &challenge)
                    .append_pair("challenge_method", "S256")
                    .append_pair("nonce", &nonce)
                    .append_pair("machine_id", &machine)
                    .append_pair("client_id", qoder::client_id(region));
                let mut url = reqwest::Url::parse(&format!("{}/users/sign-in", qoder::auth_base(region))).unwrap();
                url.query_pairs_mut()
                    .append_pair("biz_variant", "qoder")
                    .append_pair("oauth_callback", inner.as_str());
                base.device_id = Some(machine.clone());
                url.to_string()
            }
            Platform::Traework => {
                // One stable device key per install: a re-login presenting the
                // same device id with a different key is rejected (20403).
                let install_seed: [u8; 32] = Sha256::digest(
                    format!("AI-Toolkit trae bootstrap device v2:{machine}").as_bytes(),
                )
                .into();
                let (public, private) = trae_key_pair_from_seed(install_seed)?;
                base.extra["devicePublicKey"] = json!(public);
                base.extra["devicePrivateKey"] = json!(private);
                base.extra["clientId"] = json!(traework::WORK_CLIENT_ID);
                if region == CreditRegion::Intl {
                    base.extra["apiBase"] = json!("https://growsg-normal.trae.ai");
                    base.extra["ideVersion"] = json!("3.6.13");
                }
                // Real-IDE login shape (TraeWorkAssistant packet capture, MIT):
                // the SOLO client id stalls the authorization page after the
                // billing step, so the IDE client id and page snapshot versions
                // are used, and the URL is built directly — no GetLoginGuidance.
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                    .await
                    .map_err(|_| ProviderError::new(500, "无法启动本机 OAuth 回调"))?;
                let port = listener
                    .local_addr()
                    .map_err(|_| ProviderError::new(500, "无法获取 OAuth 回调端口"))?
                    .port();
                // 授权页在渲染登录界面之前，按 ^http://127.0.0.1:(\d+)/authorize$
                // 客户端强校验回调地址；路径必须恰好是 /authorize，其余路径会直接
                // 显示"登录失败/网络错误"。并发流程靠各自的监听端口隔离。
                let callback_url = format!("http://127.0.0.1:{port}/authorize");
                let (hostname, device_type, os_version) = local_device_fingerprint();
                let url = trae_authorization_url(
                    traework::authorization_origin(region),
                    region,
                    traework::WORK_CLIENT_ID,
                    &flow_id,
                    &callback_url,
                    &machine,
                    base.device_id.as_deref().unwrap_or(""),
                    &challenge,
                    &hostname,
                    &device_type,
                    &os_version,
                );
                tokio::spawn(listen_callback(
                    listener,
                    "/authorize".into(),
                    callback.clone(),
                    expires_at,
                    flow_id.clone(),
                    callback_done.clone(),
                ));
                url
            }
        };
        let mut flows = self.flows.lock().await;
        flows.retain(|_, f| f.expires_at > now_ms());
        if flows.len() >= 8 {
            return Err(ProviderError::new(
                429,
                "正在进行的登录太多，请等待已有登录完成",
            ));
        }
        flows.insert(
            flow_id.clone(),
            LoginFlow {
                platform,
                client,
                expires_at,
                nonce,
                verifier,
                base,
                callback,
                poll_lock: Arc::default(),
                callback_done,
            },
        );
        Ok(OAuthStart {
            flow_id,
            authorization_url,
            expires_at,
        })
    }
    pub async fn poll_login(&self, id: &str) -> Result<OAuthPoll, ProviderError> {
        let flow = self
            .flows
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| ProviderError::new(404, "登录流程不存在，请重新登录"))?;
        let poll_lock = flow.poll_lock.clone();
        let _poll = poll_lock.lock().await;
        let flow = self.flows.lock().await.get(id).cloned()
            .ok_or_else(|| ProviderError::new(404, "此登录已结束，请重新登录"))?;
        if flow.expires_at < now_ms() {
            flow.callback_done.cancel();
            self.flows.lock().await.remove(id);
            return Ok(OAuthPoll::Failed {
                message: "登录已过期，请重新登录".into(),
            });
        }
        let platform = flow.platform;
        let result = self.poll_flow(id, flow.clone()).await;
        match result {
            Ok(Some(c)) => {
                flow.callback_done.cancel();
                // TRAE Work identity comes from the callback echo (the official
                // client skips post-exchange GetUserInfo); other platforms and
                // echo-less flows resolve identity server-side.
                let echoed = platform == Platform::Traework && c.user_id.is_some();
                if echoed {
                    self.flows.lock().await.remove(id);
                    let label = text(&c.extra, &["label"]).unwrap_or_else(|| {
                        format!("TRAE {} {}", if c.region == CreditRegion::Cn { "CN" } else { "国际版" }, c.user_id.as_deref().unwrap_or(""))
                    });
                    return Ok(OAuthPoll::Completed {
                        platform,
                        user_id: c.user_id.clone().unwrap_or_default(),
                        label,
                        credentials: c,
                    });
                }
                let auth = self.authenticate(platform, &c).await;
                self.flows.lock().await.remove(id);
                match auth {
                    Ok((user_id, label, credentials)) => Ok(OAuthPoll::Completed {
                        platform,
                        user_id,
                        label,
                        credentials,
                    }),
                    Err(e) => Ok(OAuthPoll::Failed { message: e.message }),
                }
            }
            Ok(None) => Ok(OAuthPoll::Pending),
            Err(e) if platform == Platform::Traework && e.message != "平台取消或拒绝了登录" =>
            {
                // A candidate is not a completed login. Invalid/foreign codes must
                // not close the real browser's pending PKCE flow.
                log::warn!(
                    "TRAE OAuth 候选回调未通过验证（状态 {}），继续等待",
                    e.status
                );
                Ok(OAuthPoll::Pending)
            }
            Err(e) => {
                flow.callback_done.cancel();
                self.flows.lock().await.remove(id);
                Ok(OAuthPoll::Failed { message: e.message })
            }
        }
    }
    async fn poll_workbuddy_at(
        &self,
        id: &str,
        f: &LoginFlow,
        origin: &str,
    ) -> Result<Option<Credentials>, ProviderError> {
        let mut c = if f.base.access_token.is_empty() {
            let r = workbuddy::headers(
                f.client
                    .get(format!("{}/v2/plugin/auth/token", origin))
                    .query(&[("state", f.nonce.as_str())]),
                &f.base,
            )
            .send()
            .await
            .map_err(ProviderError::transport)?;
            if !r.status().is_success() {
                return Err(http_error(r.status().as_u16()));
            }
            let v: Value = r
                .json()
                .await
                .map_err(|_| ProviderError::new(502, "WorkBuddy 登录响应无效"))?;
            if num(&v, &["code"]) == Some(11217.0) {
                return Ok(None);
            }
            check_business(&v)?;
            if text(data(&v), &["accessToken"]).is_none() {
                return Ok(None);
            }
            let mut c = credentials(data(&v), &f.base, &["accessToken"], &["refreshToken"])?;
            if c.region == CreditRegion::Intl {
                if let Some(domain) = text(data(&v), &["domain"]) {
                    let host = domain.trim_start_matches("https://").trim_end_matches('/');
                    if !matches!(host, "www.workbuddy.ai" | "www.codebuddy.ai") {
                        return Err(ProviderError::new(
                            502,
                            "WorkBuddy 登录返回了不匹配的账号区域",
                        ));
                    }
                    c.extra["authDomain"] = json!(host);
                }
            }
            if let Some(flow) = self.flows.lock().await.get_mut(id) {
                flow.base = c.clone();
            }
            c
        } else {
            f.base.clone()
        };
        if c.region == CreditRegion::Intl {
            let response = workbuddy::headers(
                f.client
                    .get(format!("{}/v2/plugin/login/account", origin))
                    .query(&[("state", f.nonce.as_str())]),
                &c,
            )
            .send()
            .await
            .map_err(ProviderError::transport)?;
            let status = response.status().as_u16();
            let selected: Value = response
                .json()
                .await
                .map_err(|_| ProviderError::new(502, "WorkBuddy 账号选择响应无效"))?;
            if num(&selected, &["code"]) == Some(12151.0) {
                return Ok(None);
            }
            if !(200..300).contains(&status) {
                return Err(http_error(status));
            }
            check_business(&selected)?;
            let account = data(&selected);
            c.user_id = text(account, &["uid", "userId", "user_id"]);
            if c.user_id.is_none() {
                return Err(ProviderError::new(502, "WorkBuddy 未返回所选账号身份"));
            }
            for (source, target) in [
                ("type", "accountType"),
                ("enterpriseId", "enterpriseId"),
                ("nickname", "nickname"),
            ] {
                if let Some(value) = account.get(source) {
                    c.extra[target] = value.clone();
                }
            }
        }
        Ok(Some(c))
    }
    async fn poll_flow(&self, id: &str, f: LoginFlow) -> Result<Option<Credentials>, ProviderError> {
        match f.platform {
            Platform::MinimaxCode => {
                if f.base.extra["nextPollAt"].as_i64().is_some_and(|next| next > now_ms()) { return Ok(None); }
                let parameter = if f.base.extra["codeParameter"] == "user_code" { "user_code" } else { "device_code" };
                let response = f.client.post(format!("{}/oauth2/token", minimax::account_origin(f.base.region)))
                    .form(&[("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                        (parameter, f.nonce.as_str()), ("client_id", "mcode-public"), ("code_verifier", f.verifier.as_str())])
                    .send().await.map_err(ProviderError::transport)?;
                let status = response.status();
                let value: Value = response.json().await.map_err(|_| ProviderError::new(502, "MiniMax Code 授权响应无效"))?;
                let state = text(&value, &["error", "status"]).unwrap_or_default();
                if matches!(state.as_str(), "pending" | "authorization_pending" | "slow_down") {
                    let interval = f.base.extra["pollIntervalMs"].as_i64().unwrap_or(5000)
                        + if state == "slow_down" { 5000 } else { 0 };
                    if let Some(flow) = self.flows.lock().await.get_mut(id) {
                        flow.base.extra["pollIntervalMs"] = json!(interval);
                        flow.base.extra["nextPollAt"] = json!(now_ms() + interval);
                    }
                    return Ok(None);
                }
                if !status.is_success() || value.get("error").is_some()
                    || matches!(state.as_str(), "denied" | "access_denied" | "expired" | "expired_token") {
                    return Err(ProviderError::new(401, "MiniMax Code 授权被拒绝或已过期，请重新登录"));
                }
                let mut base = f.base;
                base.extra = json!({});
                base.device_id = None;
                Ok(Some(minimax::token_credentials(&value, &base)?))
            }
            Platform::Workbuddy => self.poll_workbuddy_at(id, &f, workbuddy::api_base(&f.base)).await,
            Platform::Qoder => {
                let r = f
                    .client
                    .get(format!("{}/api/v1/deviceToken/poll", qoder::api_base(&f.base)))
                    .query(&[
                        ("nonce", f.nonce.as_str()),
                        ("verifier", f.verifier.as_str()),
                        ("challenge_method", "S256"),
                    ])
                    .send()
                    .await
                    .map_err(ProviderError::transport)?;
                if matches!(r.status().as_u16(), 202 | 404) {
                    return Ok(None);
                }
                let v = json_response(r).await?;
                let c = qoder::token_credentials(data(&v), &f.base)?;
                Ok(Some(c))
            }
            Platform::Traework => {
                let cb = f.callback.lock().await.take();
                let Some(cb) = cb else { return Ok(None) };
                let cb = cb.map_err(|s| ProviderError::new(400, s))?;
                // The callback's userInfo echo names the account: derive its
                // stable per-account device identity (reference 0.12.77) and
                // present it first — the key derivation is shared with the
                // tool family that registers these devices. The authorization
                // URL's bootstrap device stays as the fallback. The identity
                // that exchanges successfully is kept in the credentials so
                // bound refreshes sign as the same device.
                let mut derived: Option<Credentials> = None;
                if let Some((uid, _)) = ["userInfo", "user_info", "UserInfo", "userinfo"]
                    .iter()
                    .find_map(|key| text(&cb, &[key]))
                    .and_then(|raw| trae_callback_identity(&raw))
                {
                    let (device_id, machine_id, public, private) = trae_derived_identity(&uid)?;
                    let mut base = f.base.clone();
                    base.device_id = Some(device_id);
                    base.extra["machineId"] = json!(machine_id);
                    base.extra["devicePublicKey"] = json!(public);
                    base.extra["devicePrivateKey"] = json!(private);
                    derived = Some(base);
                }
                if let Some(refresh) = text(
                    &cb,
                    &[
                        "refreshToken",
                        "refresh_token",
                        "RefreshToken",
                        "refresh-token",
                    ],
                )
                .filter(|_| callback_auth_code(&cb).is_none())
                {
                    if !callback_is_bound(&cb, &f.nonce) {
                        return Err(ProviderError::new(400, "刷新凭证回调未绑定当前登录流程"));
                    }
                    let mut c = f.base.clone();
                    c.refresh_token = Some(refresh);
                    return Ok(Some(
                        self.refresh_credentials(Platform::Traework, &c).await?,
                    ));
                }
                let code = callback_auth_code(&cb)
                    .ok_or_else(|| ProviderError::new(400, "Trae OAuth 回调缺少授权码"))?;
                // The callback echoes the realm that owns this login (e.g.
                // api.trae.com.cn for accounts registered there) — exchange
                // against it first. Keep the origin that actually exchanges
                // successfully for this account's identity and billing APIs.
                let mut origins: Vec<String> = if f.base.region == CreditRegion::Intl {
                    vec!["https://growsg-normal.trae.ai".into()]
                } else {
                    vec![traework::API.into(), "https://api.trae.com.cn".into(), "https://www.trae.cn".into()]
                };
                if let Some(host) = ["host", "loginHost", "login_host", "LoginHost", "consoleHost"]
                    .iter().find_map(|key| text(&cb, &[key])) {
                    let origin = if host.starts_with("https://") { host } else { format!("https://{host}") };
                    if let Some(origin) = traework::allowed_account_origin(f.base.region, &origin) {
                        origins.insert(0, origin.to_owned());
                    } else {
                        return Err(ProviderError::new(400, "TRAE 登录回调的账号区域不匹配"));
                    }
                }
                if f.base.region == CreditRegion::Cn {
                    origins.push("https://api.trae.com.cn/cloudide".into());
                }
                let mut seen = std::collections::HashSet::new();
                origins.retain(|origin| seen.insert(origin.clone()));
                let mut exchange = Err(ProviderError::new(502, "未发起 Token 交换"));
                let mut used: Option<(Credentials, String)> = None;
                // The realm binds the code to the authorization URL's device at
                // issuance, so the exchange must present the same identity
                // first (bootstrap device, install-stable key); the derived
                // per-account identity stays as the fallback.
                for base in std::iter::once(&f.base).chain(derived.as_ref()) {
                    for origin in &origins {
                        // Real-client shape (TraeWorkAssistant packet capture,
                        // MIT): IDE_PC platform, empty brand fields, and the
                        // page app version. The x-cloudide-token header must be
                        // present as an EMPTY string — its absence rejects the
                        // exchange with 20403.
                        let device = json!({"DeviceID":base.device_id,"MachineID":base.extra["machineId"],"PlatformCode":"IDE_PC","DeviceType":"PC","DeviceName":"","DeviceModel":"","ClientVersion":traework::page_app_version(base.region),"DevicePublicKey":base.extra["devicePublicKey"],"DeviceBrand":"","DeviceCPU":"","OSInfo":"","OSVersion":""});
                        let exchange_body = json!({"ClientID":base.extra["clientId"],"AuthCode":code,"CodeVerifier":f.verifier,"DeviceInfo":device,"IDEVersion":traework::page_app_version(base.region)});
                        exchange = self
                            .json(
                                f.client
                                    .post(format!("{origin}/trae/api/v3/oauth/ExchangeToken"))
                                    .header("x-cloudide-token", "")
                                    .json(&exchange_body),
                            )
                            .await;
                        if exchange.is_ok() {
                            used = Some((base.clone(), origin.clone()));
                        }
                        let retry = match &exchange {
                            Err(error) => matches!(error.status, 404 | 500..=599),
                            Ok(_) => false,
                        };
                        if !retry {
                            break;
                        }
                    }
                    match &exchange {
                        Ok(_) => break,
                        // Device-identity rejections: the realm does not know
                        // the bootstrap device (20310) or knows it with a
                        // different key (20403) — retry with the derived one.
                        Err(error)
                            if error.message.contains("代码 20310")
                                || error.message.contains("代码 20403") => {}
                        Err(_) => break,
                    }
                }
                let v = exchange?;
                let (base, origin) =
                    used.expect("a successful exchange records its identity and origin");
                let mut c = trae_exchange_credentials(&v, &base, &origin)?;
                // Identity comes straight from the callback echo (the official
                // client never calls GetUserInfo after this exchange).
                if let Some((uid, label)) = ["userInfo", "user_info", "UserInfo", "userinfo"]
                    .iter()
                    .find_map(|key| text(&cb, &[key]))
                    .and_then(|raw| trae_callback_identity(&raw))
                {
                    c.user_id = Some(uid);
                    c.extra["label"] = json!(label);
                }
                Ok(Some(c))
            }
        }
    }
}

fn trae_exchange_credentials(
    response: &Value,
    base: &Credentials,
    exchange_origin: &str,
) -> Result<Credentials, ProviderError> {
    let mut c = credentials(
        data(response),
        base,
        &["AccessToken", "accessToken", "Token", "token"],
        &["RefreshToken", "refreshToken"],
    )?;
    // Keep the SSO identity separate from the cloudide session Token.
    if let Some(user_jwt) = text(data(response), &["UserJwt", "userJwt"]) {
        c.extra["userJwt"] = json!(user_jwt);
    }
    let origin = reqwest::Url::parse(exchange_origin)
        .map_err(|_| ProviderError::new(502, "Trae Token 交换来源无效"))?
        .origin()
        .ascii_serialization();
    // /cloudide belongs to the exchange endpoint, not to the API base.
    c.extra["apiBase"] = json!(origin);
    Ok(c)
}

fn validate_browser_url(url: &str, domains: &[&str]) -> Result<(), ProviderError> {
    let u = reqwest::Url::parse(url).map_err(|_| ProviderError::new(502, "平台授权地址无效"))?;
    if u.scheme() != "https"
        || u.username() != ""
        || u.password().is_some()
        || !u.host_str().is_some_and(|h| {
            domains
                .iter()
                .any(|d| h == *d || h.ends_with(&format!(".{d}")))
        })
    {
        return Err(ProviderError::new(502, "平台返回了非官方授权地址"));
    }
    Ok(())
}

fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

// Real machine fingerprint (hostname / platform / OS version) — the
// authorization page forwards these to the server, and fabricated Windows
// values on macOS risk device-binding risk control (observed as 20403).
fn local_device_fingerprint() -> (String, String, String) {
    let run = |cmd: &str, args: &[&str]| -> Option<String> {
        std::process::Command::new(cmd)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    };
    #[cfg(target_os = "macos")]
    return (
        run("scutil", &["--get", "ComputerName"])
            .or_else(|| run("hostname", &[]))
            .unwrap_or_else(|| "Mac".into()),
        "mac".into(),
        run("sw_vers", &["-productVersion"]).unwrap_or_else(|| "macOS".into()),
    );
    #[cfg(target_os = "windows")]
    return (
        std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Windows-PC".into()),
        "windows".into(),
        "Windows".into(),
    );
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    return (
        run("hostname", &[]).unwrap_or_else(|| "Linux".into()),
        "linux".into(),
        "Linux".into(),
    );
}

// Mirrors the real Trae IDE login URL (TraeWorkAssistant packet capture,
// MIT): the IDE client id, page snapshot versions, the real device
// fingerprint and an empty x_env. auth_callback_url stays unencoded — the
// page validates it client-side; everything else follows per-value encoding.
fn trae_authorization_url(
    host: &str,
    region: CreditRegion,
    client_id: &str,
    flow_id: &str,
    callback_url: &str,
    machine_id: &str,
    device_id: &str,
    challenge: &str,
    hostname: &str,
    device_type: &str,
    os_version: &str,
) -> String {
    let params: [(&str, String, bool); 23] = [
        ("login_version", "1".into(), false),
        ("auth_from", "trae".into(), false),
        ("login_channel", "native_ide".into(), false),
        ("plugin_version", traework::page_plugin_version(region).into(), true),
        ("auth_type", "local".into(), false),
        ("client_id", client_id.into(), false),
        ("redirect", "0".into(), false),
        ("login_trace_id", flow_id.into(), true),
        ("state", flow_id.into(), true),
        ("auth_callback_url", callback_url.into(), false),
        ("machine_id", machine_id.into(), true),
        ("device_id", device_id.into(), true),
        ("x_device_id", device_id.into(), true),
        ("x_machine_id", machine_id.into(), true),
        ("x_device_brand", hostname.into(), true),
        ("x_device_type", device_type.into(), true),
        ("x_os_version", os_version.into(), true),
        ("x_env", String::new(), false),
        ("x_app_version", traework::page_app_version(region).into(), true),
        ("x_app_type", "stable".into(), false),
        ("code_challenge", challenge.into(), true),
        ("code_challenge_method", "S256".into(), false),
        ("channel_name", "common".into(), false),
    ];
    let query = params
        .iter()
        .map(|(key, value, encode)| {
            if *encode {
                format!("{key}={}", percent_encode(value))
            } else {
                format!("{key}={value}")
            }
        })
        .collect::<Vec<_>>()
        .join("&");
    format!("{}/authorization?{query}", host.trim_end_matches('/'))
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                index += 3;
            }
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

// The callback's userInfo echo reveals who is logging in. The account's own
// stable device identity is derived from its UID so the server always sees
// one device per account, on any install, across every login (the reference
// 0.12.77 contract that avoids the device-cap lockout and matches the
// exchange to the registering client).
const TRAE_IDENTITY_SEED_PREFIX: &str = "cpa-multi-plugins/trae/account-device/v1";

fn trae_identity_seed(label: &str, uid: &str) -> [u8; 32] {
    Sha256::digest(format!("{TRAE_IDENTITY_SEED_PREFIX}\n{label}\nsolo\n{uid}").as_bytes()).into()
}

fn trae_callback_identity(raw: &str) -> Option<(String, String)> {
    let parse = |value: &str| -> Option<Value> {
        let mut current = value.trim().to_string();
        for _ in 0..2 {
            if current.starts_with('"') {
                current = serde_json::from_str::<String>(&current)
                    .ok()?
                    .trim()
                    .to_string();
            }
        }
        serde_json::from_str(&current).ok().filter(Value::is_object)
    };
    let object = parse(raw)?;
    let uid = text(&object, &["UserID", "userId", "uid", "UID", "user_id"])?;
    let nickname = text(
        &object,
        &[
            "ScreenName",
            "screenName",
            "Nickname",
            "nickname",
            "Name",
            "name",
            "displayName",
        ],
    )
    .unwrap_or_default();
    Some((uid, nickname))
}

fn trae_key_pair_from_seed(seed: [u8; 32]) -> Result<(String, String), ProviderError> {
    use p256::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};
    // Byte-for-byte the reference's DeterministicDeviceKeyPair: the server
    // registers (device id -> public key), so a device id derived with the
    // shared constants must also present the shared derivation's key. For
    // every realistic seed (< n-1, probability 1-2^-224) the Go reduction
    // `seed mod (n-1) + 1` equals `seed + 1` as a big-endian integer.
    let mut d = seed;
    for byte in d.iter_mut().rev() {
        let (sum, overflow) = byte.overflowing_add(1);
        *byte = sum;
        if !overflow {
            break;
        }
    }
    let key = p256::SecretKey::from_bytes(&d.into())
        .map_err(|_| ProviderError::new(500, "设备密钥派生失败"))?;
    let public = key
        .public_key()
        .to_public_key_pem(LineEnding::LF)
        .map_err(|_| ProviderError::new(500, "设备公钥编码失败"))?;
    let private = key
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|_| ProviderError::new(500, "设备私钥编码失败"))?;
    Ok((public, private.to_string()))
}

fn trae_derived_identity(uid: &str) -> Result<(String, String, String, String), ProviderError> {
    let device_id = {
        let seed = trae_identity_seed("device-id", uid);
        let mut be = [0u8; 8];
        be.copy_from_slice(&seed[..8]);
        let n = u64::from_be_bytes(be);
        format!(
            "{:016}",
            1_000_000_000_000_000u64 + n % 9_000_000_000_000_000
        )
    };
    let machine_id = {
        let seed = trae_identity_seed("machine-id", uid);
        let mut b = [0u8; 16];
        b.copy_from_slice(&seed[..16]);
        b[6] = (b[6] & 0x0f) | 0x40;
        b[8] = (b[8] & 0x3f) | 0x80;
        let hex = hex::encode(b);
        format!(
            "{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        )
    };
    let (public, private) = trae_key_pair_from_seed(trae_identity_seed("device-key", uid))?;
    Ok((device_id, machine_id, public, private))
}

// The Trae authorization page may deliver the auth code wrapped in an
// authCodeInfo JSON payload instead of a plain code parameter.
fn auth_code_from_info(raw: &str) -> Option<String> {
    let parse = |value: &str| {
        serde_json::from_str::<serde_json::Value>(value)
            .ok()
            .filter(Value::is_object)
    };
    let mut object = parse(raw);
    if object.is_none() {
        object = percent_decode(raw).and_then(|decoded| parse(&decoded));
    }
    let object = object?;
    [
        "authCode",
        "auth_code",
        "AuthCode",
        "authorization_code",
        "code",
    ]
    .iter()
    .find_map(|key| object.get(*key).and_then(Value::as_str))
    .filter(|code| !code.is_empty())
    .map(str::to_owned)
}

fn callback_auth_code(value: &Value) -> Option<String> {
    text(
        value,
        &[
            "authCode",
            "auth_code",
            "AuthCode",
            "authorization_code",
            "code",
        ],
    )
    .or_else(|| {
        ["authCodeInfo", "auth_code_info", "AuthCodeInfo"]
            .iter()
            .find_map(|key| text(value, &[key]))
            .and_then(|info| auth_code_from_info(&info))
    })
}

fn callback_is_bound(value: &Value, expected: &str) -> bool {
    if text(value, &["state", "State", "login_trace_id"]).as_deref() == Some(expected) {
        return true;
    }
    ["authCodeInfo", "auth_code_info", "AuthCodeInfo"]
        .iter()
        .filter_map(|key| text(value, &[key]))
        .filter_map(|raw| serde_json::from_str::<Value>(&raw).ok())
        .any(|info| text(&info, &["state", "State"]).as_deref() == Some(expected))
}

async fn listen_callback(
    listener: tokio::net::TcpListener,
    path: String,
    callback: Arc<tokio::sync::Mutex<Option<Result<Value, String>>>>,
    expires_at: i64,
    expected_state: String,
    done: tokio_util::sync::CancellationToken,
) {
    let Ok(address) = listener.local_addr() else {
        return;
    };
    let expected_host = address.to_string();
    while now_ms() < expires_at {
        let accepted = tokio::select! {
            _ = done.cancelled() => return,
            accepted = tokio::time::timeout(
                std::time::Duration::from_millis((expires_at - now_ms()).max(1) as u64),
                listener.accept(),
            ) => accepted,
        };
        let Ok(Ok((mut socket, _))) = accepted else {
            break;
        };
        let mut buf = Vec::new();
        let read = tokio::time::timeout(
            std::time::Duration::from_millis((expires_at - now_ms()).clamp(1, 5000) as u64),
            async {
                loop {
                    let mut part = [0u8; 2048];
                    let n = socket.read(&mut part).await?;
                    if n == 0 {
                        return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof));
                    }
                    buf.extend_from_slice(&part[..n]);
                    if buf.len() > 32768 {
                        return Err(std::io::Error::from(std::io::ErrorKind::InvalidData));
                    }
                    if buf.windows(4).any(|part| part == b"\r\n\r\n") {
                        return Ok(());
                    }
                }
            },
        )
        .await;
        if !matches!(read, Ok(Ok(()))) {
            continue;
        }
        let request = String::from_utf8_lossy(&buf);
        let mut words = request.lines().next().unwrap_or("").split_whitespace();
        let method = words.next();
        let target = words.next();
        let version = words.next();
        let hosts: Vec<_> = request
            .lines()
            .skip(1)
            .filter_map(|line| line.split_once(':'))
            .filter(|(key, _)| key.eq_ignore_ascii_case("host"))
            .map(|(_, value)| value.trim())
            .collect();
        let valid = method == Some("GET")
            && matches!(version, Some("HTTP/1.1" | "HTTP/1.0"))
            && words.next().is_none()
            && hosts == [expected_host.as_str()];
        let parsed = target
            .filter(|target| valid && target.starts_with('/') && !target.starts_with("//"))
            .and_then(|target| reqwest::Url::parse(&format!("http://{expected_host}{target}")).ok())
            .filter(|url| url.path() == path);
        let mut outcome = None;
        if let Some(url) = parsed {
            let mut value = json!({});
            let mut duplicate = false;
            for (key, data) in url.query_pairs() {
                if value.get(key.as_ref()).is_some() {
                    duplicate = true;
                }
                value[key.as_ref()] = json!(data.as_ref());
            }
            if !duplicate {
                let bound = callback_is_bound(&value, &expected_state);
                if text(&value, &["error", "error_code", "err", "errorCode"]).is_some() {
                    if bound {
                        outcome = Some(Err("平台取消或拒绝了登录".into()));
                    }
                } else {
                    let code = callback_auth_code(&value);
                    let refresh = text(
                        &value,
                        &[
                            "refreshToken",
                            "refresh_token",
                            "RefreshToken",
                            "refresh-token",
                        ],
                    );
                    if code.is_some() || (bound && refresh.is_some()) {
                        outcome = Some(Ok(value));
                    }
                }
            }
        }
        let accepted = if let Some(outcome) = outcome {
            let mut pending = callback.lock().await;
            if pending.is_none() {
                *pending = Some(outcome);
                true
            } else {
                false
            }
        } else {
            false
        };
        let response = if accepted {
            "HTTP/1.1 202 Accepted\r\nContent-Type: text/plain; charset=utf-8\r\nConnection: close\r\n\r\n登录回调已收到，验证完成后请返回 AI-Toolkit。"
        } else {
            "HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain; charset=utf-8\r\nConnection: close\r\n\r\n回调无效、缺少流程绑定或已有回调等待验证，请重试官方登录。"
        };
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            socket.write_all(response.as_bytes()),
        )
        .await;
        let _ = socket.shutdown().await;
        // Keep listening until a verified login completes or the original deadline expires.
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn workbuddy_international_polls_selected_account_without_reusing_one_time_token() {
        use axum::{extract::State, http::HeaderMap, routing::get, Json, Router};
        use std::sync::atomic::{AtomicUsize, Ordering};
        let counts = Arc::new((AtomicUsize::new(0), AtomicUsize::new(0)));
        async fn token(State(counts): State<Arc<(AtomicUsize, AtomicUsize)>>) -> Json<Value> {
            let call = counts.0.fetch_add(1, Ordering::SeqCst);
            Json(if call == 0 { json!({"code":11217}) } else {
                json!({"code":0,"data":{"accessToken":"opaque-session","refreshToken":"synthetic-refresh","domain":"www.codebuddy.ai"}})
            })
        }
        async fn selected(State(counts): State<Arc<(AtomicUsize, AtomicUsize)>>, headers: HeaderMap) -> Json<Value> {
            assert_eq!(headers["authorization"], "Bearer opaque-session");
            assert_eq!(headers["x-domain"], "www.codebuddy.ai");
            let call = counts.1.fetch_add(1, Ordering::SeqCst);
            Json(if call == 0 { json!({"code":12151}) } else {
                json!({"code":0,"data":{"uid":"selected-user","type":"enterprise","enterpriseId":"fixture-team","nickname":"Fixture"}})
            })
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let router = Router::new().route("/v2/plugin/auth/token", get(token))
            .route("/v2/plugin/login/account", get(selected)).with_state(counts.clone());
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap(); });
        let provider = ProviderClient::new();
        let mut base = empty_credentials();
        base.region = CreditRegion::Intl;
        let flow = LoginFlow {
            platform: Platform::Workbuddy,
            client: reqwest::Client::builder().no_proxy().build().unwrap(),
            expires_at: now_ms() + 60_000,
            nonce: "fixture-state".into(), verifier: "fixture-verifier".into(), base,
            callback: Arc::default(), poll_lock: Arc::default(), callback_done: Default::default(),
        };
        provider.flows.lock().await.insert("fixture".into(), flow);
        for expected_pending in [true, true, false] {
            let current = provider.flows.lock().await["fixture"].clone();
            let result = provider.poll_workbuddy_at("fixture", &current, &origin).await.unwrap();
            assert_eq!(result.is_none(), expected_pending);
            if let Some(c) = result {
                assert_eq!(c.region, CreditRegion::Intl);
                assert_eq!(c.user_id.as_deref(), Some("selected-user"));
                assert_eq!(c.extra["enterpriseId"], "fixture-team");
                assert_eq!(c.extra["accountType"], "enterprise");
                assert_eq!(c.extra["authDomain"], "www.codebuddy.ai");
            }
        }
        assert_eq!(counts.0.load(Ordering::SeqCst), 2);
        assert_eq!(counts.1.load(Ordering::SeqCst), 2);
        server.abort();
    }

    #[tokio::test]
    async fn qoder_oauth_uses_production_client_and_never_crosses_editions() {
        let provider = ProviderClient::new();
        for region in [CreditRegion::Cn, CreditRegion::Intl] {
            let started = provider.start_login(Platform::Qoder, region).await.unwrap();
            let url = reqwest::Url::parse(&started.authorization_url).unwrap();
            assert_eq!(url.origin().ascii_serialization(), qoder::auth_base(region));
            let callback = url.query_pairs().find(|(key, _)| key == "oauth_callback").unwrap().1.into_owned();
            let callback = reqwest::Url::parse(&callback).unwrap();
            assert_eq!(callback.origin().ascii_serialization(), qoder::auth_base(region));
            assert_eq!(callback.path(), "/device/selectAccounts");
            assert!(callback.query_pairs().any(|(key, value)| key == "client_id" && value == qoder::client_id(region)));
            assert_eq!(provider.flows.lock().await[&started.flow_id].base.region, region);
        }
    }
    #[test]
    fn trae_exchange_credentials_keep_successful_origin_and_distinct_tokens() {
        for (exchange_origin, previous_realm, expected_realm) in [
            (
                "https://api.trae.com.cn/cloudide",
                Some("https://api.trae.cn"),
                "https://api.trae.com.cn",
            ),
            (
                "https://api.trae.com.cn/cloudide",
                None,
                "https://api.trae.com.cn",
            ),
            (
                "https://api.trae.cn/",
                Some("https://api.trae.com.cn"),
                "https://api.trae.cn",
            ),
        ] {
            let mut base = empty_credentials();
            base.access_token = "old-session".into();
            base.extra["clientId"] = json!(traework::WORK_CLIENT_ID);
            base.extra["userJwt"] = json!("old-identity");
            if let Some(realm) = previous_realm {
                base.extra["apiBase"] = json!(realm);
            }
            let response = json!({"Result": {
                "Token": "new-session",
                "RefreshToken": "new-refresh",
                "UserJwt": "new-identity"
            }});
            let c = trae_exchange_credentials(&response, &base, exchange_origin)
                .expect("parse exchanged credentials");
            assert_eq!(c.extra["apiBase"], expected_realm);
            assert_eq!(c.access_token, "new-session");
            assert_eq!(c.refresh_token.as_deref(), Some("new-refresh"));
            assert_eq!(c.extra["userJwt"], "new-identity");
            assert_eq!(c.extra["clientId"], traework::WORK_CLIENT_ID);
        }
    }

    #[test]
    fn login_url_has_exact_domain_boundary() {
        assert!(validate_browser_url("https://www.trae.cn/authorization", &["trae.cn"]).is_ok());
        assert!(validate_browser_url("https://trae.cn.attacker.test/", &["trae.cn"]).is_err());
        assert!(validate_browser_url("http://trae.cn/", &["trae.cn"]).is_err());
    }
    #[test]
    fn trae_authorization_url_keeps_loopback_callback_raw_and_ordered() {
        let url = trae_authorization_url(
            "https://www.trae.cn/",
            CreditRegion::Cn,
            "ono9krqynydwx5",
            "flow-1",
            "http://127.0.0.1:51724/authorize",
            "machine",
            "device",
            "challenge",
            "My Mac",
            "mac",
            "15.5",
        );
        // The page hard-validates ^http://127.0.0.1:(\d+)/authorize$ on the raw
        // value, so the callback must appear unencoded and the path exactly /authorize.
        assert!(url.starts_with("https://www.trae.cn/authorization?login_version=1&auth_from=trae&login_channel=native_ide&"));
        assert!(url.contains("&auth_callback_url=http://127.0.0.1:51724/authorize&"));
        assert!(url.contains("&state=flow-1&"));
        assert!(!url.contains("%2Fauthorize"));
        assert!(url.contains("x_device_brand=My%20Mac"));
        assert!(url.contains("x_os_version=15.5"));
        assert!(url.contains("x_env=&x_app_version=3.3.100&x_app_type=stable&"));
        assert!(url.ends_with("&code_challenge_method=S256&channel_name=common"));
    }
    #[test]
    fn international_login_uses_official_3613_versions_without_changing_cn() {
        let url = trae_authorization_url("https://www.trae.ai", CreditRegion::Intl,
            traework::WORK_CLIENT_ID, "fixture", "http://127.0.0.1:12345/authorize",
            "machine", "device", "challenge", "host", "mac", "15");
        assert!(url.contains("plugin_version=2.3.99397&"));
        assert!(url.contains("x_app_version=3.6.13&"));
        assert!(!url.contains("3.3.100"));
        assert_eq!(traework::page_app_version(CreditRegion::Cn), "3.3.100");
    }

    #[test]
    fn auth_code_is_extracted_from_info_payload() {
        let info = r#"{"AuthCode":"67kxTwHtVFLAoPuEDWmkDsCmGAqdObVTguECE-03LQ4","State":"s1"}"#;
        assert_eq!(
            auth_code_from_info(info).as_deref(),
            Some("67kxTwHtVFLAoPuEDWmkDsCmGAqdObVTguECE-03LQ4")
        );
        let encoded = percent_encode(info);
        assert_eq!(
            auth_code_from_info(&encoded).as_deref(),
            Some("67kxTwHtVFLAoPuEDWmkDsCmGAqdObVTguECE-03LQ4")
        );
        assert_eq!(auth_code_from_info("{\"AuthCode\":\"\"}"), None);
        assert_eq!(auth_code_from_info("not-json-at-all"), None);
    }
    #[test]
    fn trae_identity_is_derived_deterministically_per_account() {
        let first = trae_derived_identity("100001").unwrap();
        let again = trae_derived_identity("100001").unwrap();
        assert_eq!(first, again);
        assert_ne!(first.0, trae_derived_identity("100002").unwrap().0);
        // Device ids are 16 digits led by 1-9; machine ids are dashed v4 UUIDs.
        assert_eq!(first.0.len(), 16);
        assert!(first.0.as_bytes()[0].is_ascii_digit() && first.0.as_bytes()[0] != b'0');
        assert_eq!(first.1.len(), 36);
        assert_eq!(&first.1[14..15], "4");
        assert!(matches!(&first.1[19..20], "8" | "9" | "a" | "b"));
        assert!(first.2.starts_with("-----BEGIN PUBLIC KEY-----"));
        assert!(first.3.starts_with("-----BEGIN PRIVATE KEY-----"));
    }
    #[test]
    fn callback_identity_reads_uid_through_one_quoting_layer() {
        let plain = r#"{"UserID":"42","ScreenName":"名"}"#;
        assert_eq!(
            trae_callback_identity(plain).map(|(uid, _)| uid),
            Some("42".into())
        );
        let quoted = serde_json::to_string(plain).unwrap();
        assert_eq!(
            trae_callback_identity(&quoted).map(|(uid, _)| uid),
            Some("42".into())
        );
        assert!(trae_callback_identity("{\"other\":1}").is_none());
    }
    #[test]
    fn pkce_is_s256_and_long_enough() {
        let (v, c) = pkce();
        assert_eq!(v.len(), 64);
        assert_eq!(c, URL_SAFE_NO_PAD.encode(Sha256::digest(v)));
    }
    #[tokio::test]
    async fn callback_handles_fragmented_request_line() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let result = Arc::new(tokio::sync::Mutex::new(None));
        let done = tokio_util::sync::CancellationToken::new();
        let task = tokio::spawn(listen_callback(
            listener,
            "/authorize/probe".into(),
            result.clone(),
            now_ms() + 5000,
            "current-flow".into(),
            done.clone(),
        ));
        let mut socket = tokio::net::TcpStream::connect(addr).await.unwrap();
        socket
            .write_all(b"GET /authorize/probe?authCode=frag")
            .await
            .unwrap();
        tokio::task::yield_now().await;
        socket
            .write_all(format!("mented HTTP/1.1\r\nHost: {addr}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut response = String::new();
        socket.read_to_string(&mut response).await.unwrap();
        done.cancel();
        task.await.unwrap();
        assert!(response.starts_with("HTTP/1.1 202"));
        let callback = result.lock().await.clone().unwrap().unwrap();
        assert_eq!(callback["authCode"], "fragmented");
    }

    #[tokio::test]
    async fn unbound_callbacks_do_not_stop_or_replace_a_pending_login() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let pending = Arc::new(tokio::sync::Mutex::new(None));
        let done = tokio_util::sync::CancellationToken::new();
        let task = tokio::spawn(listen_callback(
            listener,
            "/authorize".into(),
            pending.clone(),
            now_ms() + 5000,
            "current-flow".into(),
            done.clone(),
        ));
        async fn request(address: std::net::SocketAddr, query: &str, host: &str) -> String {
            let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
            socket
                .write_all(
                    format!("GET /authorize?{query} HTTP/1.1\r\nHost: {host}\r\n\r\n").as_bytes(),
                )
                .await
                .unwrap();
            let mut response = String::new();
            socket.read_to_string(&mut response).await.unwrap();
            response
        }
        for query in [
            "error=cancel",
            "refreshToken=synthetic",
            "refreshToken=synthetic&state=old-flow",
            "code=ignored&state=a&state=b",
        ] {
            assert!(request(address, query, &address.to_string())
                .await
                .starts_with("HTTP/1.1 400"));
            assert!(pending.lock().await.is_none());
        }
        assert!(request(address, "code=ignored", "other.example")
            .await
            .starts_with("HTTP/1.1 400"));
        assert!(request(address, "code=candidate-one", &address.to_string())
            .await
            .starts_with("HTTP/1.1 202"));
        assert!(
            request(address, "code=cannot-replace", &address.to_string())
                .await
                .starts_with("HTTP/1.1 400")
        );
        assert_eq!(
            pending.lock().await.take().unwrap().unwrap()["code"],
            "candidate-one"
        );
        // Consuming a failed PKCE candidate leaves the listener available for the real browser.
        assert!(request(address, "code=candidate-two", &address.to_string())
            .await
            .starts_with("HTTP/1.1 202"));
        assert_eq!(
            pending.lock().await.take().unwrap().unwrap()["code"],
            "candidate-two"
        );
        assert!(request(
            address,
            "refreshToken=synthetic&state=current-flow",
            &address.to_string()
        )
        .await
        .starts_with("HTTP/1.1 202"));
        done.cancel();
        task.await.unwrap();
    }
}
