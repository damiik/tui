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
    ToolsListed(Vec<String>),
}

#[derive(Debug)]
pub struct McpClient {
    event_tx: mpsc::Sender<McpClientEvent>,
    client: Client,
    url: Option<String>,
}

impl McpClient {
    pub fn new(event_tx: mpsc::Sender<McpClientEvent>) -> Self {
        Self { event_tx, client: Client::new(), url: None }
    }

    pub async fn connect(&mut self, url: String, _server_name: String) {
        self.url = Some(url.clone());
        let event_tx = self.event_tx.clone();
        let client = self.client.clone();

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

                    // NEW ✅: Wyślij initialize
                    let init = ops::create_initialize_request("1.0");
                    if let Ok(body) = ops::serialize_request(&init) {
                        let _ = client.post(&url).body(body).send().await;
                    }

                    let _ = event_tx.send(McpClientEvent::Connected).await;

                    let mut stream = response.bytes_stream();

                    while let Some(item) = stream.next().await {
                        println!("mcp stream match");
                        match item {
                            Ok(bytes) => {
                                let msg = String::from_utf8_lossy(&bytes).to_string();
println!("ok bytes: {}", msg.clone());
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&msg) {
                                    if let Some(result) = json.get("result") {
                                        // SUCCESS without matching enum
                                        println!("response ok.");
                                
                                        if let Some(tools) = result.get("tools") {
                                            println!("some tools");
                                            if let Ok(tool_list) = serde_json::from_value::<Vec<String>>(tools.clone()) {
                                                let _ = event_tx.send(McpClientEvent::ToolsListed(tool_list)).await;
                                                continue;
                                            }
                                        }
                                        let _ = event_tx.send(
                                            McpClientEvent::Message(
                                            serde_json::to_string_pretty(&result).unwrap_or(msg)
                                        )).await;

                                        continue;
                                    }
                                }
                                else {

                                    let _ = event_tx.send(McpClientEvent::Message(msg)).await;
                                }
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

    pub async fn list_tools(&self) {
        if let Some(url) = &self.url {
            let list_tools_req = ops::create_list_tools_request();
            let req_body = ops::serialize_request(&list_tools_req).unwrap();
            let client = self.client.clone();
            let url_clone = url.clone();
            let event_tx = self.event_tx.clone();

            tokio::spawn(async move {
                match client.post(url_clone).body(req_body).send().await {
                    Ok(response) => {
                        if !response.status().is_success() {
                            let _ = event_tx
                                .send(McpClientEvent::Error(format!(
                                    "Failed to list tools: {}",
                                    response.status()
                                )))
                                .await;
                        }
                    }
                    Err(e) => {
                        let _ = event_tx
                            .send(McpClientEvent::Error(format!("Failed to list tools: {}", e)))
                            .await;
                    }
                }
            });
        }
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
