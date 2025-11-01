use futures_util::StreamExt;
use reqwest::Client;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, Mutex};
use std::sync::atomic::{AtomicI64, Ordering};
use tokio::time::sleep;

// ═══════════════════════════════════════════════════════════════
// EVENTS
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub enum McpClientEvent {
    Connected,
    Disconnected,
    Message(String),
    Error(String),
    ToolsListed(Vec<String>),
    Debug(String),
}

// ═══════════════════════════════════════════════════════════════
// CLIENT
// ═══════════════════════════════════════════════════════════════

#[derive(Debug)]
pub struct McpClient {
    event_tx: mpsc::Sender<McpClientEvent>,
    client: Client,
    base_url: Option<String>,
    session_endpoint: Arc<Mutex<Option<String>>>,
    pending: Arc<Mutex<HashMap<i64, oneshot::Sender<serde_json::Value>>>>,
    next_id: Arc<AtomicI64>,
    sse_shutdown: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl McpClient {
    pub fn new(event_tx: mpsc::Sender<McpClientEvent>) -> Self {
        Self {
            event_tx,
            client: Client::new(),
            base_url: None,
            session_endpoint: Arc::new(Mutex::new(None)),
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicI64::new(1)),
            sse_shutdown: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn connect(&mut self, url: String, server_name: String) {
        self.base_url = Some(url.clone());

        let event_tx = self.event_tx.clone();
        let client = self.client.clone();
        let session_endpoint = self.session_endpoint.clone();
        let pending = self.pending.clone();
        let sse_shutdown = self.sse_shutdown.clone();
        let next_id = self.next_id.clone();

        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        *sse_shutdown.lock().await = Some(shutdown_tx);

        tokio::spawn(async move {
            let _ = event_tx.send(McpClientEvent::Debug(
                format!("🔌 Connecting to {} at {}", server_name, url)
            )).await;

                match client.get(&url).send().await {
                    Ok(response) => {
                    let _ = event_tx.send(McpClientEvent::Debug(
                        format!("📡 Initial response: HTTP {}", response.status())
                    )).await;

                        if !response.status().is_success() {
                            let _ = event_tx.send(McpClientEvent::Error(
                            format!("HTTP connect failed: {}", response.status()),
                            )).await;
                        return;
                        }

                        let _ = event_tx.send(McpClientEvent::Connected).await;
                        let mut stream = response.bytes_stream();
                        let mut buf = String::new();
                        let mut endpoint: Option<String> = None;

                    let _ = event_tx.send(McpClientEvent::Debug(
                        "📥 Waiting for SSE endpoint...".to_string()
                    )).await;

                        // Parse SSE stream to extract endpoint
                        loop {
                            tokio::select! {
                                biased;

                                _ = &mut shutdown_rx => {
                                    let _ = event_tx.send(McpClientEvent::Disconnected).await;
                                    endpoint = None;
                                    break;
                                }

                                item = stream.next() => {
                                    match item {
                                        Some(Ok(chunk)) => {
                                            let txt = String::from_utf8_lossy(&chunk).to_string();
                                            buf.push_str(&txt);

                                            // Process complete SSE messages
                                            while let Some(split) = buf.find("\n\n") {
                                                let block = buf[..split].to_string();
                                                buf = buf[split + 2..].to_string();

                                            let mut event_type = String::new();
                                                let mut data = String::new();

                                                for line in block.lines() {
                                                if let Some(rest) = line.strip_prefix("event:") {
                                                    event_type = rest.trim().to_string();
                                                } else if let Some(rest) = line.strip_prefix("data:") {
                                                        if !data.is_empty() {
                                                            data.push('\n');
                                                        }
                                                        data.push_str(rest.trim());
                                                    }
                                                }

                                                if data.is_empty() {
                                                    continue;
                                                }

                                            let _ = event_tx.send(McpClientEvent::Debug(
                                                format!("📨 SSE event='{}' data='{}'", event_type, data)
                                            )).await;

                                                // Check for endpoint announcement
                                            if event_type == "endpoint" {
                                                endpoint = Some(data.clone());
                                                let _ = event_tx.send(McpClientEvent::Debug(
                                                    format!("✅ Received endpoint: {}", data)
                                                )).await;
                                                        break;
                                                    }

                                                // Try parsing as JSON-RPC
                                                match serde_json::from_str::<serde_json::Value>(&data) {
                                                    Ok(v) => {
                                                    let _ = event_tx.send(McpClientEvent::Debug(
                                                        format!("📦 JSON-RPC: {}", serde_json::to_string(&v).unwrap_or_default())
                                                    )).await;
                                                        handle_json_rpc_event(v, &event_tx, &pending).await;
                                                    }
                                                    Err(_) => {
                                                        let _ = event_tx.send(
                                                            McpClientEvent::Message(data.clone())
                                                        ).await;
                                                    }
                                                }
                                            }

                                            if endpoint.is_some() {
                                                break;
                                            }
                                        }

                                        Some(Err(e)) => {
                                            let _ = event_tx.send(
                                                McpClientEvent::Error(format!("Stream error: {}", e))
                                            ).await;
                                            break;
                                        }

                                    None => {
                                        let _ = event_tx.send(McpClientEvent::Debug(
                                            "⚠️ Stream ended without endpoint".to_string()
                                        )).await;
                                        break;
                                    }
                                    }
                                }
                            }
                        }

                        let endpoint = match endpoint {
                            Some(ep) => ep,
                            None => {
                                let _ = event_tx.send(McpClientEvent::Error(
                                    "No endpoint received from server".into()
                                )).await;
                            return;
                            }
                        };

                        // Store endpoint for future requests
                        {
                            let mut lock = session_endpoint.lock().await;
                            *lock = Some(endpoint.clone());
                        }

                        let full_url = join_url(&url, &endpoint);
                    let _ = event_tx.send(McpClientEvent::Debug(
                        format!("🔗 Session endpoint: {}", full_url)
                    )).await;


                    // Wait a bit for SSE to stabilize
                    sleep(Duration::from_millis(100)).await;

                        // Send initialize request
                    let id = next_id.fetch_add(1, Ordering::SeqCst);
                        let init = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "method": "initialize",
                            "params": {
                                "protocolVersion": "2024-11-05",
                                "capabilities": {},
                                "clientInfo": {
                                    "name": "mcp-client",
                                    "version": "0.1.0"
                                }
                            }
                        });

                    let _ = event_tx.send(McpClientEvent::Debug(
                        format!("📤 Sending initialize: {}", serde_json::to_string(&init).unwrap_or_default())
                    )).await;

                    match client.post(&full_url)
                            .header("Content-Type", "application/json")
                            .body(init.to_string())
                            .send()
                        .await
                    {
                        Ok(resp) => {
                            let _ = event_tx.send(McpClientEvent::Debug(
                                format!("📥 Initialize response: HTTP {}", resp.status())
                            )).await;

                            if resp.status().is_success() {
                                if let Ok(body) = resp.text().await {
                                    let _ = event_tx.send(McpClientEvent::Debug(
                                        format!("📄 Initialize body: {}", body)
                                    )).await;
                                }
                                let _ = event_tx.send(McpClientEvent::Message(
                                    "✅ MCP session initialized".to_string()
                                )).await;
                            } else {
                                let _ = event_tx.send(McpClientEvent::Error(
                                    format!("Initialize failed: {}", resp.status())
                                )).await;
                            }
                    }
                    Err(e) => {
                        let _ = event_tx.send(McpClientEvent::Error(
                                format!("Initialize request failed: {}", e)
                        )).await;
                        }
                    }
                }
                Err(e) => {
                    let _ = event_tx.send(McpClientEvent::Error(
                        format!("Connect error: {}", e)
                    )).await;
                }
            }
        });
    }

