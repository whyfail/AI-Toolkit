//! OpenAI request conversion for MiniMax Code's managed Anthropic endpoint.
use super::*;

fn invalid(message: &str) -> ProviderError {
    ProviderError { stage: "validation", ..ProviderError::new(400, message) }
}

fn content(value: &Value) -> Result<Vec<Value>, ProviderError> {
    if value.is_null() { return Ok(vec![]); }
    if let Some(text) = value.as_str() {
        return Ok(if text.is_empty() { vec![] } else { vec![json!({"type":"text","text":text})] });
    }
    value.as_array().ok_or_else(|| invalid("MiniMax Code 消息内容必须是文本或内容块数组"))?
        .iter().map(|part| match part["type"].as_str() {
            Some("text") => part["text"].as_str().map(|text| json!({"type":"text","text":text}))
                .ok_or_else(|| invalid("MiniMax Code 文本块格式无效")),
            Some("image_url") => {
                let url = part["image_url"]["url"].as_str().ok_or_else(|| invalid("图片缺少 URL"))?;
                let source = if let Some(data) = url.strip_prefix("data:") {
                    let (media, data) = data.split_once(";base64,").ok_or_else(|| invalid("图片 data URL 无效"))?;
                    json!({"type":"base64","media_type":media,"data":data})
                } else { json!({"type":"url","url":url}) };
                Ok(json!({"type":"image","source":source}))
            }
            _ => Err(invalid("MiniMax Code 仅支持文本和图片内容")),
        }).collect()
}

