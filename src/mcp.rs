use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, Mutex};
use std::sync::atomic::{AtomicI64, Ordering};
use tokio::time::sleep;
use tokio::task;

// helper function for safe JSON formatting
async fn format_json_safely(value: &serde_json::Value) -> String {
    let value_clone = value.clone();
    
    // Run formatting in blocking task to prevent UI freeze
    match task::spawn_blocking(move || {
        serde_json::to_string_pretty(&value_clone)
    }).await {
        Ok(Ok(formatted)) => formatted,
        _ => value.to_string(), // Fallback to compact format
    }
}

// helper to truncate large JSON
fn truncate_json_display(json_str: &str, max_lines: usize) -> (String, bool) {
    let lines: Vec<&str> = json_str.lines().collect();
    
    if lines.len() <= max_lines {
        (json_str.to_string(), false)
    } else {
        let truncated = lines[..max_lines].join("\n");
        (truncated, true)
    }
}

// ═══════════════════════════════════════════════════════════════
// EVENTS
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl std::fmt::Display for ToolInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.name, self.description)
    }
}

#[derive(Debug, Clone)]
pub enum McpClientEvent {
    Connected,
    Disconnected,
    Message(String),
    Error(String),
    ToolsListed(Vec<ToolInfo>),
    Debug(String),
    LargeResponse { total_lines: usize, chunk: String },
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
    available_tools: Arc<Mutex<Vec<ToolInfo>>>,
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
            available_tools: Arc::new(Mutex::new(Vec::new())),
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
        let available_tools = self.available_tools.clone();

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
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
                    
                    // KLUCZ: Rozpocznij długotrwałe nasłuchiwanie SSE
                    sse_listener_loop(
                        response,
                        event_tx.clone(),
                        client.clone(),
                        url.clone(),
                        session_endpoint.clone(),
                        pending.clone(),
                        next_id.clone(),
                        available_tools.clone(),
                        shutdown_rx,
                                            ).await;
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
            format!("📤 Sending tools/list (id={})", id)
        )).await;

            if let Err(e) = self.send_jsonrpc(req, Some(id)).await {
                let _ = self.event_tx.send(
                    McpClientEvent::Error(format!("list_tools send: {}", e))
                ).await;
            }
    }

    pub async fn call_tool(&self, tool_name: String, arguments: serde_json::Value) {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        });

        let _ = self.event_tx.send(McpClientEvent::Debug(
            format!("📤 Calling tool '{}' (id={})", tool_name, id)
        )).await;

        if let Err(e) = self.send_jsonrpc(req, Some(id)).await {
            let _ = self.event_tx.send(
                McpClientEvent::Error(format!("call_tool send: {}", e))
            ).await;
        }
    }

    pub async fn get_available_tools(&self) -> Vec<ToolInfo> {
        self.available_tools.lock().await.clone()
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

                if status.is_success() || status.as_u16() == 202 {
                    Ok(())
                } else {
                    if let Ok(body) = r.text().await {
                        let _ = self.event_tx.send(McpClientEvent::Debug(
                            format!("📄 Error body: {}", body)
                        )).await;
                    }
                    Err(format!("POST HTTP error: {}", status))
                }
            }
            Err(e) => Err(format!("POST error: {}", e)),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// SSE LISTENER LOOP
// ═══════════════════════════════════════════════════════════════════