    pub async fn list_tools(&self) {
            let id = self.next_id.fetch_add(1, Ordering::SeqCst);

            let req = json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/list",
                "params": {}
            });

        let _ = self.event_tx.send(McpClientEvent::Debug(
            format!("📤 Sending tools/list (id={}): {}", id, serde_json::to_string(&req).unwrap_or_default())
        )).await;

            if let Err(e) = self.send_jsonrpc(req, Some(id)).await {
                let _ = self.event_tx.send(
                    McpClientEvent::Error(format!("list_tools send: {}", e))
                ).await;
            }
    }

    async fn send_jsonrpc(
        &self,
        payload: serde_json::Value,
        expect_id: Option<i64>,
    ) -> Result<(), String> {
        let base = match &self.base_url {
            Some(b) => b.clone(),
            None => return Err("No base URL".into()),
        };

        let ep = {
            let lock = self.session_endpoint.lock().await;
            lock.clone()
        };

        let endpoint_str = match &ep {
            Some(e) => e.clone(),
            None => {
                let _ = self.event_tx.send(McpClientEvent::Debug(
                    "⚠️ No session endpoint, using base URL for request".to_string()
                )).await;
                String::new()
            }
        };

        let url = if endpoint_str.is_empty() {
            base.clone()
        } else {
            join_url(&base, &endpoint_str)
        };

        let _ = self.event_tx.send(McpClientEvent::Debug(
            format!("🌐 POST to: {}", url)
        )).await;

        if let Some(id) = expect_id {
            let (tx, _) = oneshot::channel::<serde_json::Value>();
            self.pending.lock().await.insert(id, tx);
        }

        let resp = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(payload.to_string())
            .send()
            .await;

        match resp {
            Ok(r) => {
                let status = r.status();
                let _ = self.event_tx.send(McpClientEvent::Debug(
                    format!("📥 Response: HTTP {}", status)
                )).await;

                if status.is_success() {
                    if let Ok(body) = r.text().await {
                        let _ = self.event_tx.send(McpClientEvent::Debug(
                            format!("📄 Response body: {}", body)
                        )).await;
                    }
                    Ok(())
                } else {
                    Err(format!("POST HTTP error: {}", status))
                }
            }
            Err(e) => Err(format!("POST error: {}", e)),
        }
    }
}


