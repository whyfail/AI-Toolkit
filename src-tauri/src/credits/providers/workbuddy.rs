use super::*;
const API: &str = "https://copilot.tencent.com";
const BILLING: &str = "https://www.codebuddy.cn";
pub(super) fn headers(r: RequestBuilder, c: &Credentials) -> RequestBuilder {
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
pub(super) async fn refresh(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<Credentials, ProviderError> {
    let rt = c
        .refresh_token
        .as_ref()
        .ok_or_else(|| ProviderError::new(401, "此凭证没有刷新令牌，请重新登录"))?;
    let v = client
        .json(
            headers(
                client
                    .http
                    .post(format!("{API}/v2/plugin/auth/token/refresh")),
                c,
            )
            .header("X-Refresh-Token", rt)
            .header("X-Auth-Refresh-Source", "plugin")
            .json(&json!({})),
        )
        .await?;
    credentials(
        data(&v),
        c,
        &["accessToken", "access_token"],
        &["refreshToken", "refresh_token"],
    )
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
async fn rewards(client: &ProviderClient, c: &Credentials) -> Result<Vec<Reward>, ProviderError> {
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
    data(v)["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let id = text(m, &["id", "model", "modelId", "name"])?;
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
            Some(ModelInfo {
                upstream_model_ids: ["model", "modelId"].iter()
                    .find_map(|key| underlying_model_id(&m[*key], &id))
                    .into_iter().collect(),
                max_output_tokens: token_limit(m, &["maxOutputTokens"]),
                supports_images: flag(m, &["supportsImages"]),
                supports_reasoning: flag(m, &["supportsReasoning"]),
                supports_tools: flag(m, &["supportsToolCall"]).unwrap_or(true),
                ..model(
                    id.clone(),
                    text(m, &["displayName", "display_name", "label", "name"]).unwrap_or(id),
                    token_limit(m, &["contextWindow", "contextLength", "maxInputTokens"]),
                )
            })
        })
        .collect()
}
async fn models(client: &ProviderClient, c: &Credentials) -> Result<Vec<ModelInfo>, ProviderError> {
    let (enterprise, v3) = tokio::join!(
        client.json(headers(
            client
                .http
                .get(format!("{API}/console/enterprises/personal/models")),
            c
        )),
        client.json(
            headers(client.http.get(format!("{API}/v3/config")), c)
                .header("User-Agent", "CodeBuddyIDE/4.12.0 CodeBuddy/4.12.0")
                .header("X-CodeBuddy-Request", "1")
                .header("X-Requested-With", "XMLHttpRequest")
        )
    );
    let mut models = vec![];
    let mut failed = 0;
    for r in [enterprise, v3] {
        match r {
            Ok(v) => {
                for m in parse_models(&v) {
                    if !models.iter().any(|n: &ModelInfo| n.id == m.id) {
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
pub(super) async fn inspect(
    client: &ProviderClient,
    c: &Credentials,
) -> Result<ProviderSnapshot, ProviderError> {
    let (p, r, m) = tokio::join!(packs(client, c), rewards(client, c), models(client, c));
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
        p,
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
fn prepare_chat_body(m: &ModelInfo, mut body: Value) -> Value {
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
pub(super) async fn chat(
    client: &ProviderClient,
    c: &Credentials,
    m: &ModelInfo,
    body: Value,
) -> Result<ProviderChat, ProviderError> {
    stream::response(
        headers(client.http.post(format!("{API}/v2/chat/completions")), c)
            .header("Accept", "text/event-stream")
            .json(&prepare_chat_body(m, body)),
        Platform::Workbuddy,
    )
    .await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_model_ids_do_not_replace_call_ids() {
        let models = parse_models(&json!({"models":[
            {"id":"deepseek-flash","model":"deepseek-v4-1-flash-260910"},
            {"id":"other-model","modelId":"provider-model-version"},
            {"id":"alias","model":"alias"},
            {"id":"unknown"}
        ]}));
        assert_eq!(models[0].upstream_model_ids, vec!["deepseek-v4-1-flash-260910"]);
        assert_eq!(models[1].upstream_model_ids, vec!["provider-model-version"]);
        assert!(models[2].upstream_model_ids.is_empty());
        assert!(models[3].upstream_model_ids.is_empty());
        let b = prepare_chat_body(&models[0], json!({"messages":[{"role":"user","content":"hello"}]}));
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
        assert_eq!(models.iter().map(|m| m.context_window).collect::<Vec<_>>(), vec![Some(1000000), Some(512000), Some(256000), Some(200000)]);
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