pub(super) fn request_body(body: &Value, model: &ModelInfo) -> Result<Value, ProviderError> {
    for field in ["seed", "presence_penalty", "frequency_penalty"] {
        if body.get(field).is_some_and(|value| !value.is_null()) {
            return Err(invalid(&format!("MiniMax Code 暂不支持 {field}")));
        }
    }
    if body.get("response_format").is_some_and(|value| !value.is_null() && value["type"] != "text") {
        return Err(invalid("MiniMax Code 暂不支持结构化输出"));
    }
    let mut system = vec![];
    let mut messages: Vec<Value> = vec![];
    for message in body["messages"].as_array().ok_or_else(|| invalid("messages 必须是数组"))? {
        if model.supports_images == Some(false) && message["content"].as_array()
            .is_some_and(|parts| parts.iter().any(|part| part["type"] == "image_url")) {
            return Err(invalid("MiniMax Code 当前模型不支持图片输入"));
        }
        let role = message["role"].as_str().unwrap_or("");
        let mut blocks = content(&message["content"])?;
        if matches!(role, "system" | "developer") {
            if blocks.iter().any(|block| block["type"] != "text") {
                return Err(invalid("MiniMax Code 系统指令仅支持文本"));
            }
            system.extend(blocks);
            continue;
        }
        let role = match role {
            "tool" => {
                let id = message["tool_call_id"].as_str().filter(|id| !id.is_empty())
                    .ok_or_else(|| invalid("工具结果缺少 tool_call_id"))?;
                blocks = vec![json!({"type":"tool_result","tool_use_id":id,"content":blocks})];
                "user"
            }
            "user" => "user",
            "assistant" => {
                if let Some(thinking) = message["reasoning_content"].as_str().filter(|text| !text.is_empty()) {
                    blocks.insert(0, json!({"type":"thinking","thinking":thinking,"signature":""}));
                }
                if let Some(calls) = message["tool_calls"].as_array() {
                    for call in calls {
                        let input: Value = serde_json::from_str(call["function"]["arguments"].as_str().unwrap_or(""))
                            .map_err(|_| invalid("MiniMax Code 历史工具参数必须是有效 JSON 对象"))?;
                        if !input.is_object() { return Err(invalid("MiniMax Code 历史工具参数必须是 JSON 对象")); }
                        blocks.push(json!({"type":"tool_use","id":call["id"],"name":call["function"]["name"],"input":input}));
                    }
                }
                "assistant"
            }
            _ => return Err(invalid("MiniMax Code 消息角色不受支持")),
        };
        if blocks.is_empty() { blocks.push(json!({"type":"text","text":""})); }
        if let Some(previous) = messages.last_mut().filter(|previous| previous["role"] == role) {
            previous["content"].as_array_mut().unwrap().extend(blocks);
        } else { messages.push(json!({"role":role,"content":blocks})); }
    }
    let max_tokens = body["max_tokens"].as_u64().or_else(|| body["max_completion_tokens"].as_u64()).unwrap_or(4096);
    if model.max_output_tokens.is_some_and(|maximum| max_tokens > maximum) {
        return Err(invalid("请求输出上限超过 MiniMax Code 目录声明的最大输出 Token"));
    }
    let mut output = json!({"model":model.upstream_id.as_deref().unwrap_or(&model.id),"messages":messages,
        "max_tokens":max_tokens,"stream":true});
    if !system.is_empty() { output["system"] = json!(system); }
    for field in ["temperature", "top_p"] {
        if let Some(value) = body.get(field) { output[field] = value.clone(); }
    }
    if let Some(stop) = body.get("stop").filter(|stop| !stop.is_null()) {
        output["stop_sequences"] = if stop.is_string() { json!([stop]) } else { stop.clone() };
    }
    if let Some(tools) = body["tools"].as_array().filter(|tools| !tools.is_empty()) {
        output["tools"] = Value::Array(tools.iter().map(|tool| {
            let function = &tool["function"];
            let mut result = json!({"name":function["name"],"input_schema":function["parameters"]});
            if let Some(description) = function.get("description") { result["description"] = description.clone(); }
            result
        }).collect());
        let choice = &body["tool_choice"];
        output["tool_choice"] = match choice.as_str() {
            Some("required") => json!({"type":"any"}),
            Some("none") => json!({"type":"none"}),
            _ if choice.is_object() => json!({"type":"tool","name":choice["function"]["name"]}),
            _ => json!({"type":"auto"}),
        };
        if let Some(parallel) = body["parallel_tool_calls"].as_bool() {
            output["tool_choice"]["disable_parallel_tool_use"] = json!(!parallel);
        }
    }
    if model.supports_reasoning == Some(true) {
        let effort = body["reasoning_effort"].as_str().or(model.default_reasoning_effort.as_deref());
        if let Some(effort) = effort {
            let effort = match effort {
                "none" => "off",
                "on" if !model.reasoning_effort_options.iter().any(|option| option == "on") => model.default_reasoning_effort.as_deref().unwrap_or("default"),
                effort => effort,
            };
            if effort == "off" && !model.reasoning_effort_options.is_empty()
                && !model.reasoning_effort_options.iter().any(|allowed| allowed == "off") {
                return Err(invalid("MiniMax Code 当前模型强制开启思考，不能使用 disabled／none；请使用 adaptive 或支持的推理等级"));
            }
            if !model.reasoning_effort_options.is_empty() && !model.reasoning_effort_options.iter().any(|allowed| allowed == effort) {
                return Err(invalid("MiniMax Code 推理等级不在当前模型目录允许的范围内"));
            }
            output["thinking"] = json!({"type":if effort == "off" { "disabled" } else { "adaptive" }});
            if !matches!(effort, "on" | "off" | "default") { output["output_config"] = json!({"effort":effort}); }
        }
    } else if body.get("reasoning_effort").is_some() {
        return Err(invalid("MiniMax Code 当前模型未声明思考能力"));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn m3() -> ModelInfo {
        let mut item = model("MiniMax-M3".into(), "M3".into(), Some(400000));
        item.supports_images = Some(true); item.supports_reasoning = Some(true);
        item.reasoning_effort_options = vec!["off".into(), "on".into()];
        item.default_reasoning_effort = Some("on".into()); item
    }
    #[test]
    fn converts_system_history_tools_images_and_results_without_losing_content() {
        let image = "data:image/png;base64,c3ludGhldGlj";
        let input = json!({"max_completion_tokens":123,"reasoning_effort":"off","parallel_tool_calls":false,
            "messages":[{"role":"system","content":"system"},{"role":"developer","content":"developer"},
                {"role":"user","content":[{"type":"text","text":"user"},{"type":"image_url","image_url":{"url":image}}]},
                {"role":"assistant","content":"call","reasoning_content":"history thinking", "tool_calls":[
                    {"id":"call-1","type":"function","function":{"name":"echo","arguments":"{\"value\":\"one\"}"}}]},
                {"role":"tool","tool_call_id":"call-1","content":[{"type":"text","text":"[tool_result is_error=true]\nfailed"},{"type":"image_url","image_url":{"url":image}}]},
                {"role":"user","content":"continue"}],
            "tools":[{"type":"function","function":{"name":"echo","description":"echo","parameters":{"type":"object"}}}],
            "tool_choice":{"type":"function","function":{"name":"echo"}}});
        let converted = request_body(&input, &m3()).unwrap();
        assert_eq!(converted["model"], "MiniMax-M3"); assert_eq!(converted["max_tokens"], 123);
        assert_eq!(converted["thinking"]["type"], "disabled");
        assert_eq!(converted["system"], json!([{"type":"text","text":"system"},{"type":"text","text":"developer"}]));
        assert_eq!(converted["messages"].as_array().unwrap().len(), 3);
        assert_eq!(converted["messages"][0]["content"][1]["source"], json!({"type":"base64","media_type":"image/png","data":"c3ludGhldGlj"}));
        assert_eq!(converted["messages"][1]["content"][0]["thinking"], "history thinking");
        assert_eq!(converted["messages"][1]["content"][2]["input"], json!({"value":"one"}));
        let result = &converted["messages"][2]["content"][0];
        assert_eq!(result["tool_use_id"], "call-1");
        assert_eq!(result["content"][0]["text"], "[tool_result is_error=true]\nfailed");
        assert_eq!(result["content"][1]["type"], "image");
        assert_eq!(converted["messages"][2]["content"][1]["text"], "continue");
        assert_eq!(converted["tool_choice"], json!({"type":"tool","name":"echo","disable_parallel_tool_use":true}));
        assert_eq!(converted["tools"][0]["input_schema"], json!({"type":"object"}));
    }
    #[test]
    fn catalog_efforts_and_output_limits_control_the_upstream_request() {
        let body = json!({"messages":[{"role":"user","content":"hello"}],"reasoning_effort":"high"});
        assert_eq!(request_body(&body, &m3()).unwrap_err().status, 400);
        let mut flash = m3(); flash.id = "MiniMax-M3.1-Flash".into();
        flash.reasoning_effort_options = vec!["default".into(), "low".into(), "high".into()];
        flash.default_reasoning_effort = Some("default".into());
        assert_eq!(request_body(&body, &flash).unwrap()["output_config"]["effort"], "high");
        let mut toggle = body.clone(); toggle["reasoning_effort"] = json!("on");
        let converted = request_body(&toggle, &flash).unwrap();
        assert_eq!(converted["thinking"]["type"], "adaptive"); assert!(converted.get("output_config").is_none());
        toggle["reasoning_effort"] = json!("off");
        let rejected = request_body(&toggle, &flash).unwrap_err();
        assert_eq!(rejected.status, 400);
        assert_eq!(rejected.stage, "validation");
        assert!(rejected.diagnostic().message.contains("强制开启思考"));
        assert!(rejected.diagnostic().message.contains("校验请求参数"));
        toggle["reasoning_effort"] = json!("none"); assert_eq!(request_body(&toggle, &m3()).unwrap()["thinking"]["type"], "disabled");
        let mut limited = m3(); limited.max_output_tokens = Some(32);
        toggle["max_tokens"] = json!(64); assert!(request_body(&toggle, &limited).is_err());
    }
    #[test]
    fn unsupported_semantics_and_invalid_tool_arguments_fail_before_inference() {
        let base = json!({"messages":[{"role":"user","content":"hello"}]});
        for (key,value) in [("seed",json!(1)), ("response_format",json!({"type":"json_object"}))] {
            let mut body = base.clone(); body[key] = value;
            assert_eq!(request_body(&body, &m3()).unwrap_err().status, 400);
        }
        let invalid = json!({"messages":[{"role":"assistant","tool_calls":[{"id":"call","function":{"name":"echo","arguments":"[1,2]"}}]}]});
        assert!(request_body(&invalid, &m3()).is_err());
    }
}
