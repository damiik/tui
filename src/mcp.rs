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
    next_id: AtomicI64,
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
            next_id: AtomicI64::new(1),
            sse_shutdown: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn connect(&mut self, url: String, _server_name: String) {
        self.base_url = Some(url.clone());

        let event_tx = self.event_tx.clone();
        let client = self.client.clone();
        let session_endpoint = self.session_endpoint.clone();
        let pending = self.pending.clone();
        let sse_shutdown = self.sse_shutdown.clone();
        let next_id_arc = Arc::new(AtomicI64::new(self.next_id.load(Ordering::SeqCst)));

        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        *sse_shutdown.lock().await = Some(shutdown_tx);

        tokio::spawn(async move {
            let mut attempt = 0;
            loop {
                attempt += 1;

                match client.get(&url).send().await {
                    Ok(response) => {
                        if !response.status().is_success() {
                            let _ = event_tx.send(McpClientEvent::Error(
                                format!("HTTP connect: {}", response.status()),
                            )).await;
                            sleep(Duration::from_millis(200)).await;
                            continue;
                        }

                        let _ = event_tx.send(McpClientEvent::Connected).await;
                        let mut stream = response.bytes_stream();
                        let mut buf = String::new();
                        let mut endpoint: Option<String> = None;

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

                                                let mut data = String::new();
                                                for line in block.lines() {
                                                    if let Some(rest) = line.strip_prefix("data:") {
                                                        if !data.is_empty() {
                                                            data.push('\n');
                                                        }
                                                        data.push_str(rest.trim());
                                                    }
                                                }

                                                if data.is_empty() {
                                                    continue;
                                                }

                                                // Check for endpoint announcement
                                                if let Some(ep) = data
                                                    .strip_prefix("event: endpoint")
                                                    .or_else(|| data.strip_prefix("endpoint"))
                                                {
                                                    let ep = ep.trim().to_string();
                                                    if !ep.is_empty() {
                                                        endpoint = Some(ep);
                                                        break;
                                                    }
                                                }

                                                // Try parsing as JSON-RPC
                                                match serde_json::from_str::<serde_json::Value>(&data) {
                                                    Ok(v) => {
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

                                        None => break,
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
                                sleep(Duration::from_millis(500)).await;
                                continue;
                            }
                        };

                        // Store endpoint for future requests
                        {
                            let mut lock = session_endpoint.lock().await;
                            *lock = Some(endpoint.clone());
                        }

                        let full_url = join_url(&url, &endpoint);

                        // Spawn SSE listener task
                        let sse_client = client.clone();
                        let sse_event_tx = event_tx.clone();
                        let sse_session = session_endpoint.clone();
                        let sse_pending = pending.clone();
                        let (sse_shutdown_tx, sse_shutdown_rx) = oneshot::channel();

                        // Store new shutdown handle
                        *sse_shutdown.lock().await = Some(sse_shutdown_tx);

                        tokio::spawn(async move {
                            let _ = sse_event_tx.send(McpClientEvent::Message(
                                format!("SSE started: {}", full_url)
                            )).await;

                            sse_stream_loop(
                                sse_client,
                                full_url,
                                sse_event_tx.clone(),
                                sse_session,
                                sse_pending,
                                sse_shutdown_rx,
                            ).await;
                        });

                        // Send initialize request
                        let id = next_id_arc.fetch_add(1, Ordering::SeqCst);
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

                        let _ = client.post(join_url(&url, &endpoint))
                            .header("Content-Type", "application/json")
                            .body(init.to_string())
                            .send()
                            .await;

                        break;
                    }
                    Err(e) => {
                        let _ = event_tx.send(McpClientEvent::Error(
                            format!("Connect error (attempt {}): {}", attempt, e)
                        )).await;
                        sleep(Duration::from_millis(500)).await;
                        continue;
                    }
                }
            }
        });
    }

    pub async fn list_tools(&self) {
        if let Some(url) = &self.base_url {
            let id = self.next_id.fetch_add(1, Ordering::SeqCst);

            let req = json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/list",
                "params": {}
            });

            if let Err(e) = self.send_jsonrpc(req, Some(id)).await {
                let _ = self.event_tx.send(
                    McpClientEvent::Error(format!("list_tools send: {}", e))
                ).await;
            }
        } else {
            let _ = self.event_tx.send(
                McpClientEvent::Error("Not connected to an MCP server.".to_string())
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
            None => return Err("Not connected to an MCP server.".into()),
        };

        let ep = {
            let lock = self.session_endpoint.lock().await;
            lock.clone()
        };

        let url = match ep {
            Some(ep) => join_url(&base, &ep),
            None => base,
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
            Ok(r) if r.status().is_success() => Ok(()),
            Ok(r) => Err(format!("POST HTTP error: {}", r.status())),
            Err(e) => Err(format!("POST error: {}", e)),
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// SSE LOOP
// ═══════════════════════════════════════════════════════════════

async fn sse_stream_loop(
    client: Client,
    url: String,
    event_tx: mpsc::Sender<McpClientEvent>,
    _session: Arc<Mutex<Option<String>>>,
    pending: Arc<Mutex<HashMap<i64, oneshot::Sender<serde_json::Value>>>>,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    let mut buf = String::new();
    
    loop {
        match client.get(&url).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    let _ = event_tx.send(McpClientEvent::Error(
                        format!("SSE HTTP {}", resp.status())
                    )).await;
                    sleep(Duration::from_millis(500)).await;
                    continue;
                }

                let mut stream = resp.bytes_stream();

                loop {
                    tokio::select! {
                        _ = &mut shutdown_rx => {
                            let _ = event_tx.send(McpClientEvent::Disconnected).await;
                            return;
                        }

                        chunk = stream.next() => {
                            match chunk {
                                Some(Ok(bytes)) => {
                                    let txt = String::from_utf8_lossy(&bytes).to_string();
                                    buf.push_str(&txt);

                                    while let Some(split) = buf.find("\n\n") {
                                        let block = buf[..split].to_string();
                                        buf = buf[split + 2..].to_string();

                                        let mut data = String::new();
                                        for line in block.lines() {
                                            if let Some(r) = line.strip_prefix("data:") {
                                                if !data.is_empty() {
                                                    data.push('\n');
                                                }
                                                data.push_str(r.trim());
                                            }
                                        }

                                        if data.is_empty() {
                                            continue;
                                        }

                                        match serde_json::from_str::<serde_json::Value>(&data) {
                                            Ok(v) => {
                                                handle_json_rpc_event(v, &event_tx, &pending).await;
                                            }
                                            Err(_) => {
                                                let _ = event_tx.send(
                                                    McpClientEvent::Message(data.clone())
                                                ).await;
                                            }
                                        }
                                    }
                                }

                                Some(Err(e)) => {
                                    let _ = event_tx.send(McpClientEvent::Error(
                                        format!("SSE chunk error: {}", e)
                                    )).await;
                                    break;
                                }

                                None => break,
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let _ = event_tx.send(McpClientEvent::Error(
                    format!("SSE connect error: {}", e)
                )).await;
                sleep(Duration::from_millis(500)).await;
                continue;
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// JSON-RPC EVENT HANDLER
// ═══════════════════════════════════════════════════════════════

/// Handle JSON-RPC object received via SSE
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

/// Join base URL and endpoint path
fn join_url(base: &str, endpoint: &str) -> String {
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        return endpoint.into();
    }

    let mut b = base.to_string();
    if b.ends_with('/') && endpoint.starts_with('/') {
        b.pop();
    }
    if !b.ends_with('/') && !endpoint.starts_with('/') {
        b.push('/');
    }
    b + endpoint
}
