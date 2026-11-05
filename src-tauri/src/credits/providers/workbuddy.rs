use super::*;
const API: &str = "https://copilot.tencent.com";
const BILLING: &str = "https://www.codebuddy.cn";
pub(super) const INTL_API: &str = "https://www.workbuddy.ai";

pub(super) fn api_base(c: &Credentials) -> &'static str {
    match c.region {
        CreditRegion::Cn => API,
        CreditRegion::Intl => INTL_API,
    }
}

fn international_domain(c: &Credentials) -> Result<&'static str, ProviderError> {
    match c.extra.get("authDomain").and_then(Value::as_str) {
        None | Some("" | "www.workbuddy.ai" | "workbuddy.ai") => Ok("www.workbuddy.ai"),
        Some("www.codebuddy.ai" | "codebuddy.ai") => Ok("www.codebuddy.ai"),
        _ => Err(ProviderError::new(
            400,
            "WorkBuddy 国际版凭据的官方账号域名无效",
        )),
    }
}

pub(super) fn headers(r: RequestBuilder, c: &Credentials) -> RequestBuilder {
    if c.region == CreditRegion::Intl {
        let mut r = r
            .bearer_auth(&c.access_token)
            .header("X-User-Id", c.user_id.as_deref().unwrap_or(""))
            .header(
                "X-Domain",
                international_domain(c).unwrap_or("www.workbuddy.ai"),
            )
            .header("X-Product", "SaaS")
            .header("X-IDE-Type", "WorkBuddy")
            .header("X-IDE-Name", "WorkBuddy")
            .header("X-IDE-Version", "5.9.4")
            .header("User-Agent", "WorkBuddy/5.9.4")
            .header("X-Private-Data", "true")
            .header("X-CodeBuddy-Request", "1")
            .header("X-Requested-With", "XMLHttpRequest")
            .header("Accept", "application/json")
            .header("Content-Type", "application/json");
        if let Some(id) = text(&c.extra, &["enterpriseId"]) {
            r = r.header("X-Enterprise-Id", &id).header("X-Tenant-Id", id);
        }
        return r;
    }
    r.bearer_auth(&c.access_token)
        .header("X-User-Id", c.user_id.as_deref().unwrap_or(""))
        .header("X-Domain", "www.codebuddy.cn")
        .header("X-Product", "SaaS")
        .header("X-IDE-Type", "CLI")
        .header("X-IDE-Name", "CLI")
        .header("X-IDE-Version", "2.108.1")
        .header("User-Agent", "CLI/2.108.1 CodeBuddy/2.108.1")
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
}
pub(super) async fn authenticate(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<(String, String, Credentials), ProviderError> {
    if c.region == CreditRegion::Intl {
        let identity = international_identity(client, c).await?;
        balance(client, &identity.2).await?;
        return Ok(identity);
    }
    let mut c = c.clone();
    let jwt = jwt_claims(&c.access_token);
    let uid = text(&jwt, &["sub", "uid"])
        .ok_or_else(|| ProviderError::new(400, "WorkBuddy 凭证必须包含有效账号身份"))?;
    c.user_id = Some(uid.clone());
    packs(client, &c).await?;
    c.expires_at = jwt.get("exp").and_then(epoch).or(c.expires_at);
    let name = text(&jwt, &["nickname", "name", "preferred_username"])
        .unwrap_or_else(|| format!("WorkBuddy {uid}"));
    Ok((uid, name, c))
}

fn parse_international_identity(
    v: &Value,
    c: &Credentials,
) -> Result<(String, String, Credentials), ProviderError> {
    let domain = international_domain(c)?;
    let accounts = data(v)["accounts"]
        .as_array()
        .ok_or_else(|| ProviderError::new(502, "WorkBuddy 国际版账号列表缺失"))?;
    let enterprise = text(&c.extra, &["enterpriseId"]);
    let candidates: Vec<_> = accounts
        .iter()
        .filter(|account| {
            text(account, &["enterpriseId", "enterprise_id"]) == enterprise
                && c.user_id
                    .as_ref()
                    .is_none_or(|uid| text(account, &["uid", "userId"]).as_ref() == Some(uid))
        })
        .collect();
    if candidates.len() != 1 {
        return Err(ProviderError::new(
            401,
            "WorkBuddy 国际版所选账号身份无法确认，请重新登录",
        ));
    }
    let account = candidates[0];
    let uid = text(account, &["uid", "userId"])
        .ok_or_else(|| ProviderError::new(502, "WorkBuddy 国际版账号身份字段缺失"))?;
    let name = text(account, &["nickname", "displayName", "name"])
        .unwrap_or_else(|| "WorkBuddy 国际版".into());
    let mut c = c.clone();
    c.user_id = Some(uid.clone());
    if !c.extra.is_object() {
        c.extra = json!({});
    }
    c.extra["authDomain"] = json!(domain);
    c.extra["accountType"] = account.get("type").cloned().unwrap_or(Value::Null);
    c.extra["enterpriseId"] = enterprise.map(Value::String).unwrap_or(Value::Null);
    Ok((uid, name, c))
}

async fn international_identity(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<(String, String, Credentials), ProviderError> {
    international_domain(c)?;
    let v = client
        .json(headers(
            client.http.get(format!("{INTL_API}/v2/plugin/accounts")),
            c,
        ))
        .await?;
    parse_international_identity(&v, c)
}
pub(super) async fn refresh(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<Credentials, ProviderError> {
    if c.region == CreditRegion::Intl {
        international_domain(c)?;
    }
    let rt = c
        .refresh_token
        .as_ref()
        .ok_or_else(|| ProviderError::new(401, "此凭证没有刷新令牌，请重新登录"))?;
    let v = client
        .json(
            headers(
                client
                    .http
                    .post(format!("{}/v2/plugin/auth/token/refresh", api_base(c))),
                c,
            )
            .header("X-Refresh-Token", rt)
            .header("X-Auth-Refresh-Source", "plugin")
            .json(&json!({})),
        )
        .await?;
    let mut updated = credentials(
        data(&v),
        c,
        &["accessToken", "access_token"],
        &["refreshToken", "refresh_token"],
    )?;
    if c.region == CreditRegion::Intl {
        if let Some(domain) = text(data(&v), &["domain"]) {
            updated.extra["authDomain"] = json!(domain);
        }
        updated = international_identity(client, &updated).await?.2;
    }
    Ok(updated)
}
fn resource_body(page: usize) -> Value {
    json!({"PageNumber":page,"PageSize":100,"ProductCode":"p_tcaca","Status":[0,3],"PackageEndTimeRangeBegin":chrono::Utc::now().with_timezone(&chrono::FixedOffset::east_opt(8*3600).unwrap()).format("%Y-%m-%d %H:%M:%S").to_string(),"PackageEndTimeRangeEnd":"2126-12-31 23:59:59"})
}
async fn packs(client: &ProviderClient, c: &Credentials) -> Result<Vec<CreditPack>, ProviderError> {
    let mut out = vec![];
    for page in 1..=100 {
        let v = client
            .json(
                headers(
                    client
                        .http
                        .post(format!("{BILLING}/v2/billing/meter/get-user-resource")),
                    c,
                )
                .json(&resource_body(page)),
            )
            .await?;
        let d = data(&v)
            .pointer("/Response/Data")
            .ok_or_else(|| ProviderError::new(502, "WorkBuddy 积分数据字段缺失"))?;
        let rows = d["Accounts"]
            .as_array()
            .ok_or_else(|| ProviderError::new(502, "WorkBuddy 积分资源包缺失"))?;
        for (i, p) in rows.iter().enumerate() {
            out.push(CreditPack {
                id: text(p, &["AccountId", "PackageId", "ResourceId"])
                    .unwrap_or(format!("{page}-{i}")),
                name: text(p, &["PackageName"]).unwrap_or("积分包".into()),
                unit: "credits".into(),
                remaining: num(p, &["CycleCapacityRemain", "CapacityRemain"]).map(|n| n.max(0.0)),
                expires_at: p
                    .get("CycleEndTime")
                    .or_else(|| p.get("PackageEndTime"))
                    .and_then(epoch),
            });
        }
        if rows.len() < 100 || num(d, &["TotalCount"]).is_some_and(|n| out.len() as f64 >= n) {
            break;
        }
    }
    Ok(out)
}

fn international_summary(v: &Value) -> Result<(f64, Vec<String>), ProviderError> {
    let rows = data(v)["Packages"]
        .as_array()
        .ok_or_else(|| ProviderError::new(502, "WorkBuddy 国际版积分汇总字段缺失"))?;
    let mut total = 0.0;
    let mut codes = vec![];
    for row in rows {
        let remaining = num(row, &["CycleRemainCapacity"])
            .filter(|value| value.is_finite())
            .ok_or_else(|| ProviderError::new(502, "WorkBuddy 国际版积分汇总金额缺失"))?;
        total += remaining.max(0.0);
        let code = text(row, &["PackageCode"])
            .ok_or_else(|| ProviderError::new(502, "WorkBuddy 国际版积分汇总商品标识缺失"))?;
        if !codes.contains(&code) {
            codes.push(code);
        }
    }
    if !total.is_finite() {
        return Err(ProviderError::new(502, "WorkBuddy 国际版积分汇总金额无效"));
    }
    Ok((total, codes))
}

fn international_resource_body(page: usize, codes: &[String], free: bool) -> Value {
    let mut body = json!({"PageNumber":page,"PageSize":200,"PackageCodes":codes,"Status":[0,3]});
    if free {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        body["SlicePeriodStartTime"] = json!(format!("{today} 00:00:00"));
        body["SlicePeriodEndTime"] = json!(format!("{today} 23:59:59"));
    } else {
        body["NeedRenewInfo"] = json!(true);
    }
    body
}

async fn international_resource_pages(
    client: &ProviderClient,
    c: &Credentials,
    codes: &[String],
    free: bool,
) -> Result<Vec<Value>, ProviderError> {
    let source = if free { "free" } else { "paid" };
    let mut rows = vec![];
    for page in 1..=100 {
        let v = client
            .json(
                headers(
                    client.http.post(format!(
                        "{INTL_API}/billing/meter/get-user-resource-{source}-packages"
                    )),
                    c,
                )
                .header("Accept-Language", "en")
                .json(&international_resource_body(page, codes, free)),
            )
            .await?;
        let d = data(&v);
        let accounts = d["Accounts"]
            .as_array()
            .ok_or_else(|| ProviderError::new(502, "WorkBuddy 国际版积分资源包字段缺失"))?;
        let count = accounts.len();
        rows.extend(accounts.iter().cloned());
        if count < 200 || num(d, &["TotalCount"]).is_some_and(|n| rows.len() as f64 >= n) {
            return Ok(rows);
        }
    }
    Err(ProviderError::new(
        502,
        "WorkBuddy 国际版积分资源包分页未完成",
    ))
}

fn international_packs(paid: &[Value], free: &[Value]) -> Result<Vec<CreditPack>, ProviderError> {
    let mut seen = std::collections::HashSet::new();
    paid.iter()
        .chain(free)
        .filter_map(|row| {
            let Some(id) = text(row, &["ResourceId"]) else {
                return Some(Err(ProviderError::new(
                    502,
                    "WorkBuddy 国际版积分资源包标识缺失",
                )));
            };
            if !seen.insert(id.clone()) {
                return None;
            }
            let remaining = if num(row, &["CapacityType"]) == Some(4.0) {
                row["SlicePeriodUsageDetails"]
                    .as_array()
                    .and_then(|slices| slices.first())
                    .and_then(|slice| num(slice, &["SlicePeriodCapacityRemainPrecise"]))
            } else {
                num(row, &["CycleCapacityRemainPrecise"])
            }
            .filter(|n| n.is_finite())
            .map(|n| n.max(0.0));
            Some(Ok(CreditPack {
                id,
                name: text(row, &["PackageName", "PackageCode"]).unwrap_or_else(|| "积分包".into()),
                unit: "credits".into(),
                remaining,
                expires_at: ["DeductionEndTime", "ExpiredTime", "CycleEndTime"]
                    .iter()
                    .find_map(|key| epoch(&row[*key])),
            }))
        })
        .collect()
}

fn international_balance_snapshot(
    summary: &Value,
    paid: &[Value],
    free: &[Value],
) -> Result<ProviderSnapshot, ProviderError> {
    let (balance, _) = international_summary(summary)?;
    let mut result = snapshot(international_packs(paid, free)?, vec![], vec![], None);
    // The summary is authoritative; resource details describe the same credits.
    result.balance = Some(balance);
    Ok(result)
}

async fn international_balance(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<ProviderSnapshot, ProviderError> {
    international_domain(c)?;
    if text(&c.extra, &["enterpriseId"]).is_some() {
        let v = client
            .json(
                headers(
                    client.http.post(format!(
                        "{INTL_API}/v2/billing/meter/get-enterprise-user-usage"
                    )),
                    c,
                )
                .json(&json!({})),
            )
            .await?;
        let d = data(data(&v));
        let limit = num(d, &["limitNum", "limit_num"])
            .filter(|n| n.is_finite())
            .ok_or_else(|| ProviderError::new(502, "WorkBuddy 国际版团队额度字段缺失"))?;
        let mut result = snapshot(vec![], vec![], vec![], None);
        if limit == -1.0 {
            result.quota = Some("unlimited".into());
        } else {
            let used = num(d, &["credit", "used_num"])
                .filter(|n| n.is_finite())
                .ok_or_else(|| ProviderError::new(502, "WorkBuddy 国际版团队用量字段缺失"))?;
            result.balance = Some((limit - used).max(0.0));
        }
        return Ok(result);
    }
    let summary = client
        .json(
            headers(
                client.http.post(format!(
                    "{INTL_API}/billing/meter/get-user-resource-summary"
                )),
                c,
            )
            .header("Accept-Language", "en")
            .json(&json!({})),
        )
        .await?;
    let (_, codes) = international_summary(&summary)?;
    if codes.is_empty() {
        return international_balance_snapshot(&summary, &[], &[]);
    }
    let (paid, free) = tokio::join!(
        international_resource_pages(client, c, &codes, false),
        international_resource_pages(client, c, &codes, true),
    );
    international_balance_snapshot(&summary, &paid?, &free?)
}

fn reward(v: &Value) -> Reward {
    let d = data(v);
    let d = d
        .get("checkinActivity")
        .or_else(|| d.get("activity"))
        .unwrap_or(d);
    let mut today = flag(
        d,
        &[
            "todayCheckedIn",
            "today_checked_in",
            "checkedIn",
            "checked_in",
        ],
    );
    let day = chrono::Utc::now()
        .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
        .format("%Y-%m-%d")
        .to_string();
    if d.get("checkin_dates")
        .or_else(|| d.get("checkinDates"))
        .and_then(Value::as_array)
        .is_some_and(|ds| {
            ds.iter()
                .any(|v| v.as_str().is_some_and(|s| s.starts_with(&day)))
        })
    {
        today = Some(true)
    }
    let active = flag(d, &["active", "enable", "enabled"]);
    Reward {
        id: "daily-checkin".into(),
        name: "每日签到".into(),
        kind: "credits".into(),
        status: if today == Some(true) {
            "claimed"
        } else if active == Some(false) {
            "unavailable"
        } else if today == Some(false) {
            "claimable"
        } else {
            "unknown"
        }
        .into(),
        amount: num(
            d,
            &["todayCredit", "today_credit", "dailyCredit", "daily_credit"],
        ),
        next_available_at: None,
        action_url: None,
    }
}
pub(super) async fn rewards(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<Vec<Reward>, ProviderError> {
    // International activity credits settle automatically; there is no manual claim.
    if c.region == CreditRegion::Intl {
        return Ok(vec![]);
    }
    let v = client
        .json(
            headers(
                client.http.post(format!(
                    "{BILLING}/v2/billing/meter/checkin-activity-status"
                )),
                c,
            )
            .json(&json!({})),
        )
        .await?;
    let r = reward(&v);
    if r.status != "unknown" {
        return Ok(vec![r]);
    }
    let v = client
        .json(
            headers(
                client
                    .http
                    .post(format!("{BILLING}/v2/billing/meter/checkin-status")),
                c,
            )
            .json(&json!({})),
        )
        .await?;
    Ok(vec![reward(&v)])
}
fn parse_models(v: &Value) -> Vec<ModelInfo> {
    parse_models_for_region(v, CreditRegion::Cn)
}

fn parse_models_for_region(v: &Value, region: CreditRegion) -> Vec<ModelInfo> {
    let d = data(v);
    let allowed = (region == CreditRegion::Intl)
        .then(|| {
            d["agents"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|agent| agent["name"] == "cli")
                .and_then(|agent| agent["models"].as_array())
                .filter(|models| !models.is_empty())
        })
        .flatten();
    data(v)["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let id = text(m, &["id", "model", "modelId", "name"])?;
            if region == CreditRegion::Intl
                && (flag(m, &["disabled"]) == Some(true)
                    || allowed.is_some_and(|models| {
                        !models.iter().any(|model| model.as_str() == Some(&id))
                    }))
            {
                return None;
            }
            if ["nes-", "completion-", "codewise-"]
                .iter()
                .any(|p| id.starts_with(p))
                || flag(m, &["nonChatModel", "supportsExtra"]) == Some(true)
                || flag(m, &["enabled", "enable"]) == Some(false)
                || num(m, &["maxOutputTokens", "maxTokens"]).is_some_and(|n| n <= 256.0)
                || m["tags"]
                    .as_array()
                    .is_some_and(|ts| ts.iter().any(|v| v.as_str() == Some("text-to-image")))
            {
                return None;
            }
            let context_window_options: Vec<u64> = m["contextWindow"]["supportedLengths"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()))
                .filter(|length| *length > 0)
                .fold(vec![], |mut lengths, length| {
                    if !lengths.contains(&length) {
                        lengths.push(length);
                    }
                    lengths
                });
            let context_window = token_limit(&m["contextWindow"], &["defaultLength"])
                .or_else(|| token_limit(m, &["contextWindow", "contextLength", "maxInputTokens"]))
                .into_iter().chain(context_window_options.iter().copied()).max();
            Some(ModelInfo {
                upstream_model_ids: ["model", "modelId"]
                    .iter()
                    .filter_map(|key| underlying_model_id(&m[*key], &id))
                    .fold(vec![], |mut ids, id| {
                        if !ids.contains(&id) {
                            ids.push(id);
                        }
                        ids
                    }),
                context_window_options,
                max_output_tokens: token_limit(m, &["maxOutputTokens"]),
                supports_images: if flag(m, &["disabledMultimodal"]) == Some(true) {
                    Some(false)
                } else {
                    flag(m, &["supportsImages"])
                },
                supports_reasoning: flag(m, &["supportsReasoning"]),
                reasoning_effort_options: effort_options(&m["reasoning"]["supportedEfforts"]),
                default_reasoning_effort: text(&m["reasoning"], &["defaultEffort", "effort"]),
                supports_tools: flag(m, &["supportsToolCall"]).unwrap_or(true),
                ..model(
                    id.clone(),
                    text(m, &["displayName", "display_name", "label", "name"]).unwrap_or(id),
                    context_window,
                )
            })
        })
        .collect()
}

fn catalog_url(c: &Credentials, origin: &str) -> String {
    let enterprise = if c.region == CreditRegion::Cn { None } else { text(&c.extra, &["enterpriseId"]) }
        .unwrap_or_else(|| "personal".into());
    let mut url = reqwest::Url::parse(origin).expect("validated official WorkBuddy origin");
    url.path_segments_mut()
        .expect("official WorkBuddy path")
        .extend(["console", "enterprises", &enterprise]);
    if c.region == CreditRegion::Intl {
        url.path_segments_mut().expect("official WorkBuddy path").push("config");
    }
    url.path_segments_mut().expect("official WorkBuddy path").push("models");
    url.to_string()
}

fn international_model_endpoint(config: &Value) -> Result<String, ProviderError> {
    let raw = text(data(config), &["endpoint"])
        .ok_or_else(|| ProviderError::new(502, "WorkBuddy 国际配置缺少模型服务地址"))?;
    let url = reqwest::Url::parse(&raw)
        .map_err(|_| ProviderError::new(502, "WorkBuddy 国际模型服务地址无效"))?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some()
        || url.port().is_some() || url.path() != "/" || url.query().is_some() || url.fragment().is_some()
        || !matches!(url.host_str(), Some("www.codebuddy.ai" | "www.workbuddy.ai"))
    {
        return Err(ProviderError::new(502, "WorkBuddy 国际模型服务地址不在官方国际域名范围内"));
    }
    Ok(url.origin().ascii_serialization())
}

async fn international_model_config(client: &ProviderClient, c: &Credentials) -> Result<Value, ProviderError> {
    international_domain(c)?;
    client.json(headers(client.http.get(format!("{INTL_API}/v3/config")), c)).await
}

fn international_enterprise_catalog(config: &Value) -> Result<Value, ProviderError> {
    let models = data(config).as_array()
        .ok_or_else(|| ProviderError::new(502, "WorkBuddy 国际企业模型目录格式无效"))?;
    // The official enterprise provider assigns only chat-tagged models to agents.
    let models: Vec<&Value> = models.iter().filter(|model| model["tags"].as_array()
        .is_some_and(|tags| tags.iter().any(|tag| tag == "chat"))).collect();
    Ok(json!({"models": models}))
}

fn international_catalogs(
    account_catalog: Result<Value, ProviderError>,
    v3: Result<Value, ProviderError>,
) -> Result<Vec<ModelInfo>, ProviderError> {
    let mut models = parse_models_for_region(&account_catalog?, CreditRegion::Intl);
    if models.is_empty() {
        return Err(ProviderError::new(
            502,
            "WorkBuddy 国际版账号没有可选择的聊天模型",
        ));
    }
    // The account's product/enterprise catalog is the permission source. Metadata
    // must not resurrect disabled or agent-excluded models from that catalog.
    if let Ok(v) = v3 {
        let metadata = parse_models_for_region(&v, CreditRegion::Intl);
        for model in &mut models {
            if let Some(extra) = metadata.iter().find(|extra| extra.id == model.id) {
                if model.reasoning_effort_options.is_empty() {
                    model.reasoning_effort_options = extra.reasoning_effort_options.clone();
                }
                if model.default_reasoning_effort.is_none() {
                    model.default_reasoning_effort = extra.default_reasoning_effort.clone();
                }
            }
        }
    }
    Ok(models)
}

// The official client ranks plans (free/youth 1, standard 2, advanced 3,
// flagship 4), lets the highest-priority enabled modelTiers entry claim each
// model, and locks models whose tier requires a plan above the account's; the
// upstream enforces the same boundary with code 11158. Neither the config nor
// the accounts endpoint exposes the signed-in user's plan, and personal
// accounts are rejected as free users, so personal accounts keep the free rank.
fn tier_lock_ranks(config: &Value) -> Vec<(String, i64)> {
    fn plan_rank(name: Option<&str>) -> i64 {
        match name.map(str::trim) {
            Some("standard") => 2,
            Some("advanced") => 3,
            Some("flagship") => 4,
            _ => 1, // free, youth, trial and unknown names stay at the entry rank
        }
    }
    let mut locks: Vec<(String, i64)> = Vec::new();
    let mut priorities: Vec<f64> = Vec::new();
    for tier in data(config)["modelTiers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|tier| flag(tier, &["enabled"]) != Some(false))
    {
        let priority = num(tier, &["priority"]).unwrap_or(0.0);
        let rank = plan_rank(
            text(tier, &["requiredUserType"]).as_deref().or(text(tier, &["tier"]).as_deref()),
        );
        for id in tier["modelIds"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str())
        {
            if let Some(index) = locks.iter().position(|(known, _)| known == id) {
                if priority > priorities[index] {
                    locks[index].1 = rank;
                    priorities[index] = priority;
                }
            } else {
                locks.push((id.to_owned(), rank));
                priorities.push(priority);
            }
        }
    }
    locks
}

fn remove_plan_locked_models(models: &mut Vec<ModelInfo>, locks: &[(String, i64)]) {
    models.retain(|model| {
        locks
            .iter()
            .find(|(id, _)| *id == model.id)
            .is_none_or(|(_, rank)| *rank <= 1)
    });
}

pub(super) async fn models(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<Vec<ModelInfo>, ProviderError> {
    if c.region == CreditRegion::Intl {
        let config = international_model_config(client, c).await?;
        let endpoint = international_model_endpoint(&config)?;
        let enterprise = text(&c.extra, &["enterpriseId"]).is_some_and(|id| !id.is_empty());
        let tier_locks = (!enterprise).then(|| tier_lock_ranks(&config));
        let account_catalog = if enterprise {
            let response = client.json(headers(client.http.get(catalog_url(c, &endpoint)), c)).await
                .map_err(|mut error| { error.message = format!("WorkBuddy 国际版企业模型目录：{}", error.message); error });
            response.and_then(|value| international_enterprise_catalog(&value))
        } else {
            // Personal WorkBuddy accounts obtain their model and agent permissions
            // from the authenticated product config, not the legacy IDE /models API.
            Ok(config.clone())
        };
        let mut models = international_catalogs(account_catalog, Ok(config))?;
        if let Some(locks) = tier_locks {
            remove_plan_locked_models(&mut models, &locks);
        }
        return Ok(models);
    }
    let (enterprise, v3) = tokio::join!(
        client.json(headers(client.http.get(catalog_url(c, API)), c)),
        client.json(
            headers(client.http.get(format!("{}/v3/config", api_base(c))), c)
                .header(
                    "User-Agent",
                    if c.region == CreditRegion::Intl {
                        "WorkBuddy/5.9.4"
                    } else {
                        "CodeBuddyIDE/4.12.0 CodeBuddy/4.12.0"
                    }
                )
                .header("X-CodeBuddy-Request", "1")
                .header("X-Requested-With", "XMLHttpRequest")
        )
    );
    let mut models: Vec<ModelInfo> = vec![];
    let mut failed = 0;
    for r in [enterprise, v3] {
        match r {
            Ok(v) => {
                for m in parse_models(&v) {
                    if let Some(existing) = models.iter_mut().find(|n| n.id == m.id) {
                        if existing.reasoning_effort_options.is_empty() {
                            existing.reasoning_effort_options = m.reasoning_effort_options;
                        }
                        if existing.default_reasoning_effort.is_none() {
                            existing.default_reasoning_effort = m.default_reasoning_effort;
                        }
                    } else {
                        models.push(m)
                    }
                }
            }
            Err(_) => failed += 1,
        }
    }
    if models.is_empty() {
        return Err(ProviderError::new(
            502,
            if failed == 2 {
                "WorkBuddy 模型目录查询失败"
            } else {
                "WorkBuddy 账号没有可选择的聊天模型"
            },
        ));
    }
    Ok(models)
}
pub(super) async fn balance(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<ProviderSnapshot, ProviderError> {
    if c.region == CreditRegion::Intl {
        return international_balance(client, c).await;
    }
    Ok(snapshot(packs(client, c).await?, vec![], vec![], None))
}
pub(super) async fn inspect(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<ProviderSnapshot, ProviderError> {
    let (balance, r, m) = tokio::join!(balance(client, c), rewards(client, c), models(client, c));
    let mut balance = balance?;
    let mut diagnostics = vec![];
    let r = r.unwrap_or_else(|e| {
        diagnostics.push(e.message);
        vec![]
    });
    let m = m.unwrap_or_else(|e| {
        diagnostics.push(e.message);
        vec![]
    });
    balance.rewards = r;
    balance.models = m;
    balance.diagnostic = (!diagnostics.is_empty()).then(|| diagnostics.join("；"));
    Ok(balance)
}
pub(super) async fn claim(
    client: &ProviderClient,
    a: &Account,
    c: &Credentials,
) -> Result<Vec<RewardOutcome>, ProviderError> {
    if c.region == CreditRegion::Intl {
        return Ok(vec![]);
    }
    let rs = rewards(client, c).await?;
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
            "当前没有可领取的签到积分",
            None,
        )]);
    }
    let submitted = client
        .json(
            headers(
                client
                    .http
                    .post(format!("{BILLING}/v2/billing/meter/daily-checkin")),
                c,
            )
            .json(&json!({})),
        )
        .await;
    Ok(vec![reconcile_claim(
        a,
        r,
        &submitted,
        rewards(client, c).await,
    )])
}
fn harness_system_text(text: &str) -> String {
    // These two fixed DSH templates were verified separately against WorkBuddy.
    // Keep the Harness identity and system priority; change no user/tool content.
    let text =
        if let Some(rest) = text.strip_prefix("You are an AI agent powered by DeepSeek Harness.") {
            format!("You are an AI agent running on DeepSeek Harness.{rest}")
        } else {
            text.to_owned()
        };
    text.replace(
        "You are interacting with the user through the DeepSeek Harness Web GUI whose URL the runtime context names.",
        "You are interacting with the user through the Web GUI whose URL the runtime context names.",
    )
}
#[cfg(test)]
fn prepare_chat_body(m: &ModelInfo, body: Value) -> Value {
    prepare_chat_body_for_region(m, body, CreditRegion::Cn)
}

fn prepare_chat_body_for_region(m: &ModelInfo, mut body: Value, region: CreditRegion) -> Value {
    body["model"] = json!(m.id);
    body["stream"] = json!(true);
    if let Some(o) = body.as_object_mut() {
        if let Some(v) = o.remove("max_completion_tokens") {
            o.insert("max_tokens".into(), v);
        }
        if let Some(messages) = o.get_mut("messages").and_then(Value::as_array_mut) {
            for message in messages {
                if message["role"].as_str() == Some("developer") {
                    message["role"] = json!("system");
                }
                if message["role"] == "system" && region == CreditRegion::Cn {
                    if let Some(text) = message["content"].as_str() {
                        message["content"] = json!(harness_system_text(text));
                    } else if let Some(parts) = message["content"].as_array_mut() {
                        for part in parts {
                            if part["type"] == "text" {
                                if let Some(text) = part["text"].as_str() {
                                    part["text"] = json!(harness_system_text(text));
                                }
                            }
                        }
                    }
                }
            }
        }
        let disable_tools = o.get("tool_choice").is_some_and(|choice| {
            choice.as_str() == Some("none")
                || choice.get("type").and_then(Value::as_str) == Some("none")
        });
        let choice = o
            .get("tool_choice")
            .and_then(|choice| match choice.get("type").and_then(Value::as_str) {
                Some("auto") => Some("auto"),
                Some("required") => Some("required"),
                Some("function") => choice["function"]["name"].as_str(),
                _ => None,
            })
            .map(str::to_owned);
        if disable_tools {
            o.remove("tool_choice");
            o.remove("tools");
        } else if let Some(choice) = choice {
            o.insert("tool_choice".into(), json!(choice));
        }
    }
    body
}
// The international model gateway passes chat calls only when the official
// client's conversation headers are all present; missing any one is rejected
// with upstream code 11128 channel verification.
fn conversation_headers(r: RequestBuilder) -> RequestBuilder {
    r.header("X-Conversation-ID", uuid::Uuid::new_v4().to_string())
        .header("X-Conversation-Request-ID", uuid::Uuid::new_v4().to_string())
        .header("acp-connection-id", uuid::Uuid::new_v4().to_string())
        .header("X-Conversation-Message-ID", uuid::Uuid::new_v4().to_string())
        .header("X-Request-ID", uuid::Uuid::new_v4().to_string())
}
pub(super) async fn chat(
    client: &ProviderClient,
    c: &Credentials,
    m: &ModelInfo,
    body: Value,
) -> Result<ProviderChat, ProviderError> {
    let endpoint = if c.region == CreditRegion::Intl {
        international_model_endpoint(&international_model_config(client, c).await?)?
    } else { API.into() };
    let request = headers(
        client
            .http
            .post(format!("{endpoint}/v2/chat/completions")),
        c,
    );
    let request = if c.region == CreditRegion::Intl {
        conversation_headers(request)
    } else {
        request
    };
    stream::response(
        request
            .header("Accept", "text/event-stream")
            .json(&prepare_chat_body_for_region(m, body, c.region)),
        Platform::Workbuddy,
    )
    .await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "requires explicitly selected saved international account; read-only"]
    async fn international_saved_account_read_only_smoke() {
        assert_eq!(std::env::var("AI_TOOLKIT_WORKBUDDY_DIAGNOSTIC").as_deref(), Ok("read-only"));
        let path = std::env::var("AI_TOOLKIT_DIAGNOSTIC_DB").expect("explicit application database");
        let db = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let account: String = db.query_row("SELECT metadata FROM credit_accounts WHERE platform='workbuddy' AND region='intl'", [], |row| row.get(0)).unwrap();
        let account: Account = serde_json::from_str(&account).unwrap();
        let secrets = crate::credits::secrets::SecretStore::new();
        if std::env::var("AI_TOOLKIT_DIAGNOSTIC_KEYCHAIN_UI").as_deref() == Ok("once") {
            secrets.retry_access_for(&account.id).expect("one-time keychain authorization");
        }
        let raw = secrets.get(&account.id).expect("read saved credentials").expect("saved credentials");
        let c: Credentials = serde_json::from_str(&raw).unwrap();
        assert_eq!(c.region, CreditRegion::Intl);
        assert_eq!(c.user_id.as_deref(), Some(account.user_id.as_str()));
        let client = ProviderClient::new();
        let snapshot = balance(&client, &c).await.expect("official international billing");
        assert_eq!(snapshot.unit, "credits");
        let models = models(&client, &c).await.expect("official international model catalog");
        assert!(!models.is_empty());
        println!("WorkBuddy international saved-account billing and {} catalog models verified; no inference or claims submitted", models.len());
    }


    fn intl_fixture() -> Value {
        serde_json::from_str(include_str!("fixtures/workbuddy_intl.json")).unwrap()
    }

    fn intl_credentials() -> Credentials {
        serde_json::from_value(json!({
            "region":"intl", "accessToken":"opaque-fixture-token",
            "extra":{"authDomain":"www.workbuddy.ai"}
        }))
        .unwrap()
    }

    #[test]
    fn international_identity_uses_official_accounts_with_opaque_tokens() {
        let mut c = intl_credentials();
        let fixture = intl_fixture();
        let (uid, name, credentials) =
            parse_international_identity(&fixture["accounts"], &c).unwrap();
        assert_eq!(uid, "fixture-user");
        assert_eq!(name, "Fixture account");
        assert_eq!(credentials.user_id.as_deref(), Some("fixture-user"));
        assert_eq!(credentials.extra["accountType"], "personal");
        assert_eq!(credentials.region, CreditRegion::Intl);

        c.user_id = Some("fixture-user".into());
        c.extra["enterpriseId"] = json!("fixture-team");
        let (_, name, credentials) =
            parse_international_identity(&fixture["accounts"], &c).unwrap();
        assert_eq!(name, "Fixture team");
        assert_eq!(credentials.extra["enterpriseId"], "fixture-team");
        c.user_id = Some("another-fixture-user".into());
        assert!(parse_international_identity(&fixture["accounts"], &c).is_err());
        c.user_id = None;
        c.extra["enterpriseId"] = Value::Null;
        assert!(parse_international_identity(
            &json!({"data":{"accounts":[
                {"uid":"first-fixture"},{"uid":"second-fixture"}
            ]}}),
            &c
        )
        .is_err());
    }

    #[test]
    fn account_region_keeps_official_origins_and_domains_separate() {
        let http = Client::new();
        let mut c = intl_credentials();
        c.extra["apiBase"] = json!("https://untrusted.example");
        let request = headers(
            http.post(format!("{}/v2/chat/completions", api_base(&c))),
            &c,
        )
        .build()
        .unwrap();
        assert_eq!(
            request.url().as_str(),
            "https://www.workbuddy.ai/v2/chat/completions"
        );
        assert_eq!(request.headers()["x-domain"], "www.workbuddy.ai");
        assert_eq!(request.headers()["x-ide-type"], "WorkBuddy");
        c.extra["authDomain"] = json!("copilot.tencent.com");
        assert!(international_domain(&c).is_err());
        c.extra["authDomain"] = json!("www.workbuddy.ai.untrusted.example");
        assert!(international_domain(&c).is_err());
        c.extra["authDomain"] = json!("https://www.workbuddy.ai/");
        assert!(international_domain(&c).is_err());

        c.region = CreditRegion::Cn;
        let request = headers(http.get(api_base(&c)), &c).build().unwrap();
        assert_eq!(request.url().host_str(), Some("copilot.tencent.com"));
        assert_eq!(request.headers()["x-domain"], "www.codebuddy.cn");
        assert_eq!(request.headers()["x-ide-type"], "CLI");
    }

    #[test]
    fn international_summary_is_authoritative_without_scaling_or_double_counting() {
        let fixture = intl_fixture();
        let result = international_balance_snapshot(
            &fixture["summary"],
            fixture["paid"].as_array().unwrap(),
            fixture["free"].as_array().unwrap(),
        )
        .unwrap();
        assert_eq!(result.balance, Some(2050.75));
        assert_eq!(result.packs.len(), 2);
        assert_eq!(result.packs[0].remaining, Some(2000.25));
        assert_eq!(result.packs[1].remaining, Some(50.5));
        assert!(result.packs.iter().all(|pack| pack.unit == "credits"));
        assert_eq!(
            international_summary(&fixture["summary"]).unwrap().1,
            vec!["fixture-pro", "fixture-bonus"]
        );
        assert!(
            international_summary(&json!({"data":{"Packages":[{"PackageCode":"fixture"}]}}))
                .is_err()
        );
        assert!(international_packs(&[json!({"CycleCapacityRemainPrecise":100})], &[]).is_err());
        assert_eq!(
            international_balance_snapshot(&json!({"data":{"Packages":[]}}), &[], &[])
                .unwrap()
                .balance,
            Some(0.0)
        );
    }

    #[test]
    fn resource_pages_use_live_summary_codes_and_the_native_free_slice_fields() {
        let codes = vec!["fixture-current-package".into()];
        let paid = international_resource_body(2, &codes, false);
        assert_eq!(paid["PageNumber"], 2);
        assert_eq!(paid["PageSize"], 200);
        assert_eq!(paid["PackageCodes"], json!(codes));
        assert_eq!(paid["NeedRenewInfo"], true);
        assert_eq!(paid["Status"], json!([0, 3]));
        let free = international_resource_body(1, &codes, true);
        assert!(free["SlicePeriodStartTime"]
            .as_str()
            .unwrap()
            .ends_with("00:00:00"));
        assert!(free["SlicePeriodEndTime"]
            .as_str()
            .unwrap()
            .ends_with("23:59:59"));
        assert!(free.get("NeedRenewInfo").is_none());
    }

    #[test]
    fn international_catalog_keeps_non_glm_capabilities_and_account_permissions() {
        let fixture = intl_fixture();
        let models = international_catalogs(
            Ok(fixture["catalog"].clone()),
            Ok(fixture["config"].clone()),
        )
        .unwrap();
        assert_eq!(
            models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["gpt-fixture", "claude-fixture"]
        );
        assert_eq!(models[0].upstream_model_ids, vec!["gpt-fixture-version"]);
        assert_eq!(models[0].context_window, Some(1000000));
        assert_eq!(models[0].context_window_options, vec![128000, 1000000]);
        assert_eq!(models[0].max_output_tokens, Some(32000));
        assert_eq!(
            models[0].reasoning_effort_options,
            vec!["low", "high", "max"]
        );
        assert!(!models[1].supports_tools);
        assert_eq!(models[1].supports_images, Some(false));
        assert!(models
            .iter()
            .all(|m| m.canonical_id.is_none() && !m.verified));
        assert!(international_catalogs(
            Ok(json!({"data":{"models":[]}})),
            Ok(fixture["config"].clone())
        )
        .is_err());
        assert!(international_catalogs(
            Err(ProviderError::new(403, "account catalog denied")),
            Ok(fixture["config"].clone())
        ).is_err());
        let unknown = parse_models_for_region(
            &json!({"models":[{
                "id":"context-fixture", "contextWindow":{"supportedLengths":[1000000]}
            }]}),
            CreditRegion::Intl,
        );
        assert_eq!(unknown[0].context_window, Some(1000000));
    }

    #[test]
    fn observed_international_product_config_supplies_personal_models_and_endpoint() {
        let observed: Value = serde_json::from_str(include_str!("fixtures/workbuddy_intl_catalog_observed.json")).unwrap();
        assert_eq!(international_model_endpoint(&observed).unwrap(), "https://www.codebuddy.ai");
        let models = international_catalogs(Ok(observed.clone()), Ok(observed.clone())).unwrap();
        let allowed = data(&observed)["agents"][0]["models"].as_array().unwrap();
        assert!(!models.is_empty());
        assert!(models.iter().all(|model| allowed.iter().any(|id| id.as_str() == Some(&model.id))));
        assert!(models.iter().any(|model| model.id == "default-model"));
        assert!(!models.iter().any(|model| model.id == "hy-image-3.5"));
        for endpoint in ["https://copilot.tencent.com", "http://www.codebuddy.ai", "https://www.codebuddy.ai.evil.test", "https://user@www.codebuddy.ai", "https://www.codebuddy.ai/private", "https://www.codebuddy.ai?token=fixture", "https://www.codebuddy.ai:8443"] {
            assert!(international_model_endpoint(&json!({"endpoint": endpoint})).is_err());
        }
        assert!(international_model_endpoint(&json!({})).is_err());
    }

    #[test]
    fn international_enterprise_models_obey_chat_tags_and_disabled_state() {
        let enterprise = international_enterprise_catalog(&json!({"data":[
            {"id":"permitted","tags":["chat"]},
            {"id":"disabled","tags":["chat"],"disabled":true},
            {"id":"image","tags":["image"]}
        ]})).unwrap();
        let models = international_catalogs(Ok(enterprise), Ok(json!({"models":[{"id":"disabled"},{"id":"image"}]}))).unwrap();
        assert_eq!(models.iter().map(|model| model.id.as_str()).collect::<Vec<_>>(), vec!["permitted"]);
        assert!(international_enterprise_catalog(&json!({"data":{}})).is_err());
    }

    #[test]
    fn international_catalog_paths_encode_account_metadata() {
        let mut c = intl_credentials();
        assert_eq!(
            catalog_url(&c, "https://www.codebuddy.ai"),
            "https://www.codebuddy.ai/console/enterprises/personal/config/models"
        );
        c.extra["enterpriseId"] = json!("fixture/team?model=other");
        let url = reqwest::Url::parse(&catalog_url(&c, "https://www.codebuddy.ai")).unwrap();
        assert_eq!(url.host_str(), Some("www.codebuddy.ai"));
        assert_eq!(url.query(), None);
        assert!(url.path().contains("fixture%2Fteam%3Fmodel=other"));
    }

    #[test]
    fn international_chat_keeps_system_text_and_tool_history() {
        let m = model("gpt-fixture".into(), "GPT fixture".into(), None);
        let messages = json!([
            {"role":"system","content":"You are an AI agent powered by DeepSeek Harness."},
            {"role":"user","content":[{"type":"image_url","image_url":{"url":"https://example.invalid/fixture.png"}}]},
            {"role":"assistant","tool_calls":[{"id":"fixture-tool","type":"function","function":{"name":"read","arguments":"{}"}}]},
            {"role":"tool","tool_call_id":"fixture-tool","content":"fixture result"}
        ]);
        let converted = prepare_chat_body_for_region(
            &m,
            json!({"messages":messages,"max_completion_tokens":128}),
            CreditRegion::Intl,
        );
        assert_eq!(converted["messages"], messages);
        assert_eq!(converted["model"], "gpt-fixture");
        assert_eq!(converted["max_tokens"], 128);
        assert_eq!(converted["stream"], true);
    }

    #[test]
    fn international_chat_carries_all_conversation_headers() {
        let request = conversation_headers(reqwest::Client::new().post("https://fixture.invalid"))
            .build()
            .unwrap();
        for name in [
            "x-conversation-id",
            "x-conversation-request-id",
            "acp-connection-id",
            "x-conversation-message-id",
            "x-request-id",
        ] {
            assert!(request.headers().contains_key(name), "missing {name}");
        }
    }

    #[test]
    fn international_personal_catalog_hides_plan_locked_models() {
        let config = json!({
            "data": {
                "endpoint": "https://www.codebuddy.ai",
                "modelTiers": [
                    {"id": "gpt-pro-only", "enabled": true, "priority": 200,
                     "requiredUserType": "standard", "tier": "standard",
                     "modelIds": ["gemini-3.8-flash", "grok-4.7"]},
                    {"id": "off", "enabled": false, "modelIds": ["hy4-preview"]}
                ],
                "models": [
                    {"id": "gemini-3.8-flash", "name": "Gemini"},
                    {"id": "grok-4.7", "name": "Grok"},
                    {"id": "deepseek-v4.1-flash-sg", "name": "DeepSeek"},
                    {"id": "hy4-preview", "name": "Hy"}
                ]
            }
        });
        let mut models = parse_models_for_region(&config, CreditRegion::Intl);
        remove_plan_locked_models(&mut models, &tier_lock_ranks(&config));
        let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["deepseek-v4.1-flash-sg", "hy4-preview"]);
    }

    #[tokio::test]
    async fn international_automatic_rewards_never_send_a_claim_request() {
        let c = intl_credentials();
        let client = ProviderClient::new();
        let account: Account = serde_json::from_value(json!({
            "id":"fixture-account", "platform":"workbuddy", "region":"intl", "userId":"fixture-user",
            "label":"Fixture", "enabled":true, "status":"ready", "lastError":null, "snapshot":null
        })).unwrap();
        assert!(rewards(&client, &c).await.unwrap().is_empty());
        assert!(claim(&client, &account, &c).await.unwrap().is_empty());
    }

    #[test]
    fn reasoning_levels_come_from_catalog_options_without_inventing_missing_levels() {
        let models = parse_models(&json!({"models":[
            {"id":"hy4-preview-x","supportsReasoning":true,"reasoning":{"supportedEfforts":[" high ","high","",null,3,"future-level"],"defaultEffort":"high"}},
            {"id":"legacy","supportsReasoning":true,"reasoning":{"effort":"medium"}},
            {"id":"unknown","supportsReasoning":true}
        ]}));
        assert_eq!(
            models[0].reasoning_effort_options,
            vec!["high", "future-level"]
        );
        assert_eq!(models[0].default_reasoning_effort.as_deref(), Some("high"));
        assert!(models[1].reasoning_effort_options.is_empty());
        assert_eq!(
            models[1].default_reasoning_effort.as_deref(),
            Some("medium")
        );
        assert!(models[2].reasoning_effort_options.is_empty());
        assert_eq!(models[2].default_reasoning_effort, None);
    }

    #[test]
    fn catalog_model_ids_do_not_replace_call_ids() {
        let models = parse_models(&json!({"models":[
            {"id":"deepseek-flash","model":"deepseek-v4-1-flash-260910"},
            {"id":"other-model","modelId":"provider-model-version"},
            {"id":"alias","model":"alias"},
            {"id":"unknown"}
        ]}));
        assert_eq!(
            models[0].upstream_model_ids,
            vec!["deepseek-v4-1-flash-260910"]
        );
        assert_eq!(models[1].upstream_model_ids, vec!["provider-model-version"]);
        assert!(models[2].upstream_model_ids.is_empty());
        assert!(models[3].upstream_model_ids.is_empty());
        let b = prepare_chat_body(
            &models[0],
            json!({"messages":[{"role":"user","content":"hello"}]}),
        );
        assert_eq!(b["model"], "deepseek-flash");
    }

    #[test]
    fn full_context_window_takes_precedence_over_input_budget() {
        let models = parse_models(&json!({"models":[
            {"id":"large-window","maxInputTokens":180000,"contextWindow":1000000},
            {"id":"other-window","maxInputTokens":200000,"contextLength":512000},
            {"id":"fixed-window","maxInputTokens":256000},
            {"id":"invalid-window","contextWindow":0,"maxInputTokens":200000}
        ]}));
        assert_eq!(
            models.iter().map(|m| m.context_window).collect::<Vec<_>>(),
            vec![Some(1000000), Some(512000), Some(256000), Some(200000)]
        );
    }

    #[test]
    fn model_info_preserves_explicit_limits_and_capabilities() {
        let models = parse_models(&json!({"models":[
            {"id":"vision-model","maxInputTokens":1000000,"maxOutputTokens":"128000","supportsImages":true,"supportsReasoning":false,"supportsToolCall":false},
            {"id":"glm-5.3-flash"}
        ]}));
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].context_window, Some(1000000));
        assert_eq!(models[0].max_output_tokens, Some(128000));
        assert_eq!(models[0].supports_images, Some(true));
        assert_eq!(models[0].supports_reasoning, Some(false));
        assert!(!models[0].supports_tools);
        assert_eq!(models[1].max_output_tokens, None);
        assert_eq!(models[1].supports_images, None);
        assert_eq!(models[1].supports_reasoning, None);
    }

    #[test]
    fn filters_internal_models_and_uses_live_glm_version() {
        let ms = parse_models(
            &json!({"data":{"models":[{"id":"glm-5.3","displayName":"GLM-5.3"},{"id":"nes-test"},{"id":"img","tags":["text-to-image"]}]}}),
        );
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0].canonical_id.as_deref(), Some("glm-5.3"));
        assert!(!ms[0].verified);
    }
    #[test]
    fn absent_checkin_flag_is_unknown() {
        assert_eq!(reward(&json!({"data":{"active":true}})).status, "unknown");
    }
    #[test]
    fn chat_body_translates_developer_without_rewriting_content() {
        let messages = json!([
            {"role":"developer","content":[{"type":"text","text":"Keep all instructions."}]},
            {"role":"user","content":"hello"}
        ]);
        let body = prepare_chat_body(
            &model("glm-5.3-flash".into(), "GLM-5.3-Flash".into(), None),
            json!({"model":"workbuddy/glm-5.3-flash","stream":false,"messages":messages,"max_completion_tokens":64}),
        );
        assert_eq!(body["model"], "glm-5.3-flash");
        assert_eq!(body["stream"], true);
        assert_eq!(body["max_tokens"], 64);
        assert!(body.get("max_completion_tokens").is_none());
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], messages[0]["content"]);
        assert_eq!(body["messages"][1], messages[1]);
    }

    #[test]
    fn harness_templates_keep_system_priority_and_leave_other_messages_and_tools_unchanged() {
        let original = "You are an AI agent powered by DeepSeek Harness.\nKeep tool instructions verbatim.\nYou are interacting with the user through the DeepSeek Harness Web GUI whose URL the runtime context names.\nKeep the runtime URL and file paths unchanged.";
        let expected = "You are an AI agent running on DeepSeek Harness.\nKeep tool instructions verbatim.\nYou are interacting with the user through the Web GUI whose URL the runtime context names.\nKeep the runtime URL and file paths unchanged.";
        let model = model("deepseek-v4.1-flash".into(), "DeepSeek Flash".into(), None);
        for role in ["system", "developer"] {
            for content in [
                json!(original),
                json!([{"type":"text","text":original},{"type":"text","text":"Read /Users/example/DeepSeek Harness/files verbatim."}]),
            ] {
                let input = json!({"messages":[
                    {"role":role,"content":content},
                    {"role":"user","content":original},
                    {"role":"assistant","content":original,"reasoning_content":"keep this reasoning","tool_calls":[{"id":"call_1","type":"function","function":{"name":"Read","arguments":"{}"}}]},
                    {"role":"tool","tool_call_id":"call_1","content":[{"type":"text","text":original},{"type":"image_url","image_url":{"url":"https://example.test/screenshot.png"}}]}
                ],"tools":[{"type":"function","function":{"name":"Read","description":original,"parameters":{"type":"object"}}}]});
                let mut expected_messages = input["messages"].clone();
                expected_messages[0]["role"] = json!("system");
                if content.is_string() {
                    expected_messages[0]["content"] = json!(expected);
                } else {
                    expected_messages[0]["content"][0]["text"] = json!(expected);
                }
                let converted = prepare_chat_body(&model, input.clone());
                assert_eq!(converted["messages"], expected_messages);
                assert_eq!(converted["tools"], input["tools"]);
                assert_eq!(prepare_chat_body(&model, converted.clone()), converted);
            }
        }
    }

    #[test]
    fn unrelated_system_text_and_harness_names_are_not_rewritten() {
        let model = model("deepseek-v4.1-flash".into(), "DeepSeek Flash".into(), None);
        for text in [
            "Use DeepSeek Harness with the runtime URL.",
            "Quoted example: You are an AI agent powered by DeepSeek Harness.",
            "You are an AI agent powered by DeepSeek Harnessing.",
            "Read /Users/example/DeepSeek Harness/files and the DeepSeek Harness Web GUI.",
        ] {
            let input = json!({"messages":[{"role":"system","content":text},{"role":"user","content":"hello"}]});
            assert_eq!(
                prepare_chat_body(&model, input.clone())["messages"],
                input["messages"]
            );
        }
    }

    #[test]
    fn image_attachments_and_tool_results_preserve_their_content_and_ids() {
        let image = json!({"type":"image_url","image_url":{"url":"data:image/png;base64,aW1hZ2U=","detail":"high"}});
        let remote =
            json!({"type":"image_url","image_url":{"url":"https://example.test/screenshot.png"}});
        let messages = json!([
            {"role":"user","content":[image.clone()]},
            {"role":"assistant","content":"","reasoning_content":"inspect the screenshot","tool_calls":[{"id":"call_image","type":"function","function":{"name":"Read","arguments":"{}"}}]},
            {"role":"tool","tool_call_id":"call_image","content":[{"type":"text","text":"[tool_result is_error=true]\n"},remote.clone(),image.clone()]},
            {"role":"user","content":[{"type":"text","text":"compare both images"},image,remote]},
        ]);
        let m = model("glm-5.3-flash".into(), "GLM-5.3-Flash".into(), None);
        let body = prepare_chat_body(
            &m,
            json!({"model":"workbuddy/glm-5.3-flash","messages":messages,"max_completion_tokens":128}),
        );
        assert_eq!(body["messages"], messages);
        assert_eq!(body["messages"][2]["tool_call_id"], "call_image");
        assert_eq!(
            body["messages"][2]["content"][0]["text"],
            "[tool_result is_error=true]\n"
        );
        assert_eq!(body["model"], "glm-5.3-flash");
        assert_eq!(body["max_tokens"], 128);
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn chat_body_translates_tool_selection_and_preserves_definitions() {
        let m = model("glm-5.3-flash".into(), "GLM-5.3-Flash".into(), None);
        let tools = json!([{"type":"function","function":{"name":"read_file","parameters":{"type":"object"}}}]);
        for (choice, expected) in [
            (
                json!({"type":"function","function":{"name":"read_file"}}),
                "read_file",
            ),
            (json!({"type":"auto"}), "auto"),
            (json!({"type":"required"}), "required"),
            (json!("auto"), "auto"),
            (json!("required"), "required"),
            (json!("read_file"), "read_file"),
        ] {
            let body = prepare_chat_body(&m, json!({"tools":tools,"tool_choice":choice}));
            assert_eq!(body["tool_choice"], expected);
            assert_eq!(body["tools"], tools);
        }
        for choice in [json!("none"), json!({"type":"none"})] {
            let body = prepare_chat_body(&m, json!({"tools":tools,"tool_choice":choice}));
            assert!(body.get("tool_choice").is_none());
            assert!(body.get("tools").is_none());
        }
    }
}
