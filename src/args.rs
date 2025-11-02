use serde_json::{json, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArgsError {
    #[error("Missing required parameter: {0}")]
    MissingRequired(String),
    #[error("Invalid integer value for '{param}': {value}")]
    InvalidInteger { param: String, value: String },
    #[error("Invalid boolean value for '{param}': {value}")]
    InvalidBoolean { param: String, value: String },
    #[error("Too many arguments: expected {expected}, got {got}")]
    TooManyArgs { expected: usize, got: usize },
    #[error("Invalid schema: {0}")]
    InvalidSchema(String),
}

/// Converts command-line arguments to JSON according to JSON Schema
/// 
/// Schema format (from MCP tools/list):
/// ```json
/// {
///   "type": "object",
///   "properties": {
///     "query": { "type": "string", "description": "..." },
///     "limit": { "type": "integer", "nullable": true, "description": "..." }
///   },
///   "required": ["query"]
/// }
/// ```
pub fn args_to_json(args: &[String], schema: &Value) -> Result<Value, ArgsError> {
    // Extract properties and required fields from schema
    let properties = schema
        .get("properties")
        .and_then(|p| p.as_object())
        .ok_or_else(|| ArgsError::InvalidSchema("Missing 'properties' field".into()))?;

    let required: Vec<&str> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .collect()
        })
        .unwrap_or_default();

    // Get ordered list of property names (required first, then optional)
    let mut param_names: Vec<(&str, bool)> = Vec::new();
    
    // Add required params first
    for name in &required {
        if properties.contains_key(*name) {
            param_names.push((name, true));
        }
    }
    
    // Add optional params
    for (name, _) in properties {
        let name_str = name.as_str();
        if !required.contains(&name_str) {
            param_names.push((name_str, false));
        }
    }

    // Check if we have too many arguments
    if args.len() > param_names.len() {
        return Err(ArgsError::TooManyArgs {
            expected: param_names.len(),
            got: args.len(),
        });
    }

    // Build JSON object
    let mut result = serde_json::Map::new();

    for (i, (param_name, is_required)) in param_names.iter().enumerate() {
        if i < args.len() {
            // We have an argument for this parameter
            let arg_value = &args[i];
            let prop_schema = &properties[*param_name];
            
            let json_value = convert_value(arg_value, prop_schema, param_name)?;
            result.insert(param_name.to_string(), json_value);
        } else if *is_required {
            // Missing required parameter
            return Err(ArgsError::MissingRequired(param_name.to_string()));
        }
        // Optional parameters without values are simply not included
    }

    Ok(Value::Object(result))
}

/// Converts a string value to appropriate JSON type based on schema
fn convert_value(value: &str, schema: &Value, param_name: &str) -> Result<Value, ArgsError> {
    let type_name = schema
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("string");

    match type_name {
        "string" => Ok(json!(value)),
        
        "integer" => {
            value.parse::<i64>()
                .map(|n| json!(n))
                .map_err(|_| ArgsError::InvalidInteger {
                    param: param_name.to_string(),
                    value: value.to_string(),
                })
        }
        
        "number" => {
            value.parse::<f64>()
                .map(|n| json!(n))
                .map_err(|_| ArgsError::InvalidInteger {
                    param: param_name.to_string(),
                    value: value.to_string(),
                })
        }
        
        "boolean" => {
            match value.to_lowercase().as_str() {
                "true" | "t" | "yes" | "y" | "1" => Ok(json!(true)),
                "false" | "f" | "no" | "n" | "0" => Ok(json!(false)),
                _ => Err(ArgsError::InvalidBoolean {
                    param: param_name.to_string(),
                    value: value.to_string(),
                }),
            }
        }
        
        // For arrays and objects, try to parse as JSON
        "array" | "object" => {
            serde_json::from_str(value)
                .map_err(|_| ArgsError::InvalidSchema(
                    format!("Cannot parse '{}' as {}", value, type_name)
                ))
        }
        
        _ => Ok(json!(value)), // Fallback to string
    }
}

/// Generates usage hint from schema
pub fn usage_hint(tool_name: &str, schema: &Value) -> String {
    let properties = match schema.get("properties").and_then(|p| p.as_object()) {
        Some(p) => p,
        None => return format!(":mcp run {}", tool_name),
    };

    let required: Vec<&str> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let mut parts = vec![format!(":mcp run {}", tool_name)];

    // Add required params
    for name in &required {
        if let Some(prop) = properties.get(*name) {
            let type_hint = prop
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("value");
            parts.push(format!("<{}:{}>", name, type_hint));
        }
    }

    // Add optional params
    for (name, prop) in properties {
        let name_str = name.as_str();
        if !required.contains(&name_str) {
            let type_hint = prop
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("value");
            parts.push(format!("[{}:{}]", name, type_hint));
        }
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_string_param() {
        let schema = json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" }
            },
            "required": ["query"]
        });

        let args = vec!["U*".to_string()];
        let result = args_to_json(&args, &schema).unwrap();
        
        assert_eq!(result, json!({"query": "U*"}));
    }

    #[test]
    fn test_string_and_integer() {
        let schema = json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "limit": { "type": "integer", "nullable": true }
            },
            "required": ["query"]
        });

        let args = vec!["U*".to_string(), "50".to_string()];
        let result = args_to_json(&args, &schema).unwrap();
        
        assert_eq!(result, json!({"query": "U*", "limit": 50}));
    }

    #[test]
    fn test_optional_param_omitted() {
        let schema = json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "limit": { "type": "integer", "nullable": true }
            },
            "required": ["query"]
        });

        let args = vec!["U*".to_string()];
        let result = args_to_json(&args, &schema).unwrap();
        
        assert_eq!(result, json!({"query": "U*"}));
    }

    #[test]
    fn test_missing_required() {
        let schema = json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" }
            },
            "required": ["query"]
        });

        let args = vec![];
        let result = args_to_json(&args, &schema);
        
        assert!(matches!(result, Err(ArgsError::MissingRequired(_))));
    }

    #[test]
    fn test_invalid_integer() {
        let schema = json!({
            "type": "object",
            "properties": {
                "limit": { "type": "integer" }
            },
            "required": ["limit"]
        });

        let args = vec!["not_a_number".to_string()];
        let result = args_to_json(&args, &schema);
        
        assert!(matches!(result, Err(ArgsError::InvalidInteger { .. })));
    }

    #[test]
    fn test_boolean_conversion() {
        let schema = json!({
            "type": "object",
            "properties": {
                "enabled": { "type": "boolean" }
            },
            "required": ["enabled"]
        });

        let args = vec!["true".to_string()];
        let result = args_to_json(&args, &schema).unwrap();
        assert_eq!(result, json!({"enabled": true}));

        let args = vec!["false".to_string()];
        let result = args_to_json(&args, &schema).unwrap();
        assert_eq!(result, json!({"enabled": false}));
    }

    #[test]
    fn test_usage_hint() {
        let schema = json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search pattern" },
                "limit": { "type": "integer", "nullable": true }
            },
            "required": ["query"]
        });

        let hint = usage_hint("search_components", &schema);
        assert!(hint.contains("search_components"));
        assert!(hint.contains("query"));
        assert!(hint.contains("limit"));
    }
}