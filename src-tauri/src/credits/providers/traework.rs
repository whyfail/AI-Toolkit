use super::*;
pub(super) const API: &str = "https://api.trae.cn";
pub(super) const CLIENT_ID: &str = "en1oxy7wnw8j9n";
/// The real Trae IDE OAuth client (TraeWorkAssistant packet capture, MIT).
/// The SOLO client id (CLIENT_ID) stalls the authorization page after the
/// billing step and never redirects back.
pub(crate) const WORK_CLIENT_ID: &str = "ono9krqynydwx5";
/// Authorization-page snapshot values of the real IDE client.
pub(crate) const PAGE_PLUGIN_VERSION: &str = "2.3.83560";
pub(crate) const PAGE_APP_VERSION: &str = "3.3.100";
pub(crate) const INTL_APP_VERSION: &str = "3.6.13";
pub(crate) fn page_app_version(region: CreditRegion) -> &'static str {
    match region { CreditRegion::Cn => PAGE_APP_VERSION, CreditRegion::Intl => INTL_APP_VERSION }
}
pub(crate) fn page_plugin_version(region: CreditRegion) -> &'static str {
    match region { CreditRegion::Cn => PAGE_PLUGIN_VERSION, CreditRegion::Intl => "2.3.99397" }
}
pub(super) const INTL_API: &str = "https://growsg-normal.trae.ai";
pub(super) fn authorization_origin(region: CreditRegion) -> &'static str {
    match region {
        CreditRegion::Cn => "https://www.trae.cn",
        CreditRegion::Intl => "https://www.trae.ai",
    }
}
// Public account realms from the official international 3.6.13 bootConfig.
// Validate complete origins so an imported host cannot cross editions or add a path.
pub(super) fn allowed_account_origin(region: CreditRegion, raw: &str) -> Option<&'static str> {
    let url = reqwest::Url::parse(raw).ok()?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return None;
    }
    match (region, url.host_str()?) {
        (CreditRegion::Cn, "api.trae.cn" | "www.trae.cn" | "trae.cn") => Some(API),
        (CreditRegion::Cn, "api.trae.com.cn" | "www.trae.com.cn" | "trae.com.cn") => {
            Some("https://api.trae.com.cn")
        }
        (CreditRegion::Intl, "www.trae.ai" | "trae.ai" | "growsg-normal.trae.ai") => Some(INTL_API),
        (CreditRegion::Intl, "grow-normal.trae.ai") => Some("https://grow-normal.trae.ai"),
        (CreditRegion::Intl, "grow-normal.traeapi.us") => Some("https://grow-normal.traeapi.us"),
        _ => None,
    }
}
// Imported credentials must not send a session to another edition's gateway.
fn validate_origins(c: &Credentials) -> Result<(), ProviderError> {
    for key in ["apiBase", "ugApiBase", "aiApiBase"] {
        let Some(value) = c.extra.get(key) else { continue };
        let raw = value.as_str().ok_or_else(|| ProviderError::new(400, "TRAE 账号域格式无效"))?;
        let url = reqwest::Url::parse(raw).map_err(|_| ProviderError::new(400, "TRAE 账号域格式无效"))?;
        #[cfg(test)]
        if url.scheme() == "http" && url.host_str() == Some("127.0.0.1") {
            continue;
        }
        let valid_origin = url.scheme() == "https" && url.username().is_empty()
            && url.password().is_none() && url.port().is_none() && url.path() == "/"
            && url.query().is_none() && url.fragment().is_none();
        let host = url.host_str().unwrap_or("");
        let allowed = match (c.region, key) {
            (CreditRegion::Cn, "apiBase") => allowed_account_origin(c.region, raw).is_some(),
            (CreditRegion::Intl, "apiBase") => allowed_account_origin(c.region, raw).is_some(),
            (CreditRegion::Cn, "ugApiBase") => host == "api.trae.cn",
            (CreditRegion::Intl, "ugApiBase") => matches!(host, "growsg-normal.trae.ai" | "growva-normal.trae.ai" | "grow-normal.traeapi.us"),
            (CreditRegion::Cn, "aiApiBase") => matches!(host, "trae-api-cn.mchost.guru" | "api.trae.cn" | "api.trae.com.cn"),
            (CreditRegion::Intl, "aiApiBase") => matches!(host, "coresg-normal.trae.ai" | "coreva-normal.trae.ai" | "core-normal.traeapi.us"),
            _ => false,
        };
        if !valid_origin || !allowed {
            return Err(ProviderError::new(400, "TRAE 账号域与所选区域不匹配，请重新导入该版本的凭据"));
        }
    }
    Ok(())
}

fn international_protocol_pending() -> ProviderError {
    ProviderError::new(501, "TRAE 国际版模型目录与代理尚未启用：新版请求协议待验证；登录与额度查询可用")
}