async fn sse_listener_loop(
    response: reqwest::Response,
    event_tx: mpsc::Sender<McpClientEvent>,
    client: Client,
    base_url: String,
    session_endpoint: Arc<Mutex<Option<String>>>,
    pending: Arc<Mutex<HashMap<i64, oneshot::Sender<serde_json::Value>>>>,
    next_id: Arc<AtomicI64>,
    available_tools: Arc<Mutex<Vec<ToolInfo>>>,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    let mut stream = response.bytes_stream();
    let mut buf = String::new();
    let mut endpoint_received = false;
    let mut initialized = false;

    let _ = event_tx.send(McpClientEvent::Debug(
        "📥 SSE listener loop started".to_string()
    )).await;

    loop {
        tokio::select! {
            biased;

            _ = &mut shutdown_rx => {
                let _ = event_tx.send(McpClientEvent::Debug(
                    "🛑 SSE listener shutdown requested".to_string()
                )).await;
                let _ = event_tx.send(McpClientEvent::Disconnected).await;
                break;
            }

            item = stream.next() => {
                match item {
                    Some(Ok(chunk)) => {
                        let txt = String::from_utf8_lossy(&chunk).to_string();
                        buf.push_str(&txt);

                        // Przetwarzaj kompletne wiadomości SSE
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

                            // Obsługa endpointu
                            if event_type == "endpoint" && !endpoint_received {
                                {
                                    let mut lock = session_endpoint.lock().await;
                                    *lock = Some(data.clone());
                                }
                                endpoint_received = true;

                                let _ = event_tx.send(McpClientEvent::Debug(
                                    format!("✅ Endpoint stored: {}", data)
                                )).await;

                                // Wysłanie initialize
                                send_initialize(
                                    &client,
                                    &base_url,
                                    &data,
                                    &next_id,
                                    &event_tx,
                                ).await;
                                
                                continue;
                            }

                            // Parsowanie JSON-RPC
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data) {
                                // Sprawdź czy to odpowiedź na initialize
                                if !initialized {
                                    if let Some(id) = v.get("id").and_then(|i| i.as_i64()) {
                                        if id == 1 && v.get("result").is_some() {
                                            initialized = true;
                                            let _ = event_tx.send(McpClientEvent::Message(
                                                "✅ MCP session initialized".to_string()
                                            )).await;

                                            // Automatycznie pobierz listę narzędzi
                                            auto_load_tools(
                                                &client,
                                                &base_url,
                                                &session_endpoint,
                                                &next_id,
                                                &event_tx,
                                            ).await;
                                            continue;
                                        }
                                    }
                                }

                                handle_json_rpc_event(
                                    v,
                                    &event_tx,
                                    &pending,
                                    &available_tools,
                                ).await;
                            } else {
                                let _ = event_tx.send(
                                    McpClientEvent::Message(data.clone())
                                ).await;
                            }
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
                            "⚠️ SSE stream ended".to_string()
                        )).await;
                        let _ = event_tx.send(McpClientEvent::Disconnected).await;
                        break;
                    }
                }
            }
        }
    }

    let _ = event_tx.send(McpClientEvent::Debug(
        "🔚 SSE listener loop terminated".to_string()
    )).await;
}

// ═══════════════════════════════════════════════════════════════════
// AUTO-LOAD TOOLS
// ═══════════════════════════════════════════════════════════════════

async fn auto_load_tools(
    client: &Client,
    base_url: &str,
    session_endpoint: &Arc<Mutex<Option<String>>>,
    next_id: &Arc<AtomicI64>,
    event_tx: &mpsc::Sender<McpClientEvent>,
) {
    sleep(Duration::from_millis(100)).await;

    let ep = {
        let lock = session_endpoint.lock().await;
        lock.clone()
    };

    if let Some(endpoint) = ep {
        let id = next_id.fetch_add(1, Ordering::SeqCst);
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/list",
            "params": {}
        });

        let full_url = join_url(base_url, &endpoint);

        let _ = event_tx.send(McpClientEvent::Debug(
            "🔄 Auto loading tools...".to_string()
        )).await;

        let _ = client
            .post(&full_url)
            .header("Content-Type", "application/json")
            .body(req.to_string())
            .send()
            .await;
    }
}

// ═══════════════════════════════════════════════════════════════════
// INITIALIZE REQUEST
// ═══════════════════════════════════════════════════════════════════

async fn send_initialize(
    client: &Client,
    base_url: &str,
    endpoint: &str,
    next_id: &Arc<AtomicI64>,
    event_tx: &mpsc::Sender<McpClientEvent>,
) {
    sleep(Duration::from_millis(100)).await;

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

    let full_url = join_url(base_url, endpoint);

    let _ = event_tx.send(McpClientEvent::Debug(
        format!("📤 Sending initialize to: {}", full_url)
    )).await;

    let _ = client.post(&full_url)
        .header("Content-Type", "application/json")
        .body(init.to_string())
        .send()
        .await;
}


// ═══════════════════════════════════════════════════════════════════════════
// Helper function: Split long text into multiple lines
// ═══════════════════════════════════════════════════════════════════════════

/// Splits text into lines for proper scrolling
/// - If text contains \n, split on those
/// - If line is too long, split at reasonable boundaries
fn split_for_display(text: &str, max_line_length: usize) -> Vec<String> {
    let mut result = Vec::new();
    
    // First, split on actual newlines
    for line in text.lines() {
        if line.len() <= max_line_length {
            result.push(line.to_string());
        } else {
            // Line is too long, break it intelligently
            result.extend(break_long_line(line, max_line_length));
        }
    }
    
    // If text has no newlines at all
    if result.is_empty() && !text.is_empty() {
        result.extend(break_long_line(text, max_line_length));
    }
    
    result
}

