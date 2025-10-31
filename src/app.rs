use crate::command::Command;
use crate::event::Event;
use crate::mode::Mode;
use crate::state::{Buffer, OutputLog};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyModifiers};

/// Immutable application state with functional transitions
#[derive(Debug, Clone)]
pub struct App {
    mode: Mode,
    output: OutputLog,
    input_buffer: Buffer,
    command_buffer: Buffer,
    status: String,
    quit: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            mode: Mode::Normal,
            output: OutputLog::new().with_message("MCP Client initialized. Press 'i' for INSERT mode.".to_string()),
            input_buffer: Buffer::new(),
            command_buffer: Buffer::new(),
            status: "Ready".into(),
            quit: false,
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // Pure accessors (no side effects)
    // ═══════════════════════════════════════════════════════════════

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

    // ═══════════════════════════════════════════════════════════════
    // Event handler: Self → Event → Result<Self>
    // Core functional transformation
    // ═══════════════════════════════════════════════════════════════

    pub fn handle_event(self, event: Event) -> Result<Self> {
        match event {
            Event::Key(key) => self.handle_key(key.code, key.modifiers),
            Event::Tick => Ok(self),
        }
    }

    fn handle_key(self, code: KeyCode, mods: KeyModifiers) -> Result<Self> {
        // Global keybindings
        if mods.contains(KeyModifiers::CONTROL) {
            return self.handle_ctrl_key(code);
        }

        // Mode-specific keybindings
        match self.mode {
            Mode::Normal => self.handle_normal_key(code),
            Mode::Insert => self.handle_insert_key(code),
            Mode::Command => self.handle_command_key(code),
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // Mode: NORMAL
    // ═══════════════════════════════════════════════════════════════

    fn handle_normal_key(mut self, code: KeyCode) -> Result<Self> {
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

    // ═══════════════════════════════════════════════════════════════
    // Mode: INSERT
    // ═══════════════════════════════════════════════════════════════

    fn handle_insert_key(mut self, code: KeyCode) -> Result<Self> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.status = "Exited to NORMAL mode".into();
            }
            KeyCode::Enter => {
                let input = self.input_buffer.content().to_string();
                if !input.is_empty() {
                    self.output = self.output
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

    // ═══════════════════════════════════════════════════════════════
    // Mode: COMMAND
    // ═══════════════════════════════════════════════════════════════

    fn handle_command_key(mut self, code: KeyCode) -> Result<Self> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.command_buffer = Buffer::new();
                self.status = "Command cancelled".into();
            }
            KeyCode::Enter => {
                let cmd_text = self.command_buffer.content().to_string();
                self = self.execute_command(&cmd_text)?;
                self.mode = Mode::Normal;
                self.command_buffer = Buffer::new();
            }
            KeyCode::Char(c) => {
                self.command_buffer = self.command_buffer.insert_char(c);
            }
            KeyCode::Backspace => {
                self.command_buffer = self.command_buffer.delete_char();
            }
            _ => {}
        }
        Ok(self)
    }

    // ═══════════════════════════════════════════════════════════════
    // Control key handlers
    // ═══════════════════════════════════════════════════════════════

    fn handle_ctrl_key(mut self, code: KeyCode) -> Result<Self> {
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

    // ═══════════════════════════════════════════════════════════════
    // Command execution
    // ═══════════════════════════════════════════════════════════════

    fn execute_command(mut self, text: &str) -> Result<Self> {
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
                    .with_message("  :q, :quit     - Exit application".to_string())
                    .with_message("  :clear        - Clear output".to_string())
                    .with_message("  :echo <text>  - Echo text to output".to_string())
                    .with_message("  :help         - Show this help".to_string());
                self.status = "Help displayed".into();
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
        Self::new()
    }
}