fn normalized_region(value: &str) -> Option<&'static str> {
    match value.to_ascii_uppercase().as_str() {
        "SG" | "SINGAPORE-CENTRAL" => Some("SG"),
        "US" | "US-EAST" | "VA" => Some("US"),
        "USTTP" => Some("USTTP"),
        _ => None,
    }
}
fn region_field(value: &Value, paths: &[&str]) -> Option<&'static str> {
    paths.iter().find_map(|path| {
        value
            .pointer(path)
            .and_then(Value::as_str)
            .and_then(normalized_region)
    })
}
fn upstream_region(c: &Credentials) -> &'static str {
    region_field(
        &c.extra,
        &[
            "/upstreamRegion",
            "/userRegion/_aiRegion",
            "/userRegion/region",
            "/account/uidRegion",
            "/account/storeRegion",
        ],
    )
    .unwrap_or("SG")
}
pub(super) fn apply_user_region(c: &mut Credentials, user: &Value) {
    if c.region != CreditRegion::Intl {
        return;
    }
    let tag = text(user, &["UserTag", "userTag"])
        .or_else(|| text(&c.extra, &["userTag"]))
        .or_else(|| text(&c.extra["account"], &["userTag"]));
    let store = region_field(
        user,
        &["/StoreRegion", "/storeRegion", "/account/storeRegion"],
    )
    .or_else(|| region_field(&c.extra, &["/account/storeRegion", "/storeRegion"]))
    .or_else(|| {
        (tag.as_deref() == Some("usttp") || api_base(c) == "https://grow-normal.traeapi.us")
            .then_some("USTTP")
    });
    let ai = region_field(
        user,
        &[
            "/AIRegion",
            "/aiRegion",
            "/userRegion/_aiRegion",
            "/userRegion/region",
            "/account/uidRegion",
        ],
    )
    .or_else(|| {
        region_field(
            &c.extra,
            &[
                "/userRegion/_aiRegion",
                "/userRegion/region",
                "/account/uidRegion",
            ],
        )
    })
    .or(store)
    .unwrap_or("SG");
    let routing = if store == Some("USTTP") { "USTTP" } else { ai };
    c.extra["upstreamRegion"] = json!(routing);
    c.extra["userRegion"] = json!({"region": ai, "_aiRegion": ai});
    if let Some(store) = store {
        c.extra["storeRegion"] = json!(store);
    }
    if let Some(tag) = tag {
        c.extra["userTag"] = json!(tag);
    }
    if routing == "USTTP" {
        c.extra["apiBase"] = json!("https://grow-normal.traeapi.us");
    } else if c.extra.get("apiBase").is_none() {
        c.extra["apiBase"] = json!(INTL_API);
    }
    c.extra["ugApiBase"] = json!(match routing {
        "US" => "https://growva-normal.trae.ai",
        "USTTP" => "https://grow-normal.traeapi.us",
        _ => INTL_API,
    });
    c.extra["aiApiBase"] = json!(match routing {
        "US" => "https://coreva-normal.trae.ai",
        "USTTP" => "https://core-normal.traeapi.us",
        _ => "https://coresg-normal.trae.ai",
    });
}
// Keep account, token refresh and billing calls on the account's own realm.
pub(super) fn api_base(c: &Credentials) -> String {
    text(&c.extra, &["apiBase"]).unwrap_or_else(|| {
        match c.region {
            CreditRegion::Cn => API,
            CreditRegion::Intl => INTL_API,
        }
        .into()
    })
}
// Check-in uses the IDE boot config's separate ugApi gateway. The .com.cn
// account realm accepts billing calls but returns 404 for check-in endpoints.
fn ug_api_base(c: &Credentials) -> String {
    text(&c.extra, &["ugApiBase"]).unwrap_or_else(|| {
        match c.region {
            CreditRegion::Cn => API,
            CreditRegion::Intl => match upstream_region(c) {
                "US" => "https://growva-normal.trae.ai",
                "USTTP" => "https://grow-normal.traeapi.us",
                _ => INTL_API,
            },
        }
        .into()
    })
}
/// The AI plane (model catalog + agent chat) is served by a dedicated gateway:
/// the TLB cutover moved it off api.trae.cn, which serves account/billing/UG
/// APIs. The TRAE IDE resolves api/ide/* to this host at runtime. Other realms
/// can override it with the aiApiBase credential extra.
pub(super) const AI_API: &str = "https://trae-api-cn.mchost.guru";
pub(super) fn ai_api_base(c: &Credentials) -> String {
    text(&c.extra, &["aiApiBase"]).unwrap_or_else(|| {
        match c.region {
            CreditRegion::Cn => AI_API,
            CreditRegion::Intl => match upstream_region(c) {
                "US" => "https://coreva-normal.trae.ai",
                "USTTP" => "https://core-normal.traeapi.us",
                _ => "https://coresg-normal.trae.ai",
            },
        }
        .into()
    })
}
pub(super) fn headers(r: RequestBuilder, c: &Credentials, chat: bool) -> RequestBuilder {
    let mut r = r
        .header("Authorization", format!("Cloud-IDE-JWT {}", c.access_token))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json");
    if let Some(d) = &c.device_id {
        r = r.header("X-Device-Id", d);
    }
    if !chat
        && (c.region == CreditRegion::Intl
            || text(&c.extra, &["credentialSource"]).as_deref() == Some("desktop"))
    {
        return r
            .header("User-Agent", format!("Trae/{}", ide_version(c)))
            .header(
                "X-User-Region",
                if c.region == CreditRegion::Cn {
                    "CN"
                } else {
                    upstream_region(c)
                },
            );
    }
    if chat {
        r = r
            .header("User-Agent", "Trae/0.1.61")
            .header("X-Cloudide-Token", &c.access_token)
            .header("X-Ide-Token", &c.access_token)
            .header("X-Uid", c.user_id.as_deref().unwrap_or(""))
            .header("X-App-Id", "6eefa01c-1036-4c7e-9ca5-d891f63bfcd8")
            .header("X-App-Version", "default")
            .header("X-Ide-Version", "0.1.61")
            .header("X-Ide-Version-Code", "20260820")
            .header("X-App-Version-Code", "20260820")
            .header("X-Ide-Version-Type", "stable")
            .header("X-Device-Type", "windows")
            .header("X-OS-Version", "Windows 11 Pro")
            .header("X-Device-Brand", "83DG")
            .header("Request-Traffic-Type", "prod");
        if let Some(m) = text(&c.extra, &["machineId"]) {
            r = r.header("X-Machine-Id", m);
        }
    } else {
        let rid = uuid::Uuid::new_v4();
        r = r
            .header("User-Agent", "VSCode 1.107.1 (TRAE SOLO CN)")
            .header("X-User-Region", "CN")
            .header("Accept-Language", "zh-CN")
            .header("Package-Type", "stable_cn")
            .header("X-Market-Client-Id", "VSCode 1.107.1")
            .header("X-Lgw-Req-Sdk-Type", "3")
            .header("X-Device-Brand", "83DG")
            .header("X-Device-Type", "windows")
            .header("X-OS-Version", "Windows 11 Pro")
            .header("App-Version", "0.1.61")
            .header("X-Request-Id", rid.to_string())
            .header(
                "X-TT-Trace-Id",
                format!(
                    "00-{}-{}-01",
                    uuid::Uuid::new_v4().simple(),
                    &rid.simple().to_string()[..16]
                ),
            )
            .header("Sec-Fetch-Dest", "empty")
            .header("Sec-Fetch-Mode", "no-cors")
            .header("Sec-Fetch-Site", "none");
    }
    r
}
// The TRAE Work CLI stores its login in the system keychain (service
// "trae_cli", account "auth_info") as `go-keyring-base64:<base64 json>` with
// the bound device id, console host and OAuth tokens of the local app.
#[cfg(test)]
pub(crate) fn local_credentials() -> Result<Option<Credentials>, String> {
    local_credentials_for_region(CreditRegion::Cn)
}
pub(crate) fn local_credentials_for_region(
    region: CreditRegion,
) -> Result<Option<Credentials>, String> {
    if let Some(credentials) = super::trae_desktop::local_credentials_for_region(region)? {
        return Ok(Some(credentials));
    }
    // The legacy CLI keychain service has no edition discriminator. Never import
    // it for an international account where it could select the CN session.
    if region == CreditRegion::Intl {
        return Ok(None);
    }
    let entry =
        keyring::Entry::new("trae_cli", "auth_info").map_err(|_| "系统凭据库不可用".to_string())?;
    let read = || -> Result<Option<String>, String> {
        match entry.get_password() {
            Ok(raw) => Ok(Some(raw)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err("无法读取钥匙串中的 trae_cli 凭据，请在系统弹窗中允许访问".into()),
        }
    };
    #[cfg(target_os = "macos")]
    let raw = super::super::secrets::macos::access(true, read)?;
    #[cfg(not(target_os = "macos"))]
    let raw = read()?;
    let Some(raw) = raw else {
        return Ok(None);
    };
    let encoded = raw
        .strip_prefix("go-keyring-base64:")
        .ok_or("trae_cli 凭据格式无法识别")?;
    use base64::Engine;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "trae_cli 凭据解码失败".to_string())?;
    let v: Value =
        serde_json::from_slice(&decoded).map_err(|_| "trae_cli 凭据内容无法解析".to_string())?;
    // Older CLI versions may store the token fields at the top level instead
    // of an OauthToken object; log the observed shape when neither matches.
    let token = v
        .get("OauthToken")
        .or_else(|| v.get("OAuthToken"))
        .unwrap_or(&Value::Null);
    let mut c = empty_credentials();
    let access = text(token, &["Token", "AccessToken", "accessToken"])
        .or_else(|| text(&v, &["Token", "AccessToken", "accessToken"]));
    let Some(access) = access else {
        return Err("trae_cli 凭据缺少访问令牌".into());
    };
    c.access_token = access;
    c.refresh_token = text(token, &["RefreshToken", "refresh_token"])
        .or_else(|| text(&v, &["RefreshToken", "refresh_token"]));
    if let Some(device) = text(&v, &["DeviceId", "deviceId"]) {
        c.device_id = Some(device);
    }
    if let Some(host) = text(&v, &["Host", "LoginBaseURL"]) {
        c.extra["apiBase"] = json!(host);
    }
    c.extra["credentialSource"] = json!("cli");
    c.expires_at = token.get("TokenExpireAt").and_then(epoch);
    c.user_id = text(&v, &["UserID", "UserId", "userId", "user_id"]);
    if let Some(user_jwt) = text(token, &["UserJwt", "userJwt"]) {
        c.extra["userJwt"] = json!(user_jwt);
    }
    if let Some(client_id) = text(&v, &["ClientID", "ClientId", "clientId"]) {
        c.extra["clientId"] = json!(client_id);
    }
    Ok(Some(c))
}

fn identity_headers(r: RequestBuilder, c: &Credentials) -> RequestBuilder {
    let r = r
        .header("x-cloudide-token", &c.access_token)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json");
    if let Some(device) = &c.device_id {
        r.header("X-Device-Id", device)
    } else {
        r
    }
}
fn ide_version(c: &Credentials) -> String {
    text(&c.extra, &["ideVersion"]).unwrap_or_else(|| {
        match c.region {
            CreditRegion::Cn => PAGE_APP_VERSION,
            CreditRegion::Intl => INTL_APP_VERSION,
        }
        .into()
    })
}

