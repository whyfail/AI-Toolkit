// COSY signing/encoding adapted from cpa-multi-plugins (MIT), pinned in NOTICE.md.
use super::*;
use aes::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine,
};
use rsa::{pkcs8::DecodePublicKey, Pkcs1v15Encrypt, RsaPublicKey};
pub(super) const API: &str = "https://openapi.qoder.com.cn";
const GATEWAY: &str = "https://gateway.qoder.com.cn";
pub(super) const CLIENT_ID: &str = "732aef47-9cf2-46a2-95fe-4cebb5d0d1fa";
const PUBKEY:&str="-----BEGIN PUBLIC KEY-----\nMIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDA8iMH5c02LilrsERw9t6Pv5Nc\n4k6Pz1EaDicBMpdpxKduSZu5OANqUq8er4GM95omAGIOPOh+Nx0spthYA2BqGz+l\n6HRkPJ7S236FZz73In/KVuLnwI8JJ2CbuJap8kvheCCZpmAWpb/cPx/3Vr/J6I17\nXcW+ML9FoCI6AOvOzwIDAQAB\n-----END PUBLIC KEY-----";
pub(super) fn headers(r: RequestBuilder, c: &Credentials) -> RequestBuilder {
    r.bearer_auth(&c.access_token)
        .header("User-Agent", "Qoder/0.4.3")
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
}
fn campaign_headers(r: RequestBuilder, c: &Credentials) -> RequestBuilder {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        os => os,
    };
    let mut r = r
        .bearer_auth(&c.access_token)
        .header("Accept", "application/json")
        .header("User-Agent", "Qoder")
        .header("Cosy-ClientType", "10")
        .header("Cosy-Version", "0.4.4")
        .header("Cosy-MachineOS", format!("{}_{os}", std::env::consts::ARCH));
    if let Some(machine) = &c.device_id {
        r = r.header("Cosy-MachineId", machine);
    }
    r
}

