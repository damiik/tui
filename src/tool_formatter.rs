// ============================================================================
// Tool formatting utilities - pure functions for tool descriptions
// ============================================================================

use crate::mcp::ToolInfo;
use serde_json::Value;

/// Pure function: ToolInfo → Vec<String>
/// Generates detailed, human-readable tool description
pub fn format_tool_detailed(tool: &ToolInfo) -> Vec<String> {
    let mut lines = Vec::new();

    // Header with tool name
    lines.push("═".repeat(80));
    lines.push(format!("🔧 Tool: {}", tool.name));
    lines.push("═".repeat(80));
    lines.push(String::new());

    // Description
    lines.push("Description:".to_string());
    lines.push(format!("  {}", tool.description));
    lines.push(String::new());

    // Input schema analysis
    let schema_lines = format_input_schema(&tool.input_schema);
    lines.extend(schema_lines);

    // Usage example
    lines.push(String::new());
    lines.push("Usage:".to_string());
    let usage = generate_usage_hint(&tool.name, &tool.input_schema);
    lines.push(format!("  {}", usage));

    lines.push(String::new());
    lines.push("═".repeat(80));

    lines
}

/// Pure function: ToolInfo → String
/// Generates compact one-line summary for tools list
pub fn format_tool_compact(tool: &ToolInfo) -> String {
    let params = extract_param_summary(&tool.input_schema);
    format!("{}: {}", tool.name, params)
}

/// Pure function: extracts parameter summary from schema
fn extract_param_summary(schema: &Value) -> String {
    let properties = match schema.get("properties").and_then(|p| p.as_object()) {
        Some(p) => p,
        None => return "()".to_string(),
    };

    let required: Vec<&str> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let params: Vec<String> = properties
        .iter()
        .map(|(name, prop)| {
            let type_name = prop
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("any");
            
            let is_required = required.contains(&name.as_str());
            
            if is_required {
                format!("{}: {}", name, type_name)
            } else {
                format!("[{}]: {}", name, type_name)
            }
        })
        .collect();

    if params.is_empty() {
        "()".to_string()
    } else {
        format!("({})", params.join(", "))
    }
}

/// Pure function: formats detailed input schema
fn format_input_schema(schema: &Value) -> Vec<String> {
    let mut lines = Vec::new();

    lines.push("Parameters:".to_string());

    let properties = match schema.get("properties").and_then(|p| p.as_object()) {
        Some(p) => p,
        None => {
            lines.push("  (no parameters)".to_string());
            return lines;
        }
    };

    let required: Vec<&str> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    for (name, prop) in properties {
        let is_required = required.contains(&name.as_str());
        let requirement = if is_required { "required" } else { "optional" };

        let type_name = prop
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("any");

        let description = prop
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("(no description)");

        lines.push(String::new());
        lines.push(format!("  • {} ({}, {})", name, type_name, requirement));
        
        // Wrap description at 72 characters
        let wrapped = wrap_text(description, 72, 4);
        for line in wrapped {
            lines.push(line);
        }

        // Additional type information
        if let Some(enum_values) = prop.get("enum").and_then(|e| e.as_array()) {
            let values: Vec<String> = enum_values
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| format!("'{}'", s))
                .collect();
            if !values.is_empty() {
                lines.push(format!("    Allowed values: {}", values.join(", ")));
            }
        }

        if let Some(default) = prop.get("default") {
            lines.push(format!("    Default: {}", default));
        }

        if let Some(min) = prop.get("minimum").and_then(|m| m.as_i64()) {
            lines.push(format!("    Minimum: {}", min));
        }

        if let Some(max) = prop.get("maximum").and_then(|m| m.as_i64()) {
            lines.push(format!("    Maximum: {}", max));
        }
    }

    lines
}

/// Pure function: generates usage hint
fn generate_usage_hint(tool_name: &str, schema: &Value) -> String {
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

    // Required parameters
    for name in &required {
        if let Some(prop) = properties.get(*name) {
            let type_hint = prop
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("value");
            parts.push(format!("<{}:{}>", name, type_hint));
        }
    }

    // Optional parameters
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

/// Pure function: wraps text at specified width with indentation
fn wrap_text(text: &str, width: usize, indent: usize) -> Vec<String> {
    let indent_str = " ".repeat(indent);
    let mut lines = Vec::new();
    let mut current_line = String::new();
    let effective_width = width.saturating_sub(indent);

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + word.len() + 1 <= effective_width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(format!("{}{}", indent_str, current_line));
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() {
        lines.push(format!("{}{}", indent_str, current_line));
    }

    lines
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_param_summary_simple() {
        let schema = json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" }
            },
            "required": ["query"]
        });

        let summary = extract_param_summary(&schema);
        assert_eq!(summary, "(query: string)");
    }

    #[test]
    fn test_extract_param_summary_with_optional() {
        let schema = json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "limit": { "type": "integer" }
            },
            "required": ["query"]
        });

        let summary = extract_param_summary(&schema);
        assert!(summary.contains("query: string"));
        assert!(summary.contains("[limit]: integer"));
    }

    #[test]
    fn test_wrap_text() {
        let text = "This is a very long text that should be wrapped at the specified width";
        let lines = wrap_text(text, 30, 2);
        
        assert!(lines.len() > 1);
        for line in &lines {
            assert!(line.len() <= 30);
            assert!(line.starts_with("  "));
        }
    }

    #[test]
    fn test_generate_usage_hint() {
        let schema = json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "limit": { "type": "integer" }
            },
            "required": ["query"]
        });

        let hint = generate_usage_hint("search", &schema);
        assert!(hint.contains(":mcp run search"));
        assert!(hint.contains("<query:string>"));
        assert!(hint.contains("[limit:integer]"));
    }
}