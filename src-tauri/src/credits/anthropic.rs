//! Anthropic Messages compatibility over the gateway's OpenAI-shaped provider stream.
//! https://platform.claude.com/docs/en/api/messages/create
//! https://platform.claude.com/docs/en/build-with-claude/streaming
use super::{Completion, RequestGuard};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Map, Value};

fn fields(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), &'static str> {
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("Messages 请求包含不支持的字段；目前支持文本与客户端工具");
    }
    Ok(())
}

// Cache hints affect optimization, not prompt contents. Accept their standard shape,
// but do not claim cache hits or cache pricing on providers without this capability.
fn cache_hint(object: &Map<String, Value>) -> Result<(), &'static str> {
    if let Some(hint) = object.get("cache_control").filter(|hint| !hint.is_null()) {
        let hint = hint.as_object().ok_or("cache_control 必须是对象")?;
        fields(hint, &["type", "ttl"])?;
        if hint.get("type").and_then(Value::as_str) != Some("ephemeral")
            || hint
                .get("ttl")
                .is_some_and(|ttl| !matches!(ttl.as_str(), Some("5m" | "1h")))
        {
            return Err("仅支持标准 ephemeral 缓存提示");
        }
    }
    Ok(())
}

fn text_block(block: &Value) -> Result<String, &'static str> {
    let block = block.as_object().ok_or("内容块必须是对象")?;
    fields(block, &["type", "text", "cache_control"])?;
    cache_hint(block)?;
    if block.get("type").and_then(Value::as_str) != Some("text") {
        return Err("目前仅支持 text、tool_use、tool_result；不支持视觉、文档或推理签名内容");
    }
    block
        .get("text")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or("text 必须是字符串")
}

fn text_content(content: &Value) -> Result<String, &'static str> {
    if let Some(text) = content.as_str() {
        return Ok(text.into());
    }
    let blocks = content
        .as_array()
        .ok_or("content 必须是字符串或内容块数组")?;
    let mut text = String::new();
    for block in blocks {
        text.push_str(&text_block(block)?);
    }
    Ok(text)
}

fn nonempty<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, &'static str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or("工具缺少有效的 id 或 name")
}

// Reasoning effort hints describe how hard the model should think. The gateway's
// OpenAI-shaped providers decide reasoning depth themselves and surface reasoning
// text separately, so the hint is accepted but not forwarded; rejecting it would
// break Claude-Code-style clients that always send a reasoning configuration.
fn thinking(object: &Map<String, Value>) -> Result<(), &'static str> {
    if let Some(config) = object.get("thinking").filter(|config| !config.is_null()) {
        let config = config.as_object().ok_or("thinking 必须是对象")?;
        fields(config, &["type", "budget_tokens", "display"])?;
        if !matches!(
            config.get("type").and_then(Value::as_str),
            Some("enabled" | "disabled" | "adaptive")
        ) {
            return Err("thinking type 仅支持 enabled、disabled 或 adaptive");
        }
        if let Some(budget) = config.get("budget_tokens") {
            if budget.as_u64().is_none_or(|budget| budget == 0) {
                return Err("thinking budget_tokens 必须是正整数");
            }
        }
        if let Some(display) = config.get("display") {
            if !display.is_null() && !matches!(display.as_str(), Some("omitted" | "summarized")) {
                return Err("thinking display 仅支持 omitted 或 summarized");
            }
        }
    }
    Ok(())
}

