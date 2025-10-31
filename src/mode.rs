use ratatui::style::Color;

/// Modal states inspired by Vim's philosophy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Command,
}

impl Mode {
    /// Pure function: Mode → &str
    pub const fn name(&self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Command => "COMMAND",
        }
    }

    /// Pure function: Mode → Color
    /// Signal: Each mode has distinct visual identity
    pub const fn color(&self) -> Color {
        match self {
            Mode::Normal => Color::Cyan,
            Mode::Insert => Color::Green,
            Mode::Command => Color::Yellow,
        }
    }

    /// Pure function: Mode → bool
    /// Determines if cursor should be visible
    pub const fn shows_cursor(&self) -> bool {
        matches!(self, Mode::Insert | Mode::Command)
    }

    /// Pure function: Mode → &str
    /// Help text for current mode
    pub const fn help_text(&self) -> &'static str {
        match self {
            Mode::Normal => "i:Insert | ::Command | ^Q:Quit",
            Mode::Insert => "ESC:Normal | ↵:Send | ^W:Clear",
            Mode::Command => "ESC:Cancel | ↵:Execute",
        }
    }
}
