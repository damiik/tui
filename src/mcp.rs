use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// MCP Protocol Message Types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum McpRequest {
    Initialize { params: InitializeParams },
    ListTools,
    CallTool { params: CallToolParams },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    pub protocol_version: String,
    pub capabilities: HashMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallToolParams {
    pub name: String,
    pub arguments: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpResponse {
    Success { result: serde_json::Value },
    Error { error: McpError },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
}

/// Functional composition for MCP operations
pub mod ops {
    use super::*;

    pub fn create_initialize_request(version: &str) -> McpRequest {
        McpRequest::Initialize {
            params: InitializeParams {
                protocol_version: version.to_string(),
                capabilities: HashMap::new(),
            },
        }
    }

    pub fn create_list_tools_request() -> McpRequest {
        McpRequest::ListTools
    }

    pub fn create_call_tool_request(
        name: String,
        arguments: HashMap<String, serde_json::Value>,
    ) -> McpRequest {
        McpRequest::CallTool {
            params: CallToolParams { name, arguments },
        }
    }

    pub fn serialize_request(request: &McpRequest) -> Result<String, serde_json::Error> {
        serde_json::to_string(request)
    }

    pub fn deserialize_response(data: &str) -> Result<McpResponse, serde_json::Error> {
        serde_json::from_str(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_initialize() {
        let req = ops::create_initialize_request("1.0");
        let serialized = ops::serialize_request(&req).unwrap();
        assert!(serialized.contains("initialize"));
    }

    #[test]
    fn test_serialize_list_tools() {
        let req = ops::create_list_tools_request();
        let serialized = ops::serialize_request(&req).unwrap();
        assert!(serialized.contains("list_tools"));
    }
}
