//! Structural preflight only: preserve provider extensions and never fetch $ref URLs.
use serde_json::Value;
use std::collections::HashSet;

fn invalid(path: &str, reason: &str) -> String {
    format!("{path}：{reason}")
}

fn member(path: &str, key: &str, index: usize) -> String {
    // A property name can contain arbitrary text. Keep diagnostic paths bounded
    // and omit unusual names rather than echoing user data into errors.
    if !key.is_empty() && key.len() <= 64 && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        format!("{path}.{key}")
    } else {
        format!("{path}[entry {index}]")
    }
}

fn string_set(value: &Value) -> bool {
    let Some(values) = value.as_array() else { return false };
    let mut seen = HashSet::new();
    values.iter().all(|value| value.as_str().is_some_and(|value| seen.insert(value)))
}

fn schema(value: &Value, path: &str) -> Result<(), String> {
    if value.is_boolean() {
        return Ok(());
    }
    let object = value.as_object().ok_or_else(|| invalid(path, "Schema 必须是对象或布尔值"))?;
    for (key, value) in object {
        let field = format!("{path}.{key}");
        match key.as_str() {
            "type" => {
                let valid_type = |value: &Value| matches!(value.as_str(), Some("null" | "boolean" | "object" | "array" | "number" | "integer" | "string"));
                if !valid_type(value) && !(string_set(value) && value.as_array().is_some_and(|values| !values.is_empty() && values.iter().all(valid_type))) {
                    return Err(invalid(&field, "必须是有效的 JSON Schema 类型或不重复的非空类型数组"));
                }
            }
            "required" => {
                if !string_set(value) {
                    return Err(invalid(&field, "必须是不重复的字符串数组"));
                }
            }
            "properties" | "patternProperties" | "$defs" | "definitions" | "dependentSchemas" => {
                let children = value.as_object().ok_or_else(|| invalid(&field, "必须是 Schema 对象映射"))?;
                for (index, (name, child)) in children.iter().enumerate() {
                    schema(child, &member(&field, name, index))?;
                }
            }
            "allOf" | "anyOf" | "oneOf" | "prefixItems" => {
                let children = value.as_array().filter(|values| !values.is_empty())
                    .ok_or_else(|| invalid(&field, "必须是非空 Schema 数组"))?;
                for (index, child) in children.iter().enumerate() {
                    schema(child, &format!("{field}[{index}]"))?;
                }
            }
            "items" if value.is_array() => {
                // Tuple schemas remain valid for older drafts.
                for (index, child) in value.as_array().unwrap().iter().enumerate() {
                    schema(child, &format!("{field}[{index}]"))?;
                }
            }
            "items" | "additionalItems" | "additionalProperties" | "unevaluatedItems"
            | "unevaluatedProperties" | "contains" | "propertyNames" | "not" | "if" | "then" | "else" => schema(value, &field)?,
            "dependentRequired" | "dependencies" => {
                let children = value.as_object().ok_or_else(|| invalid(&field, "必须是对象"))?;
                for (index, (name, child)) in children.iter().enumerate() {
                    let child_path = member(&field, name, index);
                    if key == "dependencies" && !child.is_array() {
                        schema(child, &child_path)?;
                    } else if !string_set(child) {
                        return Err(invalid(&child_path, "必须是不重复的字符串数组"));
                    }
                }
            }
            "$ref" | "$dynamicRef" | "$schema" | "$id" | "title" | "description" | "format" | "pattern" => {
                if !value.is_string() {
                    return Err(invalid(&field, "必须是字符串"));
                }
            }
            "enum" | "examples" => {
                if !value.is_array() {
                    return Err(invalid(&field, "必须是数组"));
                }
            }
            "minLength" | "maxLength" | "minItems" | "maxItems" | "minProperties" | "maxProperties" | "minContains" | "maxContains" => {
                if !value.as_f64().is_some_and(|n| n >= 0.0 && n.fract() == 0.0) {
                    return Err(invalid(&field, "必须是非负整数"));
                }
            }
            "minimum" | "maximum" | "multipleOf" => {
                if !value.is_number() || (key == "multipleOf" && !value.as_f64().is_some_and(|n| n > 0.0)) {
                    return Err(invalid(&field, "必须是数字，multipleOf 必须大于 0"));
                }
            }
            "exclusiveMinimum" | "exclusiveMaximum" => {
                // Draft 4 uses booleans; later drafts use numbers.
                if !value.is_number() && !value.is_boolean() {
                    return Err(invalid(&field, "必须是数字或布尔值"));
                }
            }
            "uniqueItems" | "readOnly" | "writeOnly" | "deprecated" => {
                if !value.is_boolean() {
                    return Err(invalid(&field, "必须是布尔值"));
                }
            }
            // const/default/examples and extension values are data, not schemas.
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn validate(data: &Value, messages: bool) -> Result<(), String> {
    let mut names = HashSet::new();
    if let Some(tools) = data.get("tools") {
        let tools = tools.as_array().ok_or_else(|| invalid("tools", "必须是数组"))?;
        for (index, tool) in tools.iter().enumerate() {
            let path = format!("tools[{index}]");
            let (function, path, schema_key) = if messages {
                (tool, path, "input_schema")
            } else {
                (&tool["function"], format!("{path}.function"), "parameters")
            };
            let name = function.get("name").and_then(Value::as_str).filter(|name| !name.is_empty())
                .ok_or_else(|| invalid(&format!("{path}.name"), "必须是非空字符串"))?;
            if !names.insert(name) {
                return Err(invalid(&format!("{path}.name"), "工具名称重复"));
            }
            if let Some(value) = function.get(schema_key) {
                let path = format!("{path}.{schema_key}");
                if !value.is_object() {
                    return Err(invalid(&path, "工具参数 Schema 必须是对象"));
                }
                schema(value, &path)?;
            } else if messages {
                return Err(invalid(&format!("{path}.{schema_key}"), "缺少工具参数 Schema"));
            }
        }
    }
    if let Some(choice) = data.get("tool_choice") {
        let kind = choice.as_str().or_else(|| choice.get("type").and_then(Value::as_str));
        let name = if messages && kind == Some("tool") {
            Some((&choice["name"], "tool_choice.name"))
        } else if !messages && kind == Some("function") {
            Some((&choice["function"]["name"], "tool_choice.function.name"))
        } else {
            None
        };
        if let Some((name, path)) = name {
            if !name.as_str().is_some_and(|name| names.contains(name)) {
                return Err(invalid(path, "指定的工具未在 tools 中定义"));
            }
        }
        if matches!(kind, Some("required" | "any")) && names.is_empty() {
            return Err(invalid("tool_choice", "强制工具调用时 tools 不能为空"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn structural_errors_name_the_field_without_echoing_its_value() {
        for (schema, field) in [
            (json!({"required":"fixture-secret"}), "required"),
            (json!({"required":["x","x"]}), "required"),
            (json!({"properties":[]}), "properties"),
            (json!({"properties":{"x":{"items":42}}}), "properties.x.items"),
            (json!({"oneOf":[]}), "oneOf"),
            (json!({"anyOf":[{"type":["string","string"]}]}), "anyOf[0].type"),
            (json!({"additionalProperties":"fixture-secret"}), "additionalProperties"),
        ] {
            let request = json!({"tools":[{"name":"test","input_schema":schema}]});
            let error = validate(&request, true).unwrap_err();
            assert!(error.starts_with(&format!("tools[0].input_schema.{field}：")), "{error}");
            assert!(!error.contains("fixture-secret"));
        }
    }

    #[test]
    fn complex_schemas_extensions_and_omitted_parameters_are_preserved() {
        let request = json!({"tools":[{"name":"search","input_schema":{
            "type":"object", "required":["from_combined_schema"],
            "$defs":{"node":{"anyOf":[{"type":"null"},{"$ref":"#/$defs/node"}]}},
            "properties":{"query":{"type":["string","null"]},"filter":true,"tuple":{"items":[{},false]}},
            "oneOf":[{"required":["query"]},{"properties":{"query":{"enum":["x",null]}}}],
            "additionalProperties":false, "default":{"type":"not-a-schema"},
            "x-provider-extension":{"type":"not-a-schema"}
        }}],"tool_choice":{"type":"tool","name":"search"}});
        let before = request.clone();
        validate(&request, true).unwrap();
        assert_eq!(request, before);
        validate(&json!({"tools":[{"type":"function","function":{"name":"noop"}}]}), false).unwrap();
    }

    #[test]
    fn duplicate_names_and_unavailable_choices_are_rejected_for_both_protocols() {
        for messages in [false, true] {
            let tool = if messages { json!({"name":"search","input_schema":{}}) }
                else { json!({"type":"function","function":{"name":"search","parameters":{}}}) };
            assert!(validate(&json!({"tools":[tool.clone(),tool.clone()]}), messages).unwrap_err().contains("tools[1]"));
            let choice = if messages { json!({"type":"tool","name":"missing"}) }
                else { json!({"type":"function","function":{"name":"missing"}}) };
            assert!(validate(&json!({"tools":[tool],"tool_choice":choice}), messages).unwrap_err().contains("tool_choice"));
            let choice = if messages { json!({"type":"any"}) } else { json!("required") };
            assert!(validate(&json!({"tool_choice":choice}), messages).is_err());
        }
    }
}
