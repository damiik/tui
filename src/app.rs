use crate::command::Command;
use crate::config::Config;
use crate::event::Event;
use crate::mcp::{McpClient, McpClientEvent};
use crate::mode::Mode;
use crate::state::{Buffer, OutputLog};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyModifiers};
use tokio::sync::mpsc;

/// Immutable application state with functional transitions
#[derive(Debug)]
pub struct App {
    mode: Mode,
    output: OutputLog,
    input_buffer: Buffer,
    command_buffer: Buffer,
    status: String,
    quit: bool,
    mcp_client: McpClient,
    pub mcp_event_rx: mpsc::Receiver<McpClientEvent>,
    config: Config,
}

impl App {
    pub fn new(config: Config) -> Self {
        let (mcp_event_tx, mcp_event_rx) = mpsc::channel(100);
        let mcp_client = McpClient::new(mcp_event_tx);

        Self {
            mode: Mode::Normal,
            output: OutputLog::new()
                .with_message("MCP Client initialized. Press 'i' for INSERT mode.".to_string()),
            input_buffer: Buffer::new(),
            command_buffer: Buffer::new(),
            status: "Ready".into(),
            quit: false,
            mcp_client,
            mcp_event_rx,
            config,
        }
    }

    pub const fn mode(&self) -> Mode {
        self.mode
    }

    pub fn output(&self) -> &[String] {
        self.output.lines()
    }

    pub fn input_buffer(&self) -> &str {
        self.input_buffer.content()
    }

    pub fn command_buffer(&self) -> &str {
        self.command_buffer.content()
    }