async fn campaign_json(request: RequestBuilder) -> Result<Value, ProviderError> {
    let response = request
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(ProviderError::transport)?;
    if response.status().is_success() {
        return json_response(response).await;
    }
    let status = response.status().as_u16();
    let value: Value = response.json().await.map_err(|_| http_error(status))?;
    let Some(code) = text(&value, &["errorCode"]) else {
        return Err(http_error(status));
    };
    let message = match code.as_str() {
        "RISK_BLOCKED" => "Qoder 未通过设备校验，积分未发放",
        "RISK_DEPENDENCY_UNAVAILABLE" => "Qoder 设备校验服务暂不可用，请稍后重试",
        "CAMPAIGN_NOT_ACTIVE" => "Qoder 活动已结束或尚未开始",
        "CAMPAIGN_NOT_FOUND" => "Qoder 活动不存在，请刷新领取资格",
        "GRANT_RESULT_UNKNOWN" => "Qoder 积分发放结果待确认",
        _ => "Qoder 活动请求被拒绝",
    };
    let mut error = http_error(status);
    error.message = format!("{message}（{code}）");
    // A device rejection is not an expired OAuth token.
    if matches!(
        code.as_str(),
        "RISK_BLOCKED" | "RISK_DEPENDENCY_UNAVAILABLE"
    ) {
        error.rejected = false;
    }
    Err(error)
}
pub(super) async fn authenticate(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<(String, String, Credentials), ProviderError> {
    let mut c = c.clone();
    if c.access_token.starts_with("pt-") {
        let v = client
            .json(
                client
                    .http
                    .post(format!("{API}/api/v1/jobToken/exchange"))
                    .json(&json!({"personal_token":c.access_token})),
            )
            .await?;
        c = credentials(data(&v), &c, &["token"], &["refresh_token"])?;
    }
    let v = client
        .json(headers(
            client.http.get(format!("{API}/api/v1/userinfo")),
            &c,
        ))
        .await?;
    let d = data(&v);
    let uid = text(d, &["id", "user_id", "uid"])
        .ok_or_else(|| ProviderError::new(502, "Qoder 未返回账号身份"))?;
    let label = text(d, &["name", "username"]).unwrap_or_else(|| format!("Qoder {uid}"));
    c.user_id = Some(uid.clone());
    if c.device_id.is_none() {
        c.device_id = Some(uuid::Uuid::new_v4().to_string());
    }
    c.extra = d.clone();
    Ok((uid, label, c))
}
pub(super) async fn refresh(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<Credentials, ProviderError> {
    let rt = c
        .refresh_token
        .as_ref()
        .ok_or_else(|| ProviderError::new(401, "Qoder 凭证没有刷新令牌，请重新登录"))?;
    let path = if rt.starts_with("jrt-") {
        "jobToken/refresh"
    } else {
        "deviceToken/refresh"
    };
    let v = client
        .json(
            client
                .http
                .post(format!("{API}/api/v1/{path}"))
                .json(&json!({"refresh_token":rt})),
        )
        .await?;
    let mut new = credentials(data(&v), c, &["token", "device_token"], &["refresh_token"])?;
    if let Some(ms) = num(data(&v), &["expires_in"]) {
        new.expires_at = Some(now_ms() + ms as i64)
    }
    Ok(new)
}
fn encode(plain: &[u8]) -> String {
    let s = STANDARD.encode(plain);
    let n = s.len();
    let a = n / 3;
    let rearranged = format!("{}{}{}", &s[n - a..], &s[a..n - a], &s[..a]);
    const STD: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    const CUSTOM: &[u8] = b"_doRTgHZBKcGVjlvpC,@aFSx#DPuNJme&i*MzLOEn)sUrthbf%Y^w.(kIQyXqWA!";
    rearranged
        .bytes()
        .map(|c| {
            if c == b'=' {
                '$'
            } else {
                CUSTOM[STD.iter().position(|x| *x == c).expect("base64 character")] as char
            }
        })
        .collect()
}
fn signature(payload: &str, key: &str, date: &str, body: &str, path: &str) -> String {
    format!(
        "{:x}",
        md5::compute(format!(
            "{payload}\n{key}\n{date}\n{body}\n{}",
            path.strip_prefix("/algo").unwrap_or(path)
        ))
    )
}
fn signed(
    r: RequestBuilder,
    c: &Credentials,
    body: String,
    url: &str,
    model_id: Option<&str>,
) -> Result<RequestBuilder, ProviderError> {
    let key = uuid::Uuid::new_v4().simple().to_string()[..16].to_string();
    let pubkey = RsaPublicKey::from_public_key_pem(PUBKEY)
        .map_err(|_| ProviderError::new(500, "Qoder 签名公钥配置错误"))?;
    let wrapped = STANDARD.encode(
        pubkey
            .encrypt(&mut rand::rngs::OsRng, Pkcs1v15Encrypt, key.as_bytes())
            .map_err(|_| ProviderError::new(500, "Qoder 签名失败"))?,
    );
    let identity = json!({"name":text(&c.extra,&["name"]).unwrap_or_default(),"aid":c.user_id.as_deref().unwrap_or(""),"uid":c.user_id.as_deref().unwrap_or(""),"yx_uid":"","organization_id":text(&c.extra,&["organization_id"]).unwrap_or_default(),"organization_name":text(&c.extra,&["organization_name"]).unwrap_or_default(),"user_type":text(&c.extra,&["user_type"]).unwrap_or("personal_standard".into()),"security_oauth_token":c.access_token,"refresh_token":c.refresh_token.as_deref().unwrap_or("")});
    let info = STANDARD.encode(
        cbc::Encryptor::<aes::Aes128>::new_from_slices(key.as_bytes(), key.as_bytes())
            .map_err(|_| ProviderError::new(500, "Qoder 加密失败"))?
            .encrypt_padded_vec_mut::<Pkcs7>(identity.to_string().as_bytes()),
    );
    let payload=STANDARD.encode(json!({"cosyVersion":"0.1.43","ideVersion":"","info":info,"requestId":uuid::Uuid::new_v4().to_string(),"version":"v1"}).to_string());
    let date = (now_ms() / 1000).to_string();
    let path =
        reqwest::Url::parse(url).map_err(|_| ProviderError::new(500, "Qoder 请求地址配置错误"))?;
    let sig = signature(&payload, &wrapped, &date, &body, path.path());
    let machine = c
        .device_id
        .as_deref()
        .ok_or_else(|| ProviderError::new(400, "Qoder 凭证缺少已保存的设备身份，请重新添加账号"))?;
    let mut r = r
        .header("Authorization", format!("Bearer COSY.{payload}.{sig}"))
        .header("cosy-data-policy", "AGREE")
        .header("cosy-machinetype", "desktop")
        .header("cosy-clienttype", "5")
        .header("cosy-date", date)
        .header("cosy-user", c.user_id.as_deref().unwrap_or(""))
        .header("cosy-key", wrapped)
        .header("cosy-clientip", "169.254.198.161")
        .header("cosy-version", "0.1.43")
        .header("cosy-machineid", machine)
        .header("cosy-machinetoken", URL_SAFE_NO_PAD.encode(machine))
        .header("login-version", "v2")
        .header("User-Agent", "Qoder/0.4.3")
        .header("Content-Type", "application/json")
        .header("Accept-Encoding", "identity")
        .body(body);
    if let Some(id) = model_id {
        r = r
            .header("x-model-key", id)
            .header("x-model-source", "system");
    }
    Ok(r)
}
fn parse_models(v: &Value) -> Vec<ModelInfo> {
    data(v)["chat"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            if flag(m, &["enable"]) != Some(true) {
                return None;
            }
            let key = text(m, &["key"])?;
            let name = text(m, &["display_name"]).unwrap_or(key.clone());
            // Public id follows the display name (workbuddy-style); the
            // platform key travels as upstream_id for the actual call.
            let id = display_slug(&name).unwrap_or_else(|| key.clone());
            Some(ModelInfo {
                upstream_id: (id != key).then_some(key),
                ..model(
                    id,
                    name,
                    num(m, &["max_input_tokens"]).map(|n| n as u64),
                )
            })
        })
        .collect()
}
/// Display-name slug for public model ids: `GLM-5.3-Flash` -> `glm-5.3-flash`.
fn display_slug(name: &str) -> Option<String> {
    let slug = name
        .trim()
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-");
    (!slug.is_empty()).then_some(slug)
}
async fn models(client: &ProviderClient, c: &Credentials) -> Result<Vec<ModelInfo>, ProviderError> {
    let url = format!("{GATEWAY}/algo/api/v2/model/list?Encode=1");
    let r = signed(client.http.get(&url), c, encode(b"{}"), &url, None)?;
    let v = client.json(r.header("Accept", "application/json")).await?;
    let ms = parse_models(&v);
    if ms.is_empty() {
        return Err(ProviderError::new(
            502,
            "Qoder 当前账号没有可选择的聊天模型",
        ));
    }
    Ok(ms)
}
fn parse_packs(v: &Value) -> Vec<CreditPack> {
    let d = data(v);
    let mut out = vec![];
    for (name, key) in [
        ("套餐积分", "userQuota"),
        ("附加积分", "addOnQuota"),
        ("组织积分", "orgResourcePackage"),
    ] {
        if let Some(p) = d.get(key).filter(|p| p.is_object()) {
            out.push(CreditPack {
                id: key.into(),
                name: name.into(),
                remaining: num(p, &["remaining"]),
                expires_at: d.get("expiresAt").and_then(epoch),
            });
        }
    }
    if let Some(ps) = d["dedicatedResourcePackages"].as_array() {
        for (i, p) in ps.iter().enumerate() {
            out.push(CreditPack {
                id: format!("dedicated-{i}"),
                name: text(p, &["name"]).unwrap_or("赠送积分包".into()),
                remaining: num(p, &["remaining"]),
                expires_at: p.get("expiresAt").and_then(epoch),
            });
        }
    }
    out
}
fn campaign_name(c: &Value) -> String {
    text(c, &["title", "name", "displayName"])
        .or_else(|| {
            c["placements"].as_array()?.iter().find_map(|placement| {
                text(&placement["content"]["zh"], &["title"])
                    .or_else(|| text(&placement["content"]["en"], &["title"]))
            })
        })
        .unwrap_or("平台活动权益".into())
}
fn parse_web_campaign(c: &Value) -> Option<Reward> {
    let url = text(c, &["campaignUrl", "campaign_url", "activityUrl"])?;
    let claimable = flag(c, &["claimable"]);
    if flag(c, &["showCampaign"]) == Some(false) && claimable != Some(true) {
        return None;
    }
    Some(Reward {
        id: text(c, &["campaignId", "id"]).unwrap_or_else(|| url.clone()),
        name: campaign_name(c),
        kind: "activity".into(),
        status: match claimable {
            Some(true) => "claimable",
            Some(false) => "unavailable",
            None => "unknown",
        }
        .into(),
        amount: None,
        next_available_at: c.get("startsAt").and_then(epoch),
        action_url: Some(url),
    })
}

fn parse_rewards(v: &Value) -> Vec<Reward> {
    let d = data(v);
    if flag(d, &["showCampaign"]) == Some(false) {
        return vec![];
    }
    let mut out: Vec<Reward> = d["campaigns"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let action = text(c, &["actionType"]).unwrap_or_default();
            // Only CLAIM_BENEFIT + CREDITS can participate in one-click claims.
            if action != "CLAIM_BENEFIT"
                && text(c, &["campaignUrl", "campaign_url", "activityUrl"]).is_some()
            {
                return parse_web_campaign(c);
            }
            let kind = text(&c["benefit"], &["kind"])
                .unwrap_or_default()
                .to_ascii_uppercase();
            let id = text(c, &["campaignId", "id"])?;
            let status = text(c, &["claimStatus", "status"]).unwrap_or_default();
            Some(Reward {
                id,
                name: campaign_name(c),
                kind: if kind == "CREDITS" {
                    "credits"
                } else {
                    "coupon"
                }
                .into(),
                status: if kind != "CREDITS" || action != "CLAIM_BENEFIT" {
                    "unavailable"
                } else if status == "CLAIMED" {
                    "claimed"
                } else if status == "CLAIMABLE" {
                    "claimable"
                } else {
                    "unavailable"
                }
                .into(),
                amount: if kind == "CREDITS" {
                    num(&c["benefit"], &["amount"])
                } else {
                    None
                },
                next_available_at: c
                    .get("startAt")
                    .or_else(|| c.get("startsAt"))
                    .and_then(epoch),
                action_url: None,
            })
        })
        .collect();
    // The top-level URL is the container for the detailed list, not an extra reward.
    if !d["campaigns"].is_array() {
        if let Some(campaign) = parse_web_campaign(d) {
            if !out
                .iter()
                .any(|reward| reward.action_url == campaign.action_url)
            {
                out.push(campaign);
            }
        }
    }
    out
}
async fn rewards(client: &ProviderClient, c: &Credentials) -> Result<Vec<Reward>, ProviderError> {
    rewards_at(client, c, API).await
}
async fn rewards_at(
    client: &ProviderClient,
    c: &Credentials,
    api: &str,
) -> Result<Vec<Reward>, ProviderError> {
    let v = campaign_json(campaign_headers(
        client.http.get(format!("{api}/sash/api/v1/me/campaigns")),
        c,
    ))
    .await?;
    if !data(&v)["campaigns"].is_array() && !data(&v)["showCampaign"].is_boolean() {
        return Err(ProviderError::new(502, "Qoder 未返回有效的活动领取资格"));
    }
    Ok(parse_rewards(&v))
}
pub(super) async fn inspect(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<ProviderSnapshot, ProviderError> {
    let (p, r, m) = tokio::join!(
        client.json(headers(
            client.http.get(format!("{API}/api/v2/quota/usage")),
            c
        )),
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
    Ok(snapshot(
        parse_packs(&p),
        r,
        m,
        (!diagnostics.is_empty()).then(|| diagnostics.join("；")),
    ))
}
pub(super) async fn claim(
    client: &ProviderClient,
    a: &Account,
    c: &Credentials,
) -> Result<Vec<RewardOutcome>, ProviderError> {
    claim_at(client, a, c, API, super::qoder_identity::helper_path()).await
}
async fn claim_at(
    client: &ProviderClient,
    a: &Account,
    c: &Credentials,
    api: &str,
    helper: Option<std::path::PathBuf>,
) -> Result<Vec<RewardOutcome>, ProviderError> {
    let rs = rewards_at(client, c, api).await?;
    let mut out = vec![];
    let proof = if rs
        .iter()
        .any(|r| r.status == "claimable" && r.kind == "credits")
    {
        match helper {
            Some(path) => super::qoder_identity::proof(path, &a.user_id).await?,
            None => reqwest::header::HeaderMap::new(),
        }
    } else {
        reqwest::header::HeaderMap::new()
    };
    for r in rs {
        if r.status == "claimed" && r.kind == "credits" {
            out.push(outcome(
                a,
                &r,
                "already_claimed",
                "本轮积分已领取，无需重复领取",
                None,
            ));
            continue;
        }
        if r.status != "claimable" || r.kind != "credits" {
            continue;
        }
        // IDs come only from this account's live eligible list. No guessed campaign IDs.
        let url = format!(
            "{api}/sash/api/v1/me/campaigns/{}/claim",
            urlencoding::encode(&r.id)
        );
        let submitted =
            campaign_json(campaign_headers(client.http.post(url), c).headers(proof.clone())).await;
        match &submitted {
            Ok(v) if credits_confirmed(data(v)) => {
                let d = data(v);
                out.push(outcome(
                    a,
                    &r,
                    "claimed",
                    "Qoder 确认积分已领取，余额可能延迟更新",
                    num(&d["benefit"], &["amount"]).or(r.amount),
                ));
            }
            _ => {
                // A timeout/pending grant must be read back, never blindly resubmitted.
                let reward_url = format!(
                    "{api}/sash/api/v1/me/campaigns/{}/reward",
                    urlencoding::encode(&r.id)
                );
                let confirmed = campaign_json(
                    campaign_headers(client.http.get(reward_url), c).headers(proof.clone()),
                )
                .await
                .ok()
                .filter(|v| credits_confirmed(data(v)));
                if let Some(v) = confirmed {
                    out.push(outcome(
                        a,
                        &r,
                        "claimed",
                        "回查 Qoder 确认积分已领取",
                        num(&data(&v)["benefit"], &["amount"]).or(r.amount),
                    ));
                } else {
                    let mut result =
                        reconcile_claim(a, &r, &submitted, rewards_at(client, c, api).await);
                    if result.status != "claimed" {
                        if let Err(error) = &submitted {
                            result.message = format!("{}；{}", error.message, result.message);
                            if matches!(error.status, 400 | 403 | 404 | 410) {
                                result.status = "failed".into();
                            }
                        }
                    }
                    out.push(result);
                }
            }
        }
    }
    if out.is_empty() {
        out.push(RewardOutcome {
            account_id: a.id.clone(),
            reward_id: "campaigns".into(),
            name: "活动积分".into(),
            status: "unavailable".into(),
            amount: None,
            kind: "credits".into(),
            message: "当前账号暂无已明确可领取的积分活动".into(),
            created_at: now_ms(),
        });
    }
    Ok(out)
}
fn credits_confirmed(v: &Value) -> bool {
    text(v, &["status"]).as_deref() == Some("CLAIMED")
        // The submitted ID already comes from a CREDITS campaign. Official
        // responses may only return the grant status, without repeating benefit.
        && text(&v["benefit"], &["kind"]).is_none_or(|kind| kind == "CREDITS")
}
fn body(mut b: Value, id: &str) -> Result<Value, ProviderError> {
    let messages = b["messages"]
        .as_array()
        .ok_or_else(|| ProviderError::new(400, "messages 必须是数组"))?;
    let has_images = messages.iter().any(|m| {
        m["content"]
            .as_array()
            .is_some_and(|ps| ps.iter().any(|p| p["type"] == "image_url"))
    });
    let prompt = messages
        .iter()
        .rev()
        .find(|m| m["role"] == "user")
        .map(|m| {
            m["content"].as_str().map(str::to_owned).unwrap_or_else(|| {
                m["content"]
                    .as_array()
                    .map(|ps| {
                        ps.iter()
                            .filter_map(|p| p["text"].as_str())
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default()
            })
        })
        .filter(|s| !s.is_empty() || has_images)
        .ok_or_else(|| ProviderError::new(400, "缺少 user 消息"))?;
    let mut system = vec![];
    if let Some(ms) = b["messages"].as_array() {
        for m in ms {
            if m["role"] == "system" {
                system.push(m["content"].clone())
            }
        }
    }
    let mut id_config = json!({"key":id,"source":"system","is_reasoning":true,"format":"openai"});
    if has_images {
        // Qoder CN's official RequestConverter sends multimodal parts in both
        // content and contents, and model_config.is_vl selects its vision route.
        id_config["is_vl"] = json!(true);
        for message in b["messages"].as_array_mut().unwrap() {
            if let Some(parts) = message
                .get("content")
                .filter(|content| content.is_array())
                .cloned()
            {
                message["contents"] = parts;
            }
        }
    }
    let mut out = json!({"request_id":uuid::Uuid::new_v4().to_string(),"request_set_id":uuid::Uuid::new_v4().to_string(),"chat_record_id":uuid::Uuid::new_v4().to_string(),"session_id":uuid::Uuid::new_v4().to_string(),"stream":true,"chat_task":"FREE_INPUT","chat_context":{"chatPrompt":"","extra":{"context":[],"modelConfig":id_config,"originalContent":{"type":"text","text":prompt}},"features":[],"text":{"type":"text","text":prompt}},"is_reply":true,"is_retry":false,"source":1,"version":"3","session_type":"qodercli","agent_id":"agent_common","task_id":"common","model_config":id_config,"messages":b["messages"],"system":system,"parameters":{"max_tokens":b["max_tokens"].as_u64().unwrap_or(8192)}});
    for key in ["tools", "tool_choice"] {
        if let Some(v) = b.as_object_mut().and_then(|o| o.remove(key)) {
            out[key] = v;
        }
    }
    Ok(out)
}
pub(super) async fn chat(
    client: &ProviderClient,
    c: &Credentials,
    m: &ModelInfo,
    b: Value,
) -> Result<ProviderChat, ProviderError> {
    let upstream = m.upstream_id.as_deref().unwrap_or(&m.id);
    let mut b = body(b, upstream)?;
    b["aliyun_user_type"] =
        json!(text(&c.extra, &["user_type"]).unwrap_or("personal_standard".into()));
    let url=format!("{GATEWAY}/algo/api/v2/service/pro/sse/agent_chat_generation?FetchKeys=llm_model_result&AgentId=agent_common&Encode=1");
    let r = signed(
        client.http.post(&url),
        c,
        encode(b.to_string().as_bytes()),
        &url,
        Some(upstream),
    )?
    .header("Accept", "text/event-stream")
    .header("Cache-Control", "no-cache");
    stream::response(r, Platform::Qoder).await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_requests_keep_multimodal_contents_and_enable_the_vision_route() {
        let parts = json!([{"type":"text","text":"describe this"},{"type":"image_url","image_url":{"url":"data:image/png;base64,aW1hZ2U="}}]);
        let input = json!({"messages":[{"role":"system","content":"instructions"},{"role":"user","content":parts}],"max_tokens":128});
        let converted = body(input.clone(), "gfmodel").unwrap();
        assert_eq!(converted["model_config"]["is_vl"], true);
        assert_eq!(
            converted["chat_context"]["extra"]["modelConfig"]["is_vl"],
            true
        );
        assert_eq!(converted["messages"][1]["content"], parts);
        assert_eq!(converted["messages"][1]["contents"], parts);

        let mut image_only = input;
        image_only["messages"][1]["content"] = json!([parts[1].clone()]);
        let converted = body(image_only, "gfmodel").unwrap();
        assert_eq!(converted["messages"][1]["contents"][0], parts[1]);
        assert_eq!(converted["chat_context"]["text"]["text"], "");

        let converted = body(json!({"messages":[{"role":"user","content":"read image"},{"role":"assistant","content":"","tool_calls":[{"id":"call_1","type":"function","function":{"name":"Read","arguments":"{}"}}]},{"role":"tool","tool_call_id":"call_1","content":parts}]}), "gfmodel").unwrap();
        assert_eq!(converted["messages"][2]["contents"], parts);
        assert_eq!(converted["messages"][2]["tool_call_id"], "call_1");
        assert_eq!(converted["model_config"]["is_vl"], true);
        let text = body(
            json!({"messages":[{"role":"user","content":"hello"}]}),
            "gfmodel",
        )
        .unwrap();
        assert_ne!(text["model_config"]["is_vl"], true);
        assert!(text["messages"][0].get("contents").is_none());
    }

    #[test]
    fn qoder_encoding_matches_reference_vectors() {
        assert_eq!(encode(b"{}"), "$kwm");
        assert_eq!(encode(b""), "");
    }
    #[test]
    fn signing_excludes_query_and_algo_prefix() {
        assert_eq!(
            signature("p", "k", "1", "b", "/algo/path"),
            format!("{:x}", md5::compute("p\nk\n1\nb\n/path"))
        );
    }
    #[test]
    fn coupon_and_unknown_benefit_are_never_claimed() {
        let rs = parse_rewards(
            &json!({"campaigns":[{"id":"a","actionType":"CLAIM_BENEFIT","claimStatus":"CLAIMABLE","benefit":{"kind":"REDEMPTION_CODE"}},{"id":"b","actionType":"CLAIM_BENEFIT","claimStatus":"CLAIMABLE","benefit":{"kind":"CREDITS","amount":100}},{"id":"c","actionType":"CLAIM_BENEFIT","claimStatus":"CLAIMABLE"}]}),
        );
        assert_eq!(rs[0].status, "unavailable");
        assert_eq!(rs[1].status, "claimable");
        assert_eq!(rs[2].status, "unavailable");
    }
    #[test]
    fn campaign_status_reads_wrapped_data_without_guessing_an_id() {
        let url = "https://qoder.com.cn/activities";
        let rs = parse_rewards(&json!({"data": {
            "showCampaign": true,
            "claimable": true,
            "campaignUrl": url
        }}));
        assert_eq!(rs.len(), 1);
        assert_eq!(rs[0].id, url);
        assert_eq!(rs[0].kind, "activity");
        assert_eq!(rs[0].status, "claimable");
        assert_eq!(rs[0].action_url.as_deref(), Some(url));
    }
    #[test]
    fn campaign_status_preserves_the_upstream_id_and_title() {
        let rs = parse_rewards(&json!({
            "campaignId": "daily-credits",
            "title": "每日领取积分",
            "showCampaign": true,
            "claimable": false,
            "campaignUrl": "https://qoder.com.cn/activities"
        }));
        assert_eq!(rs[0].id, "daily-credits");
        assert_eq!(rs[0].name, "每日领取积分");
        assert_eq!(rs[0].status, "unavailable");
    }
    #[test]
    fn missing_claim_eligibility_is_not_treated_as_claimed() {
        let rs = parse_rewards(&json!({"campaigns": [{
            "id": "daily-credits",
            "campaignUrl": "https://qoder.com.cn/activities"
        }]}));
        assert_eq!(rs[0].status, "unknown");
    }
    #[test]
    fn disabled_web_campaigns_are_not_shown_as_claimed_rewards() {
        let rs = parse_rewards(&json!({"campaigns": [{
            "showCampaign": false,
            "claimable": false,
            "campaignUrl": "https://qoder.com.cn/activities"
        }]}));
        assert!(rs.is_empty());
    }
    #[test]
    fn credit_campaign_with_an_information_url_still_uses_its_claim_status() {
        let rs = parse_rewards(&json!({"campaigns": [{
            "id": "credit-benefit",
            "actionType": "CLAIM_BENEFIT",
            "claimStatus": "CLAIMABLE",
            "benefit": {"kind": "CREDITS", "amount": 100},
            "campaignUrl": "https://qoder.com.cn/activities"
        }]}));
        assert_eq!(rs[0].kind, "credits");
        assert_eq!(rs[0].status, "claimable");
        assert_eq!(rs[0].amount, Some(100.0));
    }
    fn daily_campaign(status: &str) -> Value {
        json!({
            "showCampaign": true, "claimable": status == "CLAIMABLE",
            "campaignUrl": "https://openapi.qoder.com.cn/growth-page/activity-iframe",
            "campaigns": [{
                "campaignId": "daily-from-live-list", "campaignKey": "daily-key",
                "actionType": "CLAIM_BENEFIT", "claimStatus": status,
                "startAt": 1791943200i64, "endAt": 1792029540i64,
                "benefit": {"kind": "CREDITS", "amount": 100},
                "placements": [{"type": "USAGE", "content": {"zh": {"title": "每日 100 积分"}}}]
            }, {
                "campaignId": "coupon", "actionType": "CLAIM_BENEFIT", "claimStatus": "CLAIMABLE",
                "benefit": {"kind": "REDEMPTION_CODE", "amount": 1}
            }]
        })
    }
    #[test]
    fn official_daily_list_uses_placement_title_and_does_not_duplicate_container() {
        let rs = parse_rewards(&daily_campaign("CLAIMABLE"));
        assert_eq!(rs.len(), 2);
        assert_eq!(rs[0].id, "daily-from-live-list");
        assert_eq!(rs[0].name, "每日 100 积分");
        assert_eq!(rs[0].kind, "credits");
        assert_eq!(rs[0].next_available_at, Some(1791943200000));
        assert_eq!(rs[1].kind, "coupon");
    }
    #[test]
    fn server_disabled_campaigns_cannot_be_claimed() {
        let mut list = daily_campaign("CLAIMABLE");
        list["showCampaign"] = json!(false);
        assert!(parse_rewards(&list).is_empty());
    }

    type Reply = (&'static str, &'static str, u16, Value);
    const LIST: &str = "/sash/api/v1/me/campaigns";
    const CLAIM: &str = "/sash/api/v1/me/campaigns/daily-from-live-list/claim";
    const REWARD: &str = "/sash/api/v1/me/campaigns/daily-from-live-list/reward";

    async fn claim_fixture(replies: Vec<Reply>) -> Vec<RewardOutcome> {
        use axum::{extract::State, http::Request, response::IntoResponse, Json, Router};
        let queue = Arc::new(std::sync::Mutex::new(std::collections::VecDeque::from(
            replies,
        )));
        let state = queue.clone();
        let app = Router::new()
            .fallback(
                |State(queue): State<Arc<std::sync::Mutex<std::collections::VecDeque<Reply>>>>,
                 request: Request<axum::body::Body>| async move {
                    let (method, path, status, value) = queue
                        .lock()
                        .unwrap()
                        .pop_front()
                        .expect("unexpected extra request");
                    assert_eq!(request.method().as_str(), method);
                    assert_eq!(request.uri().path(), path);
                    assert_eq!(request.headers()["authorization"], "Bearer test-token");
                    assert_eq!(request.headers()["cosy-clienttype"], "10");
                    assert_eq!(request.headers()["cosy-machineid"], "saved-device");
                    assert_eq!(request.headers()["cosy-version"], "0.4.4");
                    assert_eq!(request.headers()["user-agent"], "Qoder");
                    assert_eq!(request.headers().get_all("user-agent").iter().count(), 1);
                    (
                        axum::http::StatusCode::from_u16(status).unwrap(),
                        Json(value),
                    )
                        .into_response()
                },
            )
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let api = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async {
            axum::serve(listener, app).await.unwrap();
        });
        let mut client = ProviderClient::new();
        client.http = Client::builder().no_proxy().build().unwrap();
        let mut credentials = empty_credentials();
        credentials.access_token = "test-token".into();
        credentials.device_id = Some("saved-device".into());
        let account = Account {
            id: "test-qoder".into(),
            platform: Platform::Qoder,
            user_id: "test-user".into(),
            label: "test".into(),
            enabled: true,
            status: "ready".into(),
            last_error: None,
            snapshot: None,
        };
        let result = claim_at(&client, &account, &credentials, &api, None).await;
        server.abort();
        assert!(
            queue.lock().unwrap().is_empty(),
            "expected requests were not sent"
        );
        result.unwrap()
    }

    #[tokio::test]
    async fn one_click_claims_only_live_credit_ids_with_official_headers() {
        let results = claim_fixture(vec![
            ("GET", LIST, 200, daily_campaign("CLAIMABLE")),
            ("POST", CLAIM, 200, json!({"status": "CLAIMED"})),
        ])
        .await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, "claimed");
        assert_eq!(results[0].amount, Some(100.0));
        assert_eq!(results[0].name, "每日 100 积分");
    }
    #[tokio::test]
    async fn already_claimed_daily_credits_are_not_posted_again() {
        let results = claim_fixture(vec![("GET", LIST, 200, daily_campaign("CLAIMED"))]).await;
        assert_eq!(results[0].status, "already_claimed");
        assert_eq!(results[0].amount, None);
    }
    #[tokio::test]
    async fn ambiguous_grant_reads_reward_without_reposting() {
        let results = claim_fixture(vec![
            ("GET", LIST, 200, daily_campaign("CLAIMABLE")),
            (
                "POST",
                CLAIM,
                504,
                json!({"errorCode": "GRANT_RESULT_UNKNOWN"}),
            ),
            ("GET", REWARD, 200, json!({"status": "CLAIMED"})),
        ])
        .await;
        assert_eq!(results[0].status, "claimed");
        assert_eq!(results[0].amount, Some(100.0));
        assert!(results[0].message.contains("回查"));
    }
    #[tokio::test]
    async fn pending_grant_never_reports_unconfirmed_points() {
        let results = claim_fixture(vec![
            ("GET", LIST, 200, daily_campaign("CLAIMABLE")),
            ("POST", CLAIM, 200, json!({"status": "PENDING"})),
            ("GET", REWARD, 200, json!({"status": "PENDING"})),
            ("GET", LIST, 200, daily_campaign("CLAIMABLE")),
        ])
        .await;
        assert_eq!(results[0].status, "pending");
        assert_eq!(results[0].amount, None);
    }
    #[tokio::test]
    async fn risk_rejection_is_reported_without_token_refresh_or_extra_claim() {
        let results = claim_fixture(vec![
            ("GET", LIST, 200, daily_campaign("CLAIMABLE")),
            ("POST", CLAIM, 403, json!({"errorCode": "RISK_BLOCKED"})),
            (
                "GET",
                REWARD,
                404,
                json!({"errorCode": "CAMPAIGN_NOT_FOUND"}),
            ),
            ("GET", LIST, 200, daily_campaign("CLAIMABLE")),
        ])
        .await;
        assert_eq!(results[0].status, "failed");
        assert_eq!(results[0].amount, None);
        assert!(results[0].message.contains("RISK_BLOCKED"));
    }
    #[tokio::test]
    async fn an_unavailable_campaign_is_not_submitted() {
        let results = claim_fixture(vec![("GET", LIST, 200, daily_campaign("INELIGIBLE"))]).await;
        assert_eq!(results[0].status, "unavailable");
    }
    #[test]
    fn explicit_non_credit_grant_is_not_counted_as_credits() {
        assert!(!credits_confirmed(
            &json!({"status": "CLAIMED", "benefit": {"kind": "REDEMPTION_CODE"}})
        ));
        assert!(!credits_confirmed(&json!({"success": true})));
    }
    #[test]
    fn quota_includes_dedicated_packs() {
        let ps = parse_packs(
            &json!({"userQuota":{"remaining":5},"addOnQuota":{"remaining":7},"dedicatedResourcePackages":[{"remaining":11}]}),
        );
        assert_eq!(snapshot(ps, vec![], vec![], None).balance, Some(23.0));
    }
}