/// Breaks a long line at reasonable boundaries
fn break_long_line(line: &str, max_length: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    
    for word in line.split_whitespace() {
        if current.len() + word.len() + 1 > max_length {
            if !current.is_empty() {
                lines.push(current.clone());
                current.clear();
            }
            
            // If single word is longer than max_length, split it forcefully
            if word.len() > max_length {
                let mut remaining = word;
                while remaining.len() > max_length {
                    lines.push(remaining[..max_length].to_string());
                    remaining = &remaining[max_length..];
                }
                if !remaining.is_empty() {
                    current = remaining.to_string();
                }
            } else {
                current = word.to_string();
            }
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
    }
    
    if !current.is_empty() {
        lines.push(current);
    }
    
    if lines.is_empty() {
        lines.push(String::new());
    }
    
    lines
}

// ═══════════════════════════════════════════════════════════════════
// JSON-RPC EVENT HANDLER
// ═══════════════════════════════════════════════════════════════

async fn handle_json_rpc_event(
    v: Value,
    event_tx: &mpsc::Sender<McpClientEvent>,
    pending: &Arc<Mutex<HashMap<i64, oneshot::Sender<Value>>>>,
    available_tools: &Arc<Mutex<Vec<ToolInfo>>>,
) {
    if let Some(id) = v.get("id").and_then(|v| v.as_i64()) {
        if let Some(result) = v.get("result") {
            // Tools list - NO CHANGE NEEDED
            if let Some(tools) = result.get("tools") {
                // ... existing code unchanged ...
                return;
            }

            // ═══════════════════════════════════════════════════════════════
            // CHANGE 1: tools/call result
            // ═══════════════════════════════════════════════════════════════
            if let Some(content) = result.get("content") {
                if let Some(content_array) = content.as_array() {
                    for item in content_array {
                        if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                            // Header
                            let _ = event_tx.send(McpClientEvent::Message(
                                "📋 Tool result:".to_string()
                            )).await;
                            
                            if let Ok(json_value) = serde_json::from_str::<Value>(text) {
                                let formatted = format_json_safely(&json_value).await;
                                
                                // CHANGED: Split by lines
                                let all_lines: Vec<&str> = formatted.lines().collect();
                                
                                if all_lines.len() > 200 {
                                    // Send first 200 lines
                                    for line in all_lines.iter().take(200) {
                                        let _ = event_tx.send(McpClientEvent::Message(
                                            line.to_string()
                                        )).await;
                                    }
                                    
                                    let _ = event_tx.send(McpClientEvent::Message(
                                        "".to_string()
                                    )).await;
                                    let _ = event_tx.send(McpClientEvent::Message(
                                        format!("⚠️  Response truncated: showing 200 of {} lines", all_lines.len())
                                    )).await;
                                } else {
                                    // Send all lines
                                    for line in all_lines {
                                        let _ = event_tx.send(McpClientEvent::Message(
                                            line.to_string()
                                        )).await;
                                    }
                                }
                            } else {
                                // Not JSON - also split by lines
                                let all_lines: Vec<&str> = text.lines().collect();
                                
                                if all_lines.len() > 200 {
                                    for line in all_lines.iter().take(200) {
                                        let _ = event_tx.send(McpClientEvent::Message(
                                            line.to_string()
                                        )).await;
                                    }
                                    let _ = event_tx.send(McpClientEvent::Message(
                                        format!("⚠️  Output truncated: {} of {} lines shown", 200, all_lines.len())
                                    )).await;
                                } else {
                                    for line in all_lines {
                                        let _ = event_tx.send(McpClientEvent::Message(
                                            line.to_string()
                                        )).await;
                                    }
                                }
                            }
                        }
                    }
                    return;
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // CHANGE 2: Generic result
            // ═══════════════════════════════════════════════════════════════
            let formatted = format_json_safely(result).await;
            
            // CHANGED: Split by lines
            let all_lines: Vec<&str> = formatted.lines().collect();
            
            if all_lines.len() > 200 {
                for line in all_lines.iter().take(200) {
                    let _ = event_tx.send(McpClientEvent::Message(
                        line.to_string()
                    )).await;
                }
                let _ = event_tx.send(McpClientEvent::Message(
                    "".to_string()
                )).await;
                let _ = event_tx.send(McpClientEvent::Message(
                    format!("⚠️  Response truncated: showing 200 of {} lines", all_lines.len())
                )).await;
            } else {
                for line in all_lines {
                    let _ = event_tx.send(McpClientEvent::Message(
                        line.to_string()
                    )).await;
                }
            }
            
        } else if let Some(error) = v.get("error") {
            // ═══════════════════════════════════════════════════════════════
            // CHANGE 3: Error response
            // ═══════════════════════════════════════════════════════════════
            let formatted = format_json_safely(error).await;
            
            let _ = event_tx.send(McpClientEvent::Error(
                "RPC error:".to_string()
            )).await;
            
            // CHANGED: Split by lines
            for line in formatted.lines() {
                let _ = event_tx.send(McpClientEvent::Message(
                    line.to_string()
                )).await;
            }
        }
        return;
    }

    // Notifications - NO CHANGE NEEDED
    if let Some(method) = v.get("method").and_then(|m| m.as_str()) {
        match method {
            "notifications/tools/list_changed" => {
                let _ = event_tx.send(McpClientEvent::Message(
                    "🔔 Tools list changed - use :mcp tools to refresh".to_string()
                )).await;
            }
            _ => {
                let _ = event_tx.send(McpClientEvent::Message(
                    format!("🔔 Notification: {}", method)
                )).await;
            }
        }
    }
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
        let scheme = &base[..scheme_end + 3];
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
}