    pub fn cursor_pos(&self) -> usize {
        match self.mode {
            Mode::Insert => self.input_buffer.cursor(),
            Mode::Command => self.command_buffer.cursor(),
            Mode::Normal => 0,
        }
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub const fn should_quit(&self) -> bool {
        self.quit
    }

    pub async fn handle_event(mut self, event: Event) -> Result<Self> {
        match event {
            Event::Key(key) => self.handle_key(key.code, key.modifiers).await,
            Event::Tick => {
                if let Ok(mcp_event) = self.mcp_event_rx.try_recv() {
                    self.handle_mcp_event(mcp_event).await
                } else {
                    Ok(self)
                }
            }
        }
    }

    async fn handle_mcp_event(mut self, event: McpClientEvent) -> Result<Self> {
        match event {
            McpClientEvent::Connected => {
                self.status = "MCP client connected".into();
            }
            McpClientEvent::Disconnected => {
                self.status = "MCP client disconnected".into();
            }
            McpClientEvent::Message(msg) => {
                self.output = self.output.with_message(format!("[MCP] {}", msg));
            }
            McpClientEvent::Error(err) => {
                self.output = self.output.with_message(format!("[MCP Error] {}", err));
            }
            McpClientEvent::ToolsListed(tools) => {
                self.output = self.output.with_message("Available tools:".to_string());
                for tool in tools {
                    self.output = self.output.with_message(format!("  - {}", tool));
                }
            }
        }
        Ok(self)
    }

    async fn handle_key(self, code: KeyCode, mods: KeyModifiers) -> Result<Self> {
        if mods.contains(KeyModifiers::CONTROL) {
            return self.handle_ctrl_key(code).await;
        }

        match self.mode {
            Mode::Normal => self.handle_normal_key(code).await,
            Mode::Insert => self.handle_insert_key(code).await,
            Mode::Command => self.handle_command_key(code).await,
        }
    }

    async fn handle_normal_key(mut self, code: KeyCode) -> Result<Self> {
        match code {
            KeyCode::Char('i') => {
                self.mode = Mode::Insert;
                self.status = "Entered INSERT mode".into();
            }
            KeyCode::Char(':') => {
                self.mode = Mode::Command;
                self.command_buffer = Buffer::new();
                self.status = "Entered COMMAND mode".into();
            }
            KeyCode::Char('q') => {
                self.quit = true;
            }
            _ => {}
        }
        Ok(self)
    }

    async fn handle_insert_key(mut self, code: KeyCode) -> Result<Self> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.status = "Exited to NORMAL mode".into();
            }
            KeyCode::Enter => {
                let input = self.input_buffer.content().to_string();
                if !input.is_empty() {
                    self.output = self
                        .output
                        .with_message(format!("→ {}", input))
                        .with_message(format!("← Echo: {}", input));
                    self.input_buffer = Buffer::new();
                    self.status = format!("Sent: {}", input);
                }
            }
            KeyCode::Char(c) => {
                self.input_buffer = self.input_buffer.insert_char(c);
            }
            KeyCode::Backspace => {
                self.input_buffer = self.input_buffer.delete_char();
            }
            KeyCode::Left => {
                self.input_buffer = self.input_buffer.move_left();
            }
            KeyCode::Right => {
                self.input_buffer = self.input_buffer.move_right();
            }
            KeyCode::Home => {
                self.input_buffer = self.input_buffer.move_start();
            }
            KeyCode::End => {
                self.input_buffer = self.input_buffer.move_end();
            }
            _ => {}
        }
        Ok(self)
    }

    async fn handle_command_key(mut self, code: KeyCode) -> Result<Self> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.command_buffer = Buffer::new();
                self.status = "Command cancelled".into();
                Ok(self)
            }
            KeyCode::Enter => {
                let cmd_text = self.command_buffer.content().to_string();
                let mut app = self.execute_command(&cmd_text).await?;
                app.mode = Mode::Normal;
                app.command_buffer = Buffer::new();
                Ok(app)
            }
            KeyCode::Char(c) => {
                self.command_buffer = self.command_buffer.insert_char(c);
                Ok(self)
            }
            KeyCode::Backspace => {
                self.command_buffer = self.command_buffer.delete_char();
                Ok(self)
            }
            _ => Ok(self),
        }
    }

    async fn handle_ctrl_key(mut self, code: KeyCode) -> Result<Self> {
        match code {
            KeyCode::Char('q') => {
                self.quit = true;
            }
            KeyCode::Char('w') if self.mode == Mode::Insert => {
                self.input_buffer = Buffer::new();
                self.status = "Input cleared".into();
            }
            KeyCode::Char('l') => {
                self.output = OutputLog::new();
                self.status = "Output cleared".into();
            }
            _ => {}
        }
        Ok(self)
    }

    async fn execute_command(mut self, text: &str) -> Result<Self> {
        match Command::parse(text) {
            Ok(Command::Quit) => {
                self.quit = true;
                self.status = "Quitting...".into();
            }
            Ok(Command::Clear) => {
                self.output = OutputLog::new();
                self.status = "Output cleared".into();
            }
            Ok(Command::Echo(msg)) => {
                self.output = self.output.with_message(msg.clone());
                self.status = format!("Echoed: {}", msg);
            }
            Ok(Command::Help) => {
                self.output = self.output
                    .with_message("Available commands:".to_string())
                    .with_message("  :q, :quit                - Exit application".to_string())
                    .with_message("  :clear                   - Clear output".to_string())
                    .with_message("  :echo <text>             - Echo text to output".to_string())
                    .with_message("  :mcp list                - List configured MCP servers".to_string())
                    .with_message("  :mcp tools               - List tools of connected MCP server".to_string())
                    .with_message("  :mcp cn, :mcp connect    - Connect to MCP server".to_string())
                    .with_message("  :h, :help                - Show this help".to_string());
                self.status = "Help displayed".into();
            }
            Ok(Command::McpConnect(server_name)) => {
                if let Some(name) = server_name {
                    if let Some(server) = self.config.mcp_servers.iter().find(|s| s.name == name) {
                        self.status = format!("Connecting to {}...", server.url);
                        self.mcp_client.connect(server.url.clone(), server.name.clone()).await;
                    } else {
                        self.status = format!("Server '{}' not found in config.json", name);
                    }
                } else {
                    self.output = self.output.with_message("Available MCP servers:".to_string());
                    for (i, server) in self.config.mcp_servers.iter().enumerate() {
                        self.output = self.output.with_message(format!("  [{}] {}: {}", i + 1, server.name, server.url));
                    }
                    self.output = self.output.with_message("Usage: :mcp connect [server_name|server_number]".to_string());
                }
            }
            Ok(Command::McpList) => {
                self.output = self.output.with_message("Configured MCP servers:".to_string());
                for server in &self.config.mcp_servers {
                    self.output = self
                        .output
                        .with_message(format!("  - {}: {}", server.name, server.url));
                }
            }
            Ok(Command::McpTools) => {
                self.status = "Listing tools...".to_string();
                self.mcp_client.list_tools().await;
            }
            Err(e) => {
                self.status = format!("Error: {}", e);
            }
        }
        Ok(self)
    }
}

impl Default for App {
    fn default() -> Self {
        let config = Config::from_file("config.json").unwrap();
        Self::new(config)
    }
}