// effort 与 thinking 同为推理力度提示，接受但不转发。format 承诺结构化输出，
// 属于上游无法兑现的语义，必须显式拒绝而不是静默丢弃。
fn output_config(object: &Map<String, Value>) -> Result<(), &'static str> {
    if let Some(config) = object.get("output_config").filter(|config| !config.is_null()) {
        let config = config.as_object().ok_or("output_config 必须是对象")?;
        if config.contains_key("format") {
            return Err("output_config.format 结构化输出暂不支持");
        }
        fields(config, &["effort", "task_budget"])?;
        if let Some(budget) = config.get("task_budget").filter(|budget| !budget.is_null()) {
            let budget = budget.as_object().ok_or("output_config task_budget 必须是对象")?;
            fields(budget, &["type", "total", "remaining"])?;
            if budget.get("type").and_then(Value::as_str) != Some("tokens") {
                return Err("output_config task_budget type 仅支持 tokens");
            }
            for field in ["total", "remaining"] {
                if let Some(value) = budget.get(field) {
                    if value.as_u64().is_none_or(|value| value == 0) {
                        return Err("output_config task_budget total/remaining 必须是正整数");
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn request(data: Value) -> Result<Value, &'static str> {
    let object = data.as_object().ok_or("请求必须是 JSON 对象")?;
    fields(
        object,
        &[
            "model",
            "max_tokens",
            "messages",
            "system",
            "stream",
            "tools",
            "tool_choice",
            "temperature",
            "top_p",
            "stop_sequences",
            "metadata",
            "cache_control",
            "thinking",
            "output_config",
        ],
    )?;
    cache_hint(object)?;
    thinking(object)?;
    output_config(object)?;
    if object
        .get("max_tokens")
        .and_then(Value::as_u64)
        .is_none_or(|limit| limit == 0)
    {
        return Err("Messages max_tokens 必须是正整数；不支持缓存预热请求");
    }
    for field in ["temperature", "top_p"] {
        if object
            .get(field)
            .is_some_and(|v| v.as_f64().is_none_or(|n| !(0.0..=1.0).contains(&n)))
        {
            return Err("Messages temperature 和 top_p 必须在 0 到 1 之间");
        }
    }
    let messages = object
        .get("messages")
        .and_then(Value::as_array)
        .filter(|messages| !messages.is_empty())
        .ok_or("messages 必须是非空数组")?;
    let mut converted = Vec::new();
    if let Some(system) = object.get("system") {
        converted.push(json!({"role":"system", "content":text_content(system)?}));
    }
    let mut outstanding = std::collections::HashSet::new();
    for message in messages {
        let message = message.as_object().ok_or("message 必须是对象")?;
        fields(message, &["role", "content"])?;
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .ok_or("message 缺少 role")?;
        if !matches!(role, "user" | "assistant") {
            return Err("Messages 的 role 仅支持 user 和 assistant；system 使用顶层字段");
        }
        let content = message.get("content").ok_or("message 缺少 content")?;
        let blocks = match content {
            Value::String(text) => vec![json!({"type":"text", "text":text})],
            Value::Array(blocks) if !blocks.is_empty() => blocks.clone(),
            _ => return Err("content 必须是字符串或非空内容块数组"),
        };
        let mut text = String::new();
        let mut calls = Vec::new();
        let mut saw_text = false;
        for block in blocks {
            let block_object = block.as_object().ok_or("内容块必须是对象")?;
            match block_object.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if !calls.is_empty() {
                        return Err("不支持 tool_use 后交错的文本块；请将说明文本放在工具调用前");
                    }
                    saw_text = true;
                    text.push_str(&text_block(&block)?);
                }
                Some("tool_use") if role == "assistant" => {
                    fields(
                        block_object,
                        &["type", "id", "name", "input", "cache_control"],
                    )?;
                    cache_hint(block_object)?;
                    let id = nonempty(block_object, "id")?;
                    let name = nonempty(block_object, "name")?;
                    let input = block_object
                        .get("input")
                        .filter(|input| input.is_object())
                        .ok_or("tool_use input 必须是对象")?;
                    if !outstanding.insert(id.to_owned()) {
                        return Err("tool_use id 重复");
                    }
                    calls.push(json!({"id":id, "type":"function", "function":{"name":name,"arguments":input.to_string()}}));
                }
                Some("tool_result") if role == "user" => {
                    if saw_text {
                        return Err("tool_result 必须在 user 消息的文本块之前");
                    }
                    fields(
                        block_object,
                        &[
                            "type",
                            "tool_use_id",
                            "content",
                            "is_error",
                            "cache_control",
                        ],
                    )?;
                    cache_hint(block_object)?;
                    let id = nonempty(block_object, "tool_use_id")?;
                    if !outstanding.remove(id) {
                        return Err("tool_result 未对应先前的 tool_use，或结果重复");
                    }
                    let mut result = block_object
                        .get("content")
                        .map(text_content)
                        .transpose()?
                        .unwrap_or_default();
                    if let Some(is_error) = block_object.get("is_error") {
                        let is_error = is_error
                            .as_bool()
                            .ok_or("tool_result is_error 必须是布尔值")?;
                        if is_error {
                            // OpenAI has no tool-result error flag; preserve it explicitly in
                            // the text passed to the model instead of dropping the distinction.
                            result = format!("[tool_result is_error=true]\n{result}");
                        }
                    }
                    converted.push(json!({"role":"tool","tool_call_id":id,"content":result}));
                }
                _ => {
                    return Err(
                        "不支持的内容块或角色；仅支持文本与客户端工具，不支持视觉、文档或推理签名",
                    )
                }
            }
        }
        if saw_text || !calls.is_empty() {
            let mut message = json!({"role":role,"content":text});
            if !calls.is_empty() {
                message["tool_calls"] = json!(calls);
            }
            converted.push(message);
        }
    }
    if !outstanding.is_empty() {
        return Err("历史 tool_use 必须带对应的 user tool_result");
    }
    let mut output = Map::new();
    for field in ["model", "max_tokens", "stream", "temperature", "top_p"] {
        if let Some(value) = object.get(field) {
            output.insert(field.into(), value.clone());
        }
    }
    output.insert("messages".into(), json!(converted));
    if let Some(stops) = object.get("stop_sequences") {
        let stops = stops.as_array().ok_or("stop_sequences 必须是字符串数组")?;
        if stops
            .iter()
            .any(|stop| stop.as_str().is_none_or(str::is_empty))
        {
            return Err("stop_sequences 必须是非空字符串数组");
        }
        if !stops.is_empty() {
            // These providers report only finish_reason=stop, without the matched
            // sequence. Do not invent Anthropic's stop_sequence/stop_reason.
            return Err("非空 stop_sequences 暂不支持，请使用默认停止行为");
        }
    }
    if let Some(metadata) = object.get("metadata") {
        let metadata = metadata.as_object().ok_or("metadata 必须是对象")?;
        fields(metadata, &["user_id"])?;
        if let Some(user) = metadata.get("user_id").filter(|user| !user.is_null()) {
            if user.as_str().is_none_or(|user| user.len() > 512) {
                return Err("metadata.user_id 必须是至多 512 字节的字符串");
            }
            output.insert("user".into(), user.clone());
        }
    }
    if let Some(tools) = object.get("tools") {
        let tools = tools.as_array().ok_or("tools 必须是数组")?;
        let mut names = std::collections::HashSet::new();
        let mut converted_tools = Vec::new();
        for tool in tools {
            let tool = tool.as_object().ok_or("tool 必须是对象")?;
            fields(
                tool,
                &[
                    "name",
                    "description",
                    "input_schema",
                    "cache_control",
                    "type",
                ],
            )?;
            cache_hint(tool)?;
            if tool
                .get("type")
                .is_some_and(|kind| kind.as_str() != Some("custom"))
            {
                return Err("不支持 Anthropic 服务端工具");
            }
            let name = nonempty(tool, "name")?;
            if !names.insert(name) {
                return Err("工具 name 重复");
            }
            let schema = tool
                .get("input_schema")
                .filter(|schema| schema.is_object())
                .ok_or("tool input_schema 必须是对象")?;
            let mut function = json!({"name":name,"parameters":schema});
            if let Some(description) = tool.get("description") {
                if !description.is_string() {
                    return Err("工具 description 必须是字符串");
                }
                function["description"] = description.clone();
            }
            converted_tools.push(json!({"type":"function","function":function}));
        }
        output.insert("tools".into(), json!(converted_tools));
    }
    if let Some(choice) = object.get("tool_choice") {
        let choice = choice.as_object().ok_or("tool_choice 必须是对象")?;
        fields(choice, &["type", "name", "disable_parallel_tool_use"])?;
        let kind = choice
            .get("type")
            .and_then(Value::as_str)
            .ok_or("tool_choice 缺少 type")?;
        let converted = match kind {
            "auto" => json!("auto"),
            "any" => json!("required"),
            "none" => json!("none"),
            "tool" => {
                let name = nonempty(choice, "name")?;
                if !output
                    .get("tools")
                    .and_then(Value::as_array)
                    .is_some_and(|tools| tools.iter().any(|tool| tool["function"]["name"] == name))
                {
                    return Err("tool_choice 指定的工具未定义");
                }
                json!({"type":"function","function":{"name":name}})
            }
            _ => return Err("不支持的 tool_choice type"),
        };
        if kind != "tool" && choice.contains_key("name") {
            return Err("仅 tool_choice type=tool 可以指定 name");
        }
        if matches!(kind, "any" | "tool")
            && !output
                .get("tools")
                .and_then(Value::as_array)
                .is_some_and(|tools| !tools.is_empty())
        {
            return Err("强制工具调用需要非空 tools");
        }
        output.insert("tool_choice".into(), converted);
        if let Some(disabled) = choice.get("disable_parallel_tool_use") {
            output.insert(
                "parallel_tool_calls".into(),
                json!(!disabled
                    .as_bool()
                    .ok_or("disable_parallel_tool_use 必须是布尔值")?),
            );
        }
    }
    Ok(Value::Object(output))
}

fn error_value(status: StatusCode, message: &str) -> Value {
    let kind = match status.as_u16() {
        401 => "authentication_error",
        402 => "billing_error",
        403 => "permission_error",
        404 => "not_found_error",
        413 => "request_too_large",
        429 => "rate_limit_error",
        504 => "timeout_error",
        503 | 529 => "overloaded_error",
        400..=499 => "invalid_request_error",
        _ => "api_error",
    };
    json!({"type":"error","error":{"type":kind,"message":message}})
}

pub(super) fn error(status: StatusCode, message: &str) -> Response {
    (status, Json(error_value(status, message))).into_response()
}

pub(super) fn stream_error(message: &str) -> String {
    event(error_value(StatusCode::BAD_GATEWAY, message))
}

fn event(value: Value) -> String {
    format!(
        "event: {}\ndata: {value}\n\n",
        value["type"].as_str().unwrap_or("error")
    )
}

fn tools(completion: &Completion) -> Result<Vec<Value>, &'static str> {
    completion
        .tools
        .values()
        .map(|call| {
            let id = call["id"]
                .as_str()
                .filter(|id| !id.is_empty())
                .ok_or("上游工具调用缺少 id")?;
            let name = call["function"]["name"]
                .as_str()
                .filter(|name| !name.is_empty())
                .ok_or("上游工具调用缺少 name")?;
            let arguments = call["function"]["arguments"]
                .as_str()
                .ok_or("上游工具参数无效")?;
            let input: Value =
                serde_json::from_str(arguments).map_err(|_| "上游工具参数不是有效 JSON")?;
            if !input.is_object() {
                return Err("上游工具参数必须是 JSON 对象");
            }
            Ok(json!({"type":"tool_use","id":id,"name":name,"input":input}))
        })
        .collect()
}

fn stop_reason(completion: &Completion) -> Result<&'static str, &'static str> {
    match completion.finish.as_ref().and_then(Value::as_str) {
        Some("length") => Ok("max_tokens"),
        // WorkBuddy can label a completed tool call "stop". Correct that label
        // without hiding a length limit or an unsupported upstream stop reason.
        Some("stop") | None => Ok(if completion.tools.is_empty() {
            "end_turn"
        } else {
            "tool_use"
        }),
        Some("tool_calls" | "function_call") if !completion.tools.is_empty() => Ok("tool_use"),
        Some("tool_calls" | "function_call") => Err("上游停止原因要求工具调用，但没有返回工具"),
        _ => Err("上游停止原因无法转换为 Messages"),
    }
}

fn usage(completion: &Completion) -> Value {
    let usage = completion.usage.as_ref();
    json!({"input_tokens":usage.and_then(|v| v["prompt_tokens"].as_u64()).unwrap_or(0),"output_tokens":usage.and_then(|v| v["completion_tokens"].as_u64()).unwrap_or(0)})
}

fn message_id(guard: &RequestGuard) -> String {
    format!("msg_{}", guard.id.trim_start_matches("chatcmpl-"))
}

pub(super) fn response(
    completion: Completion,
    guard: &RequestGuard,
) -> Result<Value, &'static str> {
    let mut content = Vec::new();
    if !completion.content.is_empty() {
        content.push(json!({"type":"text","text":completion.content}));
    }
    content.extend(tools(&completion)?);
    if content.is_empty() {
        content.push(json!({"type":"text","text":""}));
    }
    // reasoning_content is a provider extension, not an Anthropic signed thinking
    // block. Ordinary Messages requests expose final text/tool outputs only.
    Ok(
        json!({"id":message_id(guard),"type":"message","role":"assistant","model":guard.model,"content":content,"stop_reason":stop_reason(&completion)?,"stop_sequence":null,"usage":usage(&completion)}),
    )
}