pub(super) async fn authenticate(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<(String, String, Credentials), ProviderError> {
    validate_origins(c)?;
    // The desktop identity API accepts the Cloud IDE session in this header;
    // its separate SSO UserJwt must never replace the session used for billing.
    let v = client
        .json(
            identity_headers(
                client
                    .http
                    .post(format!("{}/cloudide/api/v3/trae/GetUserInfo", api_base(c))),
                c,
            )
            .json(&json!({"ReqSource": "IDE", "IDEVersion": ide_version(c)})),
        )
        .await?;
    let d = data(&v);
    let uid = text(d, &["UserID", "UserId", "user_id", "uid"])
        .ok_or_else(|| ProviderError::new(502, "TRAE 未返回账号身份"))?;
    let label = text(d, &["ScreenName", "Nickname", "nickname", "name"]).unwrap_or_else(|| {
        format!(
            "TRAE {}{uid}",
            if c.region == CreditRegion::Cn {
                "CN "
            } else {
                ""
            }
        )
    });
    let mut credentials = c.clone();
    credentials.user_id = Some(uid.clone());
    apply_user_region(&mut credentials, d);
    let status = client
        .json(
            identity_headers(
                client
                    .http
                    .post(format!("{}/cloudide/api/v3/trae/CheckLogin", api_base(c))),
                c,
            )
            .json(&json!({"GetAIPayHost": true})),
        )
        .await;
    if let Ok(status) = status {
        let d = data(&status);
        if flag(d, &["IsLogin"]) == Some(false) {
            return Err(http_error(401));
        }
        if credentials.device_id.is_none() {
            credentials.device_id = text(d, &["BoundDeviceID", "DeviceID"]);
        }
    }
    Ok((uid, label, credentials))
}
pub(super) async fn refresh(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<Credentials, ProviderError> {
    validate_origins(c)?;
    let rt = c
        .refresh_token
        .as_ref()
        .ok_or_else(|| ProviderError::new(401, "Trae 凭证没有刷新令牌，请重新登录"))?;
    let cid = text(&c.extra, &["clientId"]).unwrap_or_else(|| {
        if text(&c.extra, &["credentialSource"]).as_deref() == Some("cli") {
            CLIENT_ID.into()
        } else {
            WORK_CLIENT_ID.into()
        }
    });
    let version = ide_version(c);
    let platform = text(&c.extra, &["platformCode"]).unwrap_or_else(|| {
        if cid == CLIENT_ID {
            "SOLO_PC"
        } else {
            "IDE_PC"
        }
        .into()
    });
    if let (Some(private), Some(public), Some(device), Some(machine)) = (
        text(&c.extra, &["devicePrivateKey"]),
        text(&c.extra, &["devicePublicKey"]),
        c.device_id.clone(),
        text(&c.extra, &["machineId"]),
    ) {
        use base64::Engine;
        use p256::{
            ecdsa::{signature::Signer, Signature, SigningKey},
            pkcs8::DecodePrivateKey,
        };
        let key = SigningKey::from_pkcs8_pem(&private)
            .map_err(|_| ProviderError::new(400, "Trae 保存的设备密钥无效，请重新授权"))?;
        let ts = now_ms() / 1000;
        let nonce = uuid::Uuid::new_v4().simple().to_string();
        let path = "/trae/api/v3/oauth/ExchangeToken";
        let payload = format!("POST\n{path}\n{cid}\n{rt}\n{ts}\n{nonce}");
        let signature: Signature = key.sign(payload.as_bytes());
        let device = json!({"DeviceID":device,"MachineID":machine,"PlatformCode":platform,"DeviceType":"PC","DeviceName":"","DeviceModel":"","ClientVersion":version,"DevicePublicKey":public,"DeviceBrand":"","DeviceCPU":"","OSInfo":"","OSVersion":""});
        let mut request = client
            .http
            .post(format!("{}{path}", api_base(c)))
            .header("User-Agent", format!("Trae/{version}"));
        if !c.access_token.is_empty() {
            request = request
                .header("x-cloudide-token", &c.access_token)
                .header("Authorization", format!("Cloud-IDE-JWT {}", c.access_token));
        }
        let v=client.json(request.json(&json!({"ClientID":cid,"ClientSecret":"","RefreshToken":rt,"DeviceInfo":device,"DeviceProof":{"Signature":base64::engine::general_purpose::STANDARD.encode(signature.to_der().as_bytes()),"Timestamp":ts,"Nonce":nonce},"IDEVersion":version}))).await?;
        return refreshed_credentials(data(&v), c);
    }
    let v = client
        .json(
            client
                .http
                .post(format!(
                    "{}/cloudide/api/v3/trae/oauth/ExchangeToken",
                    api_base(c)
                ))
                .header("User-Agent", format!("Trae/{version}"))
                .json(&json!({"ClientID":cid,"RefreshToken":rt,"ClientSecret":"-","UserID":""})),
        )
        .await?;
    refreshed_credentials(data(&v), c)
}
fn refreshed_credentials(v: &Value, c: &Credentials) -> Result<Credentials, ProviderError> {
    let mut updated = credentials(
        v,
        c,
        &["AccessToken", "accessToken", "Token", "token"],
        &["RefreshToken", "refreshToken"],
    )?;
    if let Some(user_jwt) = text(v, &["UserJwt", "userJwt"]) {
        updated.extra["userJwt"] = json!(user_jwt);
    } else if let Some(extra) = updated.extra.as_object_mut() {
        extra.remove("userJwt");
    }
    Ok(updated)
}

fn parse_packs(v: &Value) -> (Vec<CreditPack>, Option<String>) {
    let d = data(v);
    let mut packs = vec![];
    let mut quota = vec![];
    if let Some(ps) = d["user_entitlement_pack_list"].as_array() {
        for (i, p) in ps.iter().enumerate() {
            let b = &p["entitlement_base_info"];
            let q = [
                "/quota",
                "/product_extra/subscription_extra/quota",
                "/product_extra/package_extra/quota",
            ]
            .iter()
            .find_map(|s| {
                b.pointer(s)
                    .filter(|q| q.is_object() && q.as_object().is_some_and(|o| !o.is_empty()))
            })
            .unwrap_or(&Value::Null);
            let usage = &p["usage"];
            let limit = num(q, &["credits_limit"]);
            let used = num(usage, &["credits_amount"]);
            if limit.is_some() {
                packs.push(CreditPack {
                    id: text(b, &["id", "entitlement_id"]).unwrap_or(i.to_string()),
                    name: text(p, &["display_desc"]).unwrap_or("Trae 积分包".into()),
                    remaining: limit
                        .filter(|n| *n >= 0.0)
                        .zip(used)
                        .map(|(n, u)| (n - u).max(0.0).round()),
                    expires_at: b.get("end_time").and_then(epoch),
                    unit: "credits".into(),
                });
            }
            if let Some(l) = num(q, &["premium_model_fast_request_limit"]) {
                quota.push(if l < 0.0 {
                    "快请求：不限".into()
                } else if let Some(used) = num(usage, &["premium_model_fast_amount"]) {
                    format!("快请求剩余：{} 次", (l - used).max(0.0))
                } else {
                    format!("快请求剩余未知（上限 {l} 次）")
                });
            }
            if let Some(l) = num(q, &["basic_usage_limit"]) {
                quota.push(if let Some(used) = num(usage, &["basic_usage_amount"]) {
                    format!("基础配额剩余：{}", (l - used).max(0.0))
                } else {
                    format!("基础配额剩余未知（上限 {l}）")
                });
            }
        }
    }
    (packs, (!quota.is_empty()).then(|| quota.join("；")))
}
fn parse_international_packs(v: &Value) -> (Vec<CreditPack>, Option<String>) {
    let d = data(v);
    let dollars = flag(d, &["is_dollar_usage_billing"]) == Some(true);
    let mut packs = vec![];
    let mut notes = vec![];
    for (index, p) in d["user_entitlement_pack_list"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let base = &p["entitlement_base_info"];
        let quota = [
            "/quota",
            "/product_extra/subscription_extra/quota",
            "/product_extra/package_extra/quota",
        ]
        .iter()
        .find_map(|path| {
            base.pointer(path)
                .filter(|q| q.as_object().is_some_and(|o| !o.is_empty()))
        })
        .unwrap_or(&Value::Null);
        let usage = &p["usage"];
        let id = text(base, &["id", "entitlement_id"]).unwrap_or_else(|| index.to_string());
        let name = text(p, &["display_desc"]).unwrap_or_else(|| "TRAE 额度包".into());
        let fields: &[(&str, &str, &str, &str)] = if dollars {
            &[
                ("basic_usage_limit", "basic_usage_amount", "基础额度", "usd"),
                ("bonus_usage_limit", "bonus_usage_amount", "奖励额度", "usd"),
            ]
        } else {
            &[(
                "premium_model_fast_request_limit",
                "premium_model_fast_amount",
                "快请求",
                "requests",
            )]
        };
        for (limit_key, usage_key, label, unit) in fields {
            if *limit_key == "bonus_usage_limit" && flag(quota, &["no_bonus_quota"]) == Some(true) {
                continue;
            }
            let Some(limit) = num(quota, &[*limit_key]).filter(|n| n.is_finite()) else {
                continue;
            };
            let used = num(usage, &[*usage_key]).filter(|n| n.is_finite() && *n >= 0.0);
            packs.push(CreditPack {
                id: format!("{id}:{limit_key}"),
                name: format!("{name} · {label}"),
                unit: (*unit).into(),
                remaining: (limit >= 0.0)
                    .then_some(limit)
                    .zip(used)
                    .map(|(l, u)| (l - u).max(0.0)),
                expires_at: base.get("end_time").and_then(epoch),
            });
            if limit < 0.0 {
                notes.push(format!("{label}：不限"));
            }
        }
    }
    (packs, (!notes.is_empty()).then(|| notes.join("；")))
}
fn usage_snapshot(v: &Value, c: &Credentials) -> ProviderSnapshot {
    let (packs, quota) = match c.region {
        CreditRegion::Cn => parse_packs(v),
        CreditRegion::Intl => parse_international_packs(v),
    };
    let mut result = snapshot(packs, vec![], vec![], None);
    result.quota = quota;
    if c.region == CreditRegion::Intl {
        if result.packs.is_empty() {
            result.unit = match flag(data(v), &["is_dollar_usage_billing"]) {
                Some(true) => "usd",
                Some(false) => "requests",
                None => "unknown",
            }
            .into();
        }
    } else if let Some(balance) = parse_credit_balance(v) {
        result.balance = Some(balance);
    } else if result.packs.is_empty()
        && flag(data(v), &["is_credits_billing"]) == Some(true)
        && flag(data(v), &["is_dollar_usage_billing"]) != Some(true)
    {
        result.balance = Some(0.0);
    }
    result
}
async fn fetch_usage(client: &ProviderClient, c: &Credentials) -> Result<Value, ProviderError> {
    validate_origins(c)?;
    let (origin, path, body) = match c.region {
        CreditRegion::Cn => (
            api_base(c),
            "/trae/api/v2/pay/ide_user_ent_usage",
            json!({"require_usage":true,"req_source":1}),
        ),
        CreditRegion::Intl => (
            ug_api_base(c),
            "/trae/api/v1/pay/ide_user_ent_usage",
            json!({"require_usage":true}),
        ),
    };
    client
        .json(headers(client.http.post(format!("{origin}{path}")), c, false).json(&body))
        .await
}
fn parse_credit_balance(v: &Value) -> Option<f64> {
    let d = data(v);
    if flag(d, &["is_credits_billing"]) != Some(true)
        || flag(d, &["is_dollar_usage_billing"]) == Some(true)
    {
        return None;
    }
    let summary = &d["usage_summary"];
    let total = num(summary, &["total_amount"])?;
    let consumed = num(summary, &["consumed_amount"])?;
    (total.is_finite() && consumed.is_finite() && total >= 0.0 && consumed >= 0.0)
        .then(|| (total - consumed).max(0.0))
}
pub(super) fn parse_models(v: &Value) -> Vec<ModelInfo> {
    data(v)["config_info_list"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let id = text(m, &["config_name"])?;
            let name = text(&m["display_config"], &["display_name"])?;
            if flag(m, &["config_switch"]) == Some(false)
                || flag(m, &["is_invisible_to_user"]) == Some(true)
            {
                return None;
            }
            let contact = m["display_contact_config"]
                .as_str()
                .and_then(|config| serde_json::from_str::<Value>(config).ok())
                .unwrap_or(Value::Null);
            let upstream_model_ids = m["model_detail_list"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|detail| underlying_model_id(&detail["model_name"], &id))
                .fold(Vec::new(), |mut ids, id| {
                    if !ids.contains(&id) {
                        ids.push(id);
                    }
                    ids
                });
            Some(ModelInfo {
                upstream_model_ids,
                // A hidden multimodal badge is not a declaration that the model
                // cannot read images. Keep it unknown unless support is advertised.
                supports_images: flag(&contact["multimodal"], &["enable"])
                    .filter(|supported| *supported)
                    .or_else(|| {
                        flag(&m["display_config"], &["multimodal"]).filter(|supported| *supported)
                    }),
                supports_reasoning: flag(&contact["reasoning"], &["enable"])
                    .filter(|supported| *supported),
                reasoning_effort_options: effort_options(&m["reasoning_effort_config"]["options"]),
                default_reasoning_effort: text(&m["reasoning_effort_config"], &["default_level"]),
                ..model(
                    id,
                    name,
                    token_limit(&m["context_window_tokens"], &["max", "dev"]),
                )
            })
        })
        .collect()
}
pub(super) async fn models(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<Vec<ModelInfo>, ProviderError> {
    validate_origins(c)?;
    if c.region == CreditRegion::Intl {
        return Err(international_protocol_pending());
    }
    let v=client.json(headers(client.http.post(format!("{}/api/ide/v1/get_detail_param", ai_api_base(c))),c,true).json(&json!({"function":"solo_work_lite","config_names":null,"need_prompt":false,"current_config_info":null,"poly_prompt":true,"mode_type":null}))).await?;
    let m = parse_models(&v);
    if m.is_empty() {
        return Err(ProviderError::new(
            502,
            "Trae 官方模型目录为空或当前账号无法访问",
        ));
    }
    Ok(m)
}
fn reward(v: &Value, c: &Credentials) -> Reward {
    let d = data(v);
    // did_checked_in is a device limit, not confirmation for this account.
    let claimed = flag(d, &["checked_in"]) == Some(true);
    Reward {
        id: "daily-checkin".into(),
        name: "每日签到".into(),
        kind: "credits".into(),
        status: if claimed {
            "claimed"
        } else if c.device_id.is_none() {
            "device_required"
        } else if flag(d, &["enable"]) == Some(false) || flag(d, &["did_checked_in"]) == Some(true)
        {
            "unavailable"
        } else if flag(d, &["checked_in"]) == Some(false) {
            "claimable"
        } else {
            "unknown"
        }
        .into(),
        // The official client uses extra_credits for the membership bonus hint.
        amount: num(d, &["credits"]),
        next_available_at: None,
        action_url: None,
    }
}
pub(super) async fn rewards(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<Vec<Reward>, ProviderError> {
    validate_origins(c)?;
    // Check-in is a CN entitlement. The official international client does not
    // expose it; sharing the compiled CN implementation is not eligibility.
    if c.region == CreditRegion::Intl {
        return Ok(vec![]);
    }
    let v = client
        .json(
            headers(
                client.http.post(format!(
                    "{}/trae/api/v2/ug/checkin_credits/status",
                    ug_api_base(c)
                )),
                c,
                false,
            )
            .json(&json!({"req_source":2})),
        )
        .await?;
    Ok(vec![reward(&v, c)])
}
pub(super) async fn balance(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<ProviderSnapshot, ProviderError> {
    Ok(usage_snapshot(&fetch_usage(client, c).await?, c))
}
pub(super) async fn inspect(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<ProviderSnapshot, ProviderError> {
    let (p, r, m) = tokio::join!(
        fetch_usage(client, c),
        rewards(client, c),
        models(client, c)
    );
    let p = p?;
    let mut diagnostics = vec![];
    let r = r.unwrap_or_else(|e| {
        diagnostics.push(e.message);
        vec![]
    });
    let m = m.unwrap_or_else(|e| {
        diagnostics.push(e.message);
        vec![]
    });
    let mut s = usage_snapshot(&p, c);
    s.rewards = r;
    s.models = m;
    s.diagnostic = (!diagnostics.is_empty()).then(|| diagnostics.join("；"));
    Ok(s)
}
pub(super) async fn claim(
    client: &ProviderClient,
    a: &Account,
    c: &Credentials,
) -> Result<Vec<RewardOutcome>, ProviderError> {
    let rs = rewards(client, c).await?;
    if rs.is_empty() {
        return Ok(vec![]);
    }
    let r = &rs[0];
    if r.status != "claimable" {
        return Ok(vec![outcome(
            a,
            r,
            if r.status == "claimed" {
                "already_claimed"
            } else {
                "unavailable"
            },
            "当前账号或绑定设备没有可领取权益",
            None,
        )]);
    }
    let submitted = client
        .json(
            headers(
                client.http.post(format!(
                    "{}/trae/api/v2/ug/checkin_credits/claim",
                    ug_api_base(c)
                )),
                c,
                false,
            )
            .json(&json!({"req_source":2})),
        )
        .await;
    Ok(vec![reconcile_claim(
        a,
        r,
        &submitted,
        rewards(client, c).await,
    )])
}
pub(super) fn chat_body(mut b: Value, id: &str) -> Result<Value, ProviderError> {
    let messages = b
        .get_mut("messages")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| ProviderError::new(400, "messages 必须是数组"))?;
    for m in messages {
        if let Some(s) = m["content"].as_str() {
            m["content"] = json!([{"type":"text","text":s}]);
        }
        if let Some(ts) = m.get_mut("tool_calls").and_then(Value::as_array_mut) {
            for t in ts {
                if let Some(o) = t.as_object_mut() {
                    if let Some(f) = o.remove("function") {
                        o.insert("function_call".into(), f);
                    }
                }
            }
        }
    }
    if let Some(ts) = b.get_mut("tools").and_then(Value::as_array_mut) {
        for t in ts {
            if let Some(p) = t.pointer_mut("/function/parameters") {
                if !p.is_string() {
                    *p = json!(p.to_string());
                }
            }
        }
    }
    if let Some(o) = b.as_object_mut() {
        o.retain(|k, _| {
            [
                "messages",
                "tools",
                "tool_choice",
                "temperature",
                "top_p",
                "max_tokens",
                "presence_penalty",
                "frequency_penalty",
                "seed",
                "n",
                "stop",
            ]
            .contains(&k.as_str())
        });
    }
    b["function"] = json!("solo_work_lite");
    b["config_name"] = json!(id);
    b["model"] = json!(id);
    b["stream"] = json!(true);
    if b["tool_choice"] == "none" {
        b.as_object_mut().unwrap().remove("tools");
    }
    if let Some(name) = b
        .pointer("/tool_choice/function/name")
        .and_then(Value::as_str)
        .map(str::to_owned)
    {
        b["tool_choice"] = json!(name)
    }
    Ok(b)
}
pub(super) async fn chat(
    client: &ProviderClient,
    c: &Credentials,
    m: &ModelInfo,
    body: Value,
) -> Result<ProviderChat, ProviderError> {
    validate_origins(c)?;
    if c.region == CreditRegion::Intl {
        return Err(international_protocol_pending());
    }
    stream::response(
        headers(
            client
                .http
                .post(format!("{}/api/agent/v3/llm_utils_chat", ai_api_base(c))),
            c,
            true,
        )
        .header("Accept", "text/event-stream")
        .json(&chat_body(body, &m.id)?),
        Platform::Traework,
    )
    .await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "requires explicitly selected isolated international desktop login; read-only"]
    async fn international_desktop_identity_and_billing_smoke() {
        assert_eq!(std::env::var("AI_TOOLKIT_TRAE_DIAGNOSTIC").as_deref(), Ok("read-only"));
        let root = std::path::PathBuf::from(std::env::var("AI_TOOLKIT_TRAE_PROFILE").expect("explicit isolated profile"));
        let c = super::super::trae_desktop::read_credentials_for_region(
            &root.join("User/globalStorage/storage.json"),
            Some(&json!({"appVersion": INTL_APP_VERSION})),
            CreditRegion::Intl,
        ).expect("decode isolated desktop login").expect("desktop login present");
        let client = ProviderClient::new();
        let (uid, _, authenticated) = authenticate(&client, &c).await.expect("official identity query");
        assert_eq!(authenticated.region, CreditRegion::Intl);
        assert_eq!(c.user_id.as_deref(), Some(uid.as_str()));
        let snapshot = balance(&client, &authenticated).await.expect("official international billing query");
        assert!(matches!(snapshot.unit.as_str(), "usd" | "requests"));
        assert!(snapshot.packs.iter().all(|p| matches!(p.unit.as_str(), "usd" | "requests")));
        println!("TRAE international isolated desktop identity and native-unit billing verified; no inference or claims submitted");
    }

    #[test]
    fn imported_origins_reject_cross_edition_credentials_before_network_access() {
        for (region, key, wrong) in [
            (CreditRegion::Intl, "apiBase", API),
            (CreditRegion::Intl, "aiApiBase", AI_API),
            (CreditRegion::Intl, "ugApiBase", API),
            (CreditRegion::Cn, "apiBase", INTL_API),
            (CreditRegion::Cn, "aiApiBase", "https://coresg-normal.trae.ai"),
            (CreditRegion::Intl, "apiBase", "https://growsg-normal.trae.ai.evil.test"),
        ] {
            let mut c = empty_credentials(); c.region = region; c.extra[key] = json!(wrong);
            assert!(validate_origins(&c).is_err());
        }
        let mut c = empty_credentials(); c.region = CreditRegion::Intl;
        c.extra["apiBase"] = json!("https://grow-normal.trae.ai");
        apply_user_region(&mut c, &json!({"AIRegion":"US"}));
        assert_eq!(api_base(&c), "https://grow-normal.trae.ai");
        assert!(validate_origins(&c).is_ok());
    }

    #[tokio::test]
    async fn international_models_and_chat_require_verified_official_protocol_samples() {
        let mut c = empty_credentials(); c.region = CreditRegion::Intl;
        let client = ProviderClient::new();
        assert_eq!(models(&client, &c).await.unwrap_err().status, 501);
        let m = parse_models(&json!({"config_info_list":[{"config_name":"fixture","display_config":{"display_name":"Fixture"}}]})).remove(0);
        match chat(&client, &c, &m, json!({"messages":[]})).await {
            Err(error) => assert_eq!(error.status, 501),
            Ok(_) => panic!("unverified international protocol must not be submitted"),
        }
    }

    #[test]
    fn observed_international_catalog_declares_the_stream_model_version() {
        let catalog: Value = serde_json::from_str(include_str!("fixtures/trae_intl_catalog_observed.json")).unwrap();
        let stream: Value = serde_json::from_str(include_str!("fixtures/trae_intl_stream_observed.json")).unwrap();
        let function = &catalog["response"]["data"]["function_configs"][0];
        assert_eq!(function["function"], "solo_agent");
        let models = parse_models(function);
        assert_eq!(models.len(), 1);
        let model = &models[0];
        assert_eq!(model.id, "deepseek-v4-flash-0731");
        let observed = stream["streams"][0]["events"].as_array().unwrap().iter()
            .find(|event| event["event"] == "model_config").unwrap();
        assert_eq!(observed["data"]["config_name"], model.id);
        assert!(model.upstream_model_ids.iter().any(|id| Some(id.as_str()) == observed["data"]["model_name"].as_str()));
        assert!(identity_matches(model, observed["data"]["model_name"].as_str().unwrap()));
        assert!(!identity_matches(model, "deepseek-v4-flash-0731__undeclared"));
        // The capture omitted token limits; keep context unknown rather than inventing one.
        assert_eq!(model.context_window, None);
    }

    #[test]
    fn international_realms_follow_official_account_and_ai_region_separately() {
        let mut c = empty_credentials();
        c.region = CreditRegion::Intl;
        assert_eq!(authorization_origin(c.region), "https://www.trae.ai");
        for (region, ai, ug) in [
            ("SG", "https://coresg-normal.trae.ai", INTL_API),
            (
                "US-East",
                "https://coreva-normal.trae.ai",
                "https://growva-normal.trae.ai",
            ),
        ] {
            apply_user_region(&mut c, &json!({"AIRegion":region}));
            assert_eq!(api_base(&c), INTL_API);
            assert_eq!(ai_api_base(&c), ai);
            assert_eq!(ug_api_base(&c), ug);
        }
        apply_user_region(&mut c, &json!({"AIRegion":"US", "UserTag":"usttp"}));
        assert_eq!(api_base(&c), "https://grow-normal.traeapi.us");
        assert_eq!(ug_api_base(&c), "https://grow-normal.traeapi.us");
        assert_eq!(ai_api_base(&c), "https://core-normal.traeapi.us");
    }
    #[test]
    fn account_origins_cannot_cross_editions_or_contain_arbitrary_components() {
        assert_eq!(
            allowed_account_origin(CreditRegion::Intl, INTL_API),
            Some(INTL_API)
        );
        assert!(allowed_account_origin(CreditRegion::Cn, INTL_API).is_none());
        assert!(allowed_account_origin(CreditRegion::Intl, API).is_none());
        for host in [
            "http://growsg-normal.trae.ai",
            "https://growsg-normal.trae.ai.evil.test",
            "https://evil.trae.ai",
            "https://growsg-normal.trae.ai:8443",
            "https://user@growsg-normal.trae.ai",
            "https://growsg-normal.trae.ai/path",
            "https://growsg-normal.trae.ai/?x=1",
        ] {
            assert!(allowed_account_origin(CreditRegion::Intl, host).is_none());
        }
    }
    #[test]
    fn international_native_dollars_preserve_fractions_and_unknown_usage() {
        let fixture: Value = serde_json::from_str(include_str!("fixtures/trae_intl_billing.json")).unwrap();
        let value = fixture["dollars"].clone();
        let (packs, note) = parse_international_packs(&value);
        assert_eq!(packs.len(), 4);
        assert!(packs.iter().all(|p| p.unit == "usd"));
        assert_eq!(packs[0].remaining, Some(17.875));
        assert_eq!(packs[1].remaining, Some(3.75));
        assert_eq!(packs[2].remaining, None);
        assert_eq!(packs[3].remaining, None);
        assert!(note.unwrap().contains("不限"));
        assert!(parse_credit_balance(&value).is_none());
    }
    #[test]
    fn missing_international_billing_metadata_keeps_balance_and_unit_unknown() {
        let mut c = empty_credentials(); c.region = CreditRegion::Intl;
        let s = usage_snapshot(&json!({}), &c);
        assert_eq!(s.balance, None);
        assert_eq!(s.unit, "unknown");
        assert!(s.packs.is_empty());
    }

    #[test]
    fn legacy_international_request_quota_is_never_reported_as_credits_or_usd() {
        let fixture: Value = serde_json::from_str(include_str!("fixtures/trae_intl_billing.json")).unwrap();
        let (packs, _) = parse_international_packs(&fixture["requests"]);
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].unit, "requests");
        assert_eq!(packs[0].remaining, Some(477.0));
    }
    #[tokio::test]
    async fn international_checkin_never_submits_cn_entitlement_requests() {
        let mut c = empty_credentials();
        c.region = CreditRegion::Intl;
        c.extra = json!({"ugApiBase":"http://127.0.0.1:1"});
        assert!(rewards(&ProviderClient::new(), &c)
            .await
            .unwrap()
            .is_empty());
    }
    #[test]
    fn reasoning_levels_keep_native_names_and_do_not_infer_tiers_from_a_badge() {
        let models = parse_models(&json!({"config_info_list":[
            {"config_name":"glm-5.3-flash","display_config":{"display_name":"GLM-5.3-Flash"},"reasoning_effort_config":{"support_thinking":true,"options":["light","high","extra_high"],"default_level":"extra_high"}},
            {"config_name":"unknown","display_config":{"display_name":"Unknown"},"display_contact_config":"{\"reasoning\":{\"enable\":true}}"}
        ]}));
        assert_eq!(
            models[0].reasoning_effort_options,
            vec!["light", "high", "extra_high"]
        );
        assert_eq!(
            models[0].default_reasoning_effort.as_deref(),
            Some("extra_high")
        );
        assert!(models[1].reasoning_effort_options.is_empty());
        assert_eq!(models[1].default_reasoning_effort, None);
    }

    #[test]
    fn checkin_gateway_is_independent_of_the_account_realm() {
        let mut c = empty_credentials();
        assert_eq!(ug_api_base(&c), API);
        for realm in ["https://api.trae.cn", "https://api.trae.com.cn"] {
            c.extra = json!({"apiBase":realm});
            assert_eq!(api_base(&c), realm);
            assert_eq!(ug_api_base(&c), "https://api.trae.cn");
        }
        c.extra["ugApiBase"] = json!("http://127.0.0.1:12345");
        assert_eq!(ug_api_base(&c), "http://127.0.0.1:12345");
    }

    #[test]
    fn catalog_model_ids_preserve_declared_variants_and_exclude_deployment_endpoints() {
        let models = parse_models(&json!({"config_info_list":[
            {"config_name":"deepseek-flash","display_config":{"display_name":"DeepSeek Flash"},"model_detail_list":[
                {"model_name":"deepseek-v4-1-flash-260910"},
                {"model_name":"deepseek-v4-1-flash-260910"},
                {"model_name":"deepseek-v4-1-flash-261017"},
                {"model_name":"deepseek-flash__dev"},
                {"model_name":"ep-deployment"},
                {"model_name":""}
            ]},
            {"config_name":"hidden","display_config":{"display_name":"Hidden"},"model_detail_list":[{"model_name":"hidden__dev","encrypted_model_params":{"EncryptedStr":"opaque"}}]}
        ]}));
        assert_eq!(
            models[0].upstream_model_ids,
            vec!["deepseek-v4-1-flash-260910", "deepseek-v4-1-flash-261017", "deepseek-flash__dev"]
        );
        assert_eq!(models[1].upstream_model_ids, vec!["hidden__dev"]);
        let b = chat_body(
            json!({"messages":[{"role":"user","content":"hello"}]}),
            &models[0].id,
        )
        .unwrap();
        assert_eq!(b["config_name"], "deepseek-flash");
    }

    #[test]
    fn context_uses_advertised_maximum_when_present() {
        let models = parse_models(&json!({"config_info_list":[
            {"config_name":"large-window","display_config":{"display_name":"Large"},"context_window_tokens":{"dev":200000,"max":1000000}},
            {"config_name":"fixed-window","display_config":{"display_name":"Fixed"},"context_window_tokens":{"dev":200000}},
            {"config_name":"invalid-window","display_config":{"display_name":"Invalid"},"context_window_tokens":{"dev":256000,"max":0}}
        ]}));
        assert_eq!(
            models.iter().map(|m| m.context_window).collect::<Vec<_>>(),
            vec![Some(1000000), Some(200000), Some(256000)]
        );
    }

    #[test]
    fn model_info_uses_advertised_capabilities_without_guessing_output_limits() {
        let models = parse_models(&json!({"config_info_list":[
            {"config_name":"vision-model","display_config":{"display_name":"Vision model","multimodal":true},"context_window_tokens":{"dev":200000},"display_contact_config":"{\"reasoning\":{\"enable\":true},\"multimodal\":{\"enable\":true}}","reasoning_effort_config":{"support_thinking":false}},
            {"config_name":"hidden-badges","display_config":{"display_name":"Hidden badges","multimodal":false},"display_contact_config":"{\"reasoning\":{\"enable\":false}}"},
            {"config_name":"unknown","display_config":{"display_name":"Unknown"},"display_contact_config":"invalid-json"}
        ]}));
        assert_eq!(models.len(), 3);
        assert_eq!(models[0].context_window, Some(200000));
        assert_eq!(models[0].supports_images, Some(true));
        assert_eq!(models[0].supports_reasoning, Some(true));
        for model in &models {
            assert_eq!(model.max_output_tokens, None);
        }
        for model in &models[1..] {
            assert_eq!(model.supports_images, None);
            assert_eq!(model.supports_reasoning, None);
        }
    }

    #[tokio::test]
    async fn desktop_identity_billing_and_checkin_use_their_own_gateways() {
        use axum::http::HeaderMap;
        use axum::{routing::post, Json, Router};
        use std::sync::atomic::{AtomicUsize, Ordering};
        let claims = Arc::new(AtomicUsize::new(0));
        let status_claims = claims.clone();
        let submitted_claims = claims.clone();
        let ug_app = Router::new()
            .route("/trae/api/v2/ug/checkin_credits/status", post(move |headers: HeaderMap, Json(body): Json<Value>| {
                let claims = status_claims.clone();
                async move {
                    assert_eq!(headers["authorization"], "Cloud-IDE-JWT session-test");
                    assert_eq!(headers["x-device-id"], "existing-device-with-keys");
                    assert_eq!(body["req_source"], 2);
                    Json(json!({"enable":true,"checked_in":claims.load(Ordering::SeqCst) > 0,"credits":100,"extra_credits":100}))
                }
            }))
            .route("/trae/api/v2/ug/checkin_credits/claim", post(move |headers: HeaderMap, Json(body): Json<Value>| {
                let claims = submitted_claims.clone();
                async move {
                    assert_eq!(headers["authorization"], "Cloud-IDE-JWT session-test");
                    assert_eq!(headers["x-device-id"], "existing-device-with-keys");
                    assert_eq!(body["req_source"], 2);
                    claims.fetch_add(1, Ordering::SeqCst);
                    Json(json!({"code":0}))
                }
            }));
        let ug_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let ug_address = ug_listener.local_addr().unwrap();
        let ug_server = tokio::spawn(async move {
            axum::serve(ug_listener, ug_app).await.unwrap();
        });
        let app = Router::new()
            .route("/cloudide/api/v3/trae/GetUserInfo", post(|headers: HeaderMap, Json(body): Json<Value>| async move {
                assert_eq!(headers["x-cloudide-token"], "session-test");
                assert!(!headers.contains_key("authorization"));
                assert_eq!(body["IDEVersion"], "3.4.2");
                Json(json!({"Result":{"UserID":"test-user","Nickname":"Test"}}))
            }))
            .route("/cloudide/api/v3/trae/CheckLogin", post(|headers: HeaderMap, Json(body): Json<Value>| async move {
                assert_eq!(headers["x-cloudide-token"], "session-test");
                assert_eq!(body["GetAIPayHost"], true);
                Json(json!({"Result":{"IsLogin":true,"BoundDeviceID":"test-device"}}))
            }))
            .route("/trae/api/v2/pay/ide_user_ent_usage", post(|headers: HeaderMap, Json(body): Json<Value>| async move {
                assert_eq!(headers["authorization"], "Cloud-IDE-JWT session-test");
                assert_eq!(body["require_usage"], true);
                assert_eq!(body["req_source"], 1);
                Json(json!({"is_credits_billing":true,"is_dollar_usage_billing":false,"usage_summary":{"total_amount":1500,"consumed_amount":200},"user_entitlement_pack_list":[{"entitlement_base_info":{"quota":{"credits_limit":1500}},"usage":{}}]}))
            }))
            .route("/api/ide/v1/get_detail_param", post(|| async {
                Json(json!({"config_info_list":[{"config_name":"test-model","display_config":{"display_name":"Test"}}]}))
            }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let mut c = empty_credentials();
        c.access_token = "session-test".into();
        c.device_id = Some("existing-device-with-keys".into());
        c.extra = json!({"apiBase":format!("http://{address}"),"ugApiBase":format!("http://{ug_address}"),"aiApiBase":format!("http://{address}"),"userJwt":"separate-sso-token","credentialSource":"desktop","ideVersion":"3.4.2"});
        let client = ProviderClient::new();
        let (uid, _, authenticated) = authenticate(&client, &c).await.unwrap();
        assert_eq!(uid, "test-user");
        assert_eq!(authenticated.access_token, "session-test");
        assert_eq!(authenticated.extra["userJwt"], "separate-sso-token");
        assert_eq!(
            authenticated.device_id.as_deref(),
            Some("existing-device-with-keys")
        );
        let snapshot = inspect(&client, &authenticated).await.unwrap();
        assert_eq!(snapshot.balance, Some(1300.0));
        assert_eq!(snapshot.packs[0].remaining, None);
        assert_eq!(snapshot.models.len(), 1);
        assert!(snapshot.diagnostic.is_none());
        assert_eq!(snapshot.rewards[0].status, "claimable");
        assert_eq!(snapshot.rewards[0].amount, Some(100.0));
        let account = Account {
            id: "test-trae".into(),
            platform: Platform::Traework,
            region: CreditRegion::Cn,
            user_id: uid,
            label: "Test".into(),
            enabled: true,
            status: "ready".into(),
            last_error: None,
            snapshot: None,
        };
        let outcomes = claim(&client, &account, &authenticated).await.unwrap();
        assert_eq!(outcomes[0].status, "claimed");
        assert_eq!(outcomes[0].amount, Some(100.0));
        assert_eq!(claims.load(Ordering::SeqCst), 1);
        let outcomes = claim(&client, &account, &authenticated).await.unwrap();
        assert_eq!(outcomes[0].status, "already_claimed");
        assert_eq!(outcomes[0].amount, None);
        assert_eq!(claims.load(Ordering::SeqCst), 1);
        let device_only = reward(
            &json!({"checked_in":false,"did_checked_in":true,"credits":100}),
            &authenticated,
        );
        let mut rejected = ProviderError::new(403, "Device already checked in");
        rejected.rejected = true;
        let outcome = reconcile_claim(
            &account,
            &snapshot.rewards[0],
            &Err(rejected),
            Ok(vec![device_only]),
        );
        assert_eq!(outcome.status, "failed");
        assert_eq!(outcome.amount, None);
        ug_server.abort();
        server.abort();
    }

    #[test]
    fn refreshed_identity_is_distinct_from_the_session() {
        let mut previous = empty_credentials();
        previous.access_token = "old-session".into();
        previous.extra = json!({"userJwt":"old-user-jwt","apiBase":"https://api.trae.cn"});
        let updated = refreshed_credentials(&json!({"Token":"new-session","UserJwt":"new-user-jwt","RefreshToken":"new-refresh","TokenExpireAt":2000000000}), &previous).unwrap();
        assert_eq!(updated.access_token, "new-session");
        assert_eq!(updated.extra["userJwt"], "new-user-jwt");
        assert_eq!(updated.expires_at, Some(2000000000000));
        assert_eq!(updated.extra["apiBase"], "https://api.trae.cn");
    }

    #[tokio::test]
    async fn desktop_refresh_uses_its_actual_client_and_session_header() {
        use axum::http::HeaderMap;
        use axum::{routing::post, Json, Router};
        use p256::pkcs8::{EncodePrivateKey, LineEnding};
        for (region, version) in [(CreditRegion::Cn, "3.4.2"), (CreditRegion::Intl, INTL_APP_VERSION)] {
        let app = Router::new().route("/trae/api/v3/oauth/ExchangeToken", post(move |headers: HeaderMap, Json(body): Json<Value>| async move {
            assert_eq!(headers["x-cloudide-token"], "expired-session-test");
            assert_eq!(body["ClientID"], "desktop-beta-client");
            assert_eq!(body["DeviceInfo"]["PlatformCode"], "IDE_PC");
            assert_eq!(body["DeviceInfo"]["DeviceID"], "existing-device");
            assert_eq!(body["IDEVersion"], version);
            assert_eq!(body["DeviceInfo"]["ClientVersion"], version);
            assert!(body["DeviceProof"]["Signature"].as_str().is_some_and(|signature| !signature.is_empty()));
            Json(json!({"Result":{"Token":"refreshed-session","RefreshToken":"refreshed-refresh"}}))
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let key = p256::SecretKey::from_slice(&[1u8; 32]).unwrap();
        let mut c = empty_credentials();
        c.region = region;
        c.access_token = "expired-session-test".into();
        c.refresh_token = Some("refresh-test".into());
        c.device_id = Some("existing-device".into());
        c.extra = json!({"apiBase":format!("http://{address}"),"clientId":"desktop-beta-client",
            "credentialSource":"desktop","platformCode":"IDE_PC","ideVersion":version,
            "machineId":"existing-machine","devicePublicKey":"test-public-key",
            "devicePrivateKey":key.to_pkcs8_pem(LineEnding::LF).unwrap().as_str()});
        let updated = refresh(&ProviderClient::new(), &c).await.unwrap();
        assert_eq!(updated.access_token, "refreshed-session");
        assert_eq!(updated.region, region);
        assert_eq!(updated.extra["apiBase"], c.extra["apiBase"]);
        assert_eq!(updated.extra["machineId"], c.extra["machineId"]);
        assert_eq!(updated.extra["clientId"], "desktop-beta-client");
        server.abort();
        }
    }

    #[tokio::test]
    #[ignore = "requires the user's local TRAE CN desktop login; read-only identity and billing"]
    async fn desktop_import_read_only_smoke() {
        assert_eq!(
            std::env::var("AI_TOOLKIT_TRAE_DIAGNOSTIC").as_deref(),
            Ok("read-only")
        );
        let c = local_credentials()
            .unwrap()
            .expect("desktop credentials available");
        assert_eq!(
            text(&c.extra, &["credentialSource"]).as_deref(),
            Some("desktop")
        );
        let client = ProviderClient::new();
        let (uid, _, authenticated) = authenticate(&client, &c).await.unwrap();
        assert!(c.user_id.as_deref().is_none_or(|stored| stored == uid));
        let v = client
            .json(
                headers(
                    client.http.post(format!(
                        "{}/trae/api/v2/pay/ide_user_ent_usage",
                        api_base(&authenticated)
                    )),
                    &authenticated,
                    false,
                )
                .json(&json!({"require_usage":true,"req_source":1})),
            )
            .await
            .unwrap();
        let (packs, quota) = parse_packs(&v);
        let result = snapshot(packs, vec![], vec![], None);
        let balance = parse_credit_balance(&v).or(result.balance);
        if balance.is_none() {
            fn shape(value: &Value) -> Value {
                match value {
                    Value::Object(map) => Value::Object(
                        map.iter()
                            .map(|(key, value)| (key.clone(), shape(value)))
                            .collect(),
                    ),
                    Value::Array(values) => {
                        Value::Array(values.iter().take(2).map(shape).collect())
                    }
                    Value::String(_) => json!("string"),
                    Value::Number(_) => json!("number"),
                    Value::Bool(_) => json!("bool"),
                    Value::Null => Value::Null,
                }
            }
            println!("TRAE billing response field types: {}", shape(&v));
        }
        assert!(balance.is_some());
        println!(
            "TRAE desktop identity verified; balance={:?}; packs={}; quota_present={}",
            balance,
            result.packs.len(),
            quota.is_some()
        );
    }
    #[test]
    fn signin_reward_is_not_balance() {
        let c = empty_credentials();
        for (credits, extra) in [(100, 100), (150, 50), (200, 100), (200, 0)] {
            let r = reward(
                &json!({"checked_in":false,"credits":credits,"extra_credits":extra}),
                &c,
            );
            assert_eq!(r.amount, Some(credits as f64));
        }
        assert_eq!(reward(&json!({"extra_credits":100}), &c).amount, None);
        let (p, _) = parse_packs(&json!({"user_entitlement_pack_list":[]}));
        assert!(p.is_empty());
    }
    #[test]
    fn checkin_requires_account_confirmation_and_respects_device_limits() {
        let mut c = empty_credentials();
        c.device_id = Some("test-device".into());
        for (value, expected) in [
            (
                json!({"enable":true,"checked_in":false,"did_checked_in":false}),
                "claimable",
            ),
            (
                json!({"enable":true,"checked_in":false,"did_checked_in":true}),
                "unavailable",
            ),
            (json!({"enable":true,"did_checked_in":true}), "unavailable"),
            (json!({"checked_in":true,"did_checked_in":true}), "claimed"),
            (json!({"checked_in":true,"did_checked_in":false}), "claimed"),
            (json!({"enable":false,"checked_in":false}), "unavailable"),
            (json!({"enable":true}), "unknown"),
        ] {
            assert_eq!(reward(&value, &c).status, expected, "{value}");
        }
    }
    #[test]
    fn quota_units_are_separate() {
        let (p, q) = parse_packs(
            &json!({"user_entitlement_pack_list":[{"entitlement_base_info":{"quota":{"credits_limit":200,"premium_model_fast_request_limit":10}},"usage":{"credits_amount":23,"premium_model_fast_amount":3}}]}),
        );
        assert_eq!(p[0].remaining, Some(177.0));
        assert!(q.unwrap().contains("7 次"));
    }
    #[test]
    fn missing_usage_is_not_an_unspent_balance() {
        let (ps, _) = parse_packs(
            &json!({"user_entitlement_pack_list":[{"entitlement_base_info":{"quota":{"credits_limit":200}}}]}),
        );
        assert_eq!(ps[0].remaining, None);
        assert_eq!(snapshot(ps, vec![], vec![], None).balance, None);
    }
    #[test]
    fn credit_summary_requires_complete_credit_amounts() {
        let bill = json!({"is_credits_billing":true,"is_dollar_usage_billing":false,
            "usage_summary":{"total_amount":1500,"consumed_amount":200}});
        assert_eq!(parse_credit_balance(&bill), Some(1300.0));
        let mut invalid = bill.clone();
        invalid["is_dollar_usage_billing"] = json!(true);
        assert_eq!(parse_credit_balance(&invalid), None);
        invalid = bill.clone();
        invalid["is_credits_billing"] = json!(false);
        assert_eq!(parse_credit_balance(&invalid), None);
        invalid = bill.clone();
        invalid["usage_summary"]["consumed_amount"] = Value::Null;
        assert_eq!(parse_credit_balance(&invalid), None);
        for amount in [json!(-1), json!("NaN"), json!("inf")] {
            invalid = bill.clone();
            invalid["usage_summary"]["total_amount"] = amount;
            assert_eq!(parse_credit_balance(&invalid), None);
        }
        invalid = bill;
        invalid["usage_summary"]["consumed_amount"] = json!(1600);
        assert_eq!(parse_credit_balance(&invalid), Some(0.0));
    }
    #[test]
    fn promotional_or_canceled_packs_can_have_remaining_credits() {
        let (packs, _) = parse_packs(&json!({"user_entitlement_pack_list":[{
            "status":3,"entitlement_base_info":{"product_type":3,"quota":{"credits_limit":200}},
            "usage":{"credits_amount":23}
        }]}));
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].remaining, Some(177.0));
    }
    #[test]
    fn tool_roundtrip_request_keeps_id_and_serializes_schema() {
        let b=chat_body(json!({"messages":[{"role":"assistant","content":null,"tool_calls":[{"id":"call1","function":{"name":"read","arguments":"{}"}}]},{"role":"tool","tool_call_id":"call1","content":"ok"}],"tools":[{"function":{"name":"read","parameters":{"type":"object"}}}],"reasoning_effort":"high"}),"glm-5.3").unwrap();
        assert_eq!(b["messages"][1]["tool_call_id"], "call1");
        assert!(b["tools"][0]["function"]["parameters"].is_string());
        assert!(b.get("reasoning_effort").is_none());
    }

    #[test]
    fn image_attachments_and_tool_results_survive_native_tool_conversion() {
        let image = json!({"type":"image_url","image_url":{"url":"data:image/png;base64,aW1hZ2U=","detail":"high"}});
        let remote =
            json!({"type":"image_url","image_url":{"url":"https://example.test/screenshot.png"}});
        let messages = json!([
            {"role":"user","content":[image.clone()]},
            {"role":"assistant","content":null,"reasoning_content":"inspect the screenshot","tool_calls":[{"id":"call_image","type":"function","function":{"name":"Read","arguments":"{}"}}]},
            {"role":"tool","tool_call_id":"call_image","content":[{"type":"text","text":"[tool_result is_error=true]\n"},remote.clone(),image.clone()]},
            {"role":"user","content":[{"type":"text","text":"compare both images"},image,remote]},
        ]);
        let body = chat_body(json!({"messages":messages,"max_tokens":128,"tools":[{"type":"function","function":{"name":"Read","parameters":{"type":"object"}}}]}), "glm-5.3-flash").unwrap();
        for index in [0, 2, 3] {
            assert_eq!(body["messages"][index], messages[index]);
        }
        assert_eq!(body["messages"][1]["tool_calls"][0]["id"], "call_image");
        assert_eq!(
            body["messages"][1]["reasoning_content"],
            "inspect the screenshot"
        );
        assert_eq!(
            body["messages"][1]["tool_calls"][0]["function_call"]["name"],
            "Read"
        );
        assert_eq!(body["messages"][2]["tool_call_id"], "call_image");
        assert_eq!(
            body["messages"][2]["content"][0]["text"],
            "[tool_result is_error=true]\n"
        );
        assert!(body["tools"][0]["function"]["parameters"].is_string());
        assert_eq!(body["function"], "solo_work_lite");
        assert_eq!(body["config_name"], "glm-5.3-flash");
        assert_eq!(body["model"], "glm-5.3-flash");
        assert_eq!(body["max_tokens"], 128);
        assert_eq!(body["stream"], true);
    }
}