// ═══════════════════════════════════════════════════════════════
// JSON-RPC EVENT HANDLER
// ═══════════════════════════════════════════════════════════════

async fn handle_json_rpc_event(
    v: serde_json::Value,
    event_tx: &mpsc::Sender<McpClientEvent>,
    pending: &Arc<Mutex<HashMap<i64, oneshot::Sender<serde_json::Value>>>>,
) {
    // Handle responses (with id)
    if let Some(id) = v.get("id").and_then(|v| v.as_i64()) {
        let mut pending_guard = pending.lock().await;
        if let Some(tx) = pending_guard.remove(&id) {
            if let Some(result) = v.get("result") {
                let _ = tx.send(result.clone());

                // Handle tools/list response
                if let Some(tools) = result.get("tools") {
                    if let Some(tools_array) = tools.as_array() {
                        let tool_names: Vec<String> = tools_array
                            .iter()
                            .filter_map(|t| t.get("name")?.as_str())
                            .map(|s| s.to_string())
                            .collect();

                        if !tool_names.is_empty() {
                            let _ = event_tx.send(
                                McpClientEvent::ToolsListed(tool_names)
                            ).await;
                            return;
                        }
                    }
                }

                let _ = event_tx.send(McpClientEvent::Message(
                    serde_json::to_string_pretty(result).unwrap_or_default()
                )).await;
            } else if let Some(error) = v.get("error") {
                let _ = event_tx.send(McpClientEvent::Error(
                    format!("RPC error: {}", 
                        serde_json::to_string_pretty(error).unwrap_or_default()
                    )
                )).await;
            }
            return;
        }
    }

    // Handle notifications (no id)
    if let Some(method) = v.get("method").and_then(|m| m.as_str()) {
        match method {
            "notifications/tools/list_changed" => {
                let _ = event_tx.send(McpClientEvent::Message(
                    "Tools list changed - use :mcp tools to refresh".to_string()
                )).await;
            }
            _ => {
                let _ = event_tx.send(McpClientEvent::Message(
                    format!("Notification: {}", method)
                )).await;
            }
        }
        return;
    }

    // Fallback: display raw JSON
    let _ = event_tx.send(McpClientEvent::Message(
        serde_json::to_string_pretty(&v).unwrap_or_default()
    )).await;
}

// ═══════════════════════════════════════════════════════════════
// UTILITIES
// ═══════════════════════════════════════════════════════════════

/// Join base URL and endpoint path intelligently
/// If endpoint starts with '/', replace the path in base URL
/// Otherwise append to base URL
fn join_url(base: &str, endpoint: &str) -> String {
    // If endpoint is absolute URL, use it directly
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        return endpoint.into();
    }

    // Parse base URL to extract scheme, host, and port
    if let Some(scheme_end) = base.find("://") {
        let scheme = &base[..scheme_end + 3]; // includes ://
        let rest = &base[scheme_end + 3..];
        
        // Find where path starts (after host:port)
        let path_start = rest.find('/').unwrap_or(rest.len());
        let host_port = &rest[..path_start];
        
        // If endpoint starts with '/', it replaces the entire path
        if endpoint.starts_with('/') {
            return format!("{}{}{}", scheme, host_port, endpoint);
        }
        
        // Otherwise, append to existing path
        let existing_path = if path_start < rest.len() {
            &rest[path_start..]
        } else {
            ""
        };
        
        let mut result = format!("{}{}{}", scheme, host_port, existing_path);
        if !result.ends_with('/') && !endpoint.starts_with('/') {
            result.push('/');
        }
        if result.ends_with('/') && endpoint.starts_with('/') {
            result.pop();
        }
        result.push_str(endpoint);
        return result;
    }
    
    // Fallback: simple concatenation
    let mut b = base.to_string();
    if b.ends_with('/') && endpoint.starts_with('/') {
        b.pop();
    }
    if !b.ends_with('/') && !endpoint.starts_with('/') {
        b.push('/');
    }
    b + endpoint
}

#[cfg(test)]
mod url_tests {
    use super::*;

    #[test]
    fn test_join_url_absolute_endpoint() {
        assert_eq!(
            join_url("http://localhost:8080/sse", "/messages?session=123"),
            "http://localhost:8080/messages?session=123"
        );
    }

    #[test]
    fn test_join_url_relative_endpoint() {
        assert_eq!(
            join_url("http://localhost:8080/sse", "messages"),
            "http://localhost:8080/sse/messages"
        );
    }

    #[test]
    fn test_join_url_no_path() {
        assert_eq!(
            join_url("http://localhost:8080", "/messages"),
            "http://localhost:8080/messages"
        );
    }

    #[test]
    fn test_join_url_with_query() {
        assert_eq!(
            join_url("http://localhost:8080/api", "/session?id=abc"),
            "http://localhost:8080/session?id=abc"
        );
    }

    #[test]
    fn test_join_url_full_url_endpoint() {
        assert_eq!(
            join_url("http://localhost:8080/sse", "http://example.com/path"),
            "http://example.com/path"
        );
    }
}