#[derive(Default)]
pub(super) struct MessageStream {
    completion: Completion,
    text_started: bool,
    received_bytes: usize,
}

impl MessageStream {
    pub(super) fn start(&self, guard: &RequestGuard) -> String {
        event(
            json!({"type":"message_start","message":{"id":message_id(guard),"type":"message","role":"assistant","model":guard.model,"content":[],"stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":0,"output_tokens":0}}}),
        )
    }

    pub(super) fn push(&mut self, data: &Value) -> Result<Vec<String>, &'static str> {
        self.received_bytes += data.to_string().len();
        if self.received_bytes > 16 * 1024 * 1024 {
            return Err("上游 Messages 响应超过内存保护上限");
        }
        self.completion.push(data);
        let mut events = Vec::new();
        let delta = data["choices"]
            .as_array()
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("delta").or_else(|| choice.get("message")));
        if let Some(text) = delta
            .and_then(|delta| delta["content"].as_str())
            .filter(|text| !text.is_empty())
        {
            if !self.text_started {
                self.text_started = true;
                events.push(event(json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}})));
            }
            events.push(event(json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":text}})));
        }
        Ok(events)
    }

    pub(super) fn finish(self) -> Result<Vec<String>, &'static str> {
        let mut events = Vec::new();
        let mut index = 0;
        if self.text_started {
            events.push(event(json!({"type":"content_block_stop","index":0})));
            index = 1;
        }
        // Buffer tool metadata/JSON until complete: OpenAI may split the function
        // name, while Anthropic requires its final name in content_block_start.
        for tool in tools(&self.completion)? {
            let mut start = tool.clone();
            start["input"] = json!({});
            events.push(event(
                json!({"type":"content_block_start","index":index,"content_block":start}),
            ));
            events.push(event(json!({"type":"content_block_delta","index":index,"delta":{"type":"input_json_delta","partial_json":tool["input"].to_string()}})));
            events.push(event(json!({"type":"content_block_stop","index":index})));
            index += 1;
        }
        let usage = usage(&self.completion);
        events.push(event(json!({"type":"message_delta","delta":{"stop_reason":stop_reason(&self.completion)?,"stop_sequence":null},"usage":usage})));
        events.push(event(json!({"type":"message_stop"})));
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_system_tools_and_tool_results_without_losing_error_semantics() {
        let converted = request(json!({"model":"workbuddy/glm-5.3-flash","max_tokens":32,"system":[{"type":"text","text":"instructions","cache_control":{"type":"ephemeral"}}],"messages":[{"role":"assistant","content":[{"type":"text","text":"checking"},{"type":"tool_use","id":"call_1","name":"read","input":{"path":"a"}}]},{"role":"user","content":[{"type":"tool_result","tool_use_id":"call_1","content":[{"type":"text","text":"missing"}],"is_error":true},{"type":"text","text":"try again"}]}],"tools":[{"name":"read","input_schema":{"type":"object"},"cache_control":{"type":"ephemeral","ttl":"1h"}}],"tool_choice":{"type":"tool","name":"read","disable_parallel_tool_use":true},"metadata":{"user_id":"test-user"},"stop_sequences":[],"temperature":0.5,"top_p":0.8})).unwrap();
        assert_eq!(
            converted["messages"][0],
            json!({"role":"system","content":"instructions"})
        );
        assert_eq!(
            converted["messages"][1]["tool_calls"][0]["function"]["arguments"],
            "{\"path\":\"a\"}"
        );
        assert_eq!(
            converted["messages"][2],
            json!({"role":"tool","tool_call_id":"call_1","content":"[tool_result is_error=true]\nmissing"})
        );
        assert_eq!(converted["messages"][3]["content"], "try again");
        assert_eq!(converted["parallel_tool_calls"], false);
        assert_eq!(converted["tool_choice"]["function"]["name"], "read");
        assert_eq!(converted["user"], "test-user");
        assert!(converted.get("stop").is_none());
    }

    #[test]
    fn rejects_unsupported_semantics_instead_of_dropping_them() {
        let base =
            json!({"model":"test","max_tokens":32,"messages":[{"role":"user","content":"hello"}]});
        for kind in [
            "image",
            "document",
            "thinking",
            "redacted_thinking",
            "server_tool_use",
        ] {
            let mut data = base.clone();
            data["messages"][0]["content"] = json!([{"type":kind}]);
            assert!(request(data).is_err(), "{kind}");
        }
        for field in ["thinking", "top_k"] {
            let mut data = base.clone();
            data[field] = json!({});
            assert!(request(data).is_err(), "{field}");
        }
        let mut data = base.clone();
        data["thinking"] = json!({"type":"enabled","budget_tokens":2048});
        data["output_config"] = json!({"effort":"max"});
        let converted = request(data).unwrap();
        assert!(converted.get("thinking").is_none());
        assert!(converted.get("output_config").is_none());
        let mut data = base.clone();
        data["output_config"] = json!({"format":{"type":"json_schema","schema":{}}});
        assert!(request(data).is_err());
        let mut data = base;
        data["messages"][0]["content"] = json!([{"type":"text","text":"hi","citations":[]}]);
        assert!(request(data).is_err());
    }

    #[test]
    fn any_and_none_tool_choices_map_to_openai() {
        for (kind, expected) in [("any", "required"), ("none", "none"), ("auto", "auto")] {
            let converted = request(json!({"model":"test","max_tokens":32,"messages":[{"role":"user","content":"hello"}],"tools":[{"name":"read","input_schema":{"type":"object"}}],"tool_choice":{"type":kind}})).unwrap();
            assert_eq!(converted["tool_choice"], expected);
        }
    }

    #[test]
    fn tool_stop_correction_keeps_length_and_unknown_finish_semantics() {
        for (finish, expected) in [
            ("stop", Ok("tool_use")),
            ("length", Ok("max_tokens")),
            ("content_filter", Err("上游停止原因无法转换为 Messages")),
        ] {
            let mut completion = Completion::default();
            completion.push(&json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"read","arguments":"{}"}}]},"finish_reason":finish}]}));
            assert_eq!(stop_reason(&completion), expected);
            assert_eq!(tools(&completion).unwrap()[0]["input"], json!({}));
        }
        let mut truncated = MessageStream::default();
        truncated.push(&json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"read","arguments":"{\"path\":"}}]},"finish_reason":"length"}]})).unwrap();
        assert!(truncated.finish().is_err());
    }
}
