use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum McpClientEvent {
    Connected,
    Disconnected,
    Message(String),
    Error(String),
}

#[derive(Debug)]
pub struct McpClient {
    event_tx: mpsc::Sender<McpClientEvent>,
}

impl McpClient {
    pub fn new(event_tx: mpsc::Sender<McpClientEvent>) -> Self {
        Self { event_tx }
    }

    pub async fn connect(&self, url: String, _server_name: String) {
        let client = Client::new();
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            match client.get(&url).send().await {
                Ok(response) => {
                    if !response.status().is_success() {
                        let _ = event_tx
                            .send(McpClientEvent::Error(format!(
                                "Failed to connect: {}",
                                response.status()
                            )))
                            .await;
                        return;
                    }

                    let _ = event_tx.send(McpClientEvent::Connected).await;
                    let mut stream = response.bytes_stream();

                    while let Some(item) = stream.next().await {
                        match item {
                            Ok(bytes) => {
                                let msg = String::from_utf8_lossy(&bytes).to_string();
                                let _ = event_tx.send(McpClientEvent::Message(msg)).await;
                            }
                            Err(e) => {
                                let _ = event_tx
                                    .send(McpClientEvent::Error(format!("Stream error: {}", e)))
                                    .await;
                                break;
                            }
                        }
                    }
                    let _ = event_tx.send(McpClientEvent::Disconnected).await;
                }
                Err(e) => {
                    let _ = event_tx
                        .send(McpClientEvent::Error(format!("Connection error: {}", e)))
                        .await;
                }
            }
        });
    }
}

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