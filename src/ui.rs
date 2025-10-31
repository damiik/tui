use crate::app::App;
use crate::mode::Mode;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};

/// Pure UI rendering logic - no side effects
pub struct UI {
    scroll_offset: usize,
}

impl UI {
    pub const fn new() -> Self {
        Self { scroll_offset: 0 }
    }

    /// Pure function: Frame × App → ()
    /// Renders application state to terminal frame
    pub fn render(&self, frame: &mut Frame, app: &App) {
        let layout = Self::create_layout(frame.area());

        self.render_output(frame, app, layout.output);
        self.render_status_bar(frame, app, layout.status);
        self.render_input_line(frame, app, layout.input);
    }

    // ═══════════════════════════════════════════════════════════════
    // Layout composition
    // ═══════════════════════════════════════════════════════════════

    fn create_layout(area: Rect) -> LayoutAreas {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),      // Output area
                Constraint::Length(1),   // Status bar
                Constraint::Length(1),   // Input line
            ])
            .split(area);

        LayoutAreas {
            output: chunks[0],
            status: chunks[1],
            input: chunks[2],
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // Output area rendering
    // ═══════════════════════════════════════════════════════════════

    fn render_output(&self, frame: &mut Frame, app: &App, area: Rect) {
        let lines: Vec<Line> = app
            .output()
            .iter()
            .map(|s| Line::from(s.as_str()))
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(
                " Output ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset as u16, 0));

        frame.render_widget(paragraph, area);

        // Render scrollbar if content exceeds visible area
        if app.output().len() > area.height as usize - 2 {
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));

            let mut scrollbar_state = ScrollbarState::new(app.output().len())
                .position(self.scroll_offset);

            frame.render_stateful_widget(
                scrollbar,
                area.inner(ratatui::layout::Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                &mut scrollbar_state,
            );
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // Status bar rendering - shows mode and status message
    // ═══════════════════════════════════════════════════════════════

    fn render_status_bar(&self, frame: &mut Frame, app: &App, area: Rect) {
        let mode = app.mode();

        let mode_indicator = Span::styled(
            format!(" {} ", mode.name()),
            Style::default()
                .fg(Color::Black)
                .bg(mode.color())
                .add_modifier(Modifier::BOLD),
        );

        let status_text = Span::styled(
            format!(" {} ", app.status()),
            Style::default().fg(Color::White),
        );

        let help_text = Span::styled(
            format!(" {} ", mode.help_text()),
            Style::default().fg(Color::DarkGray),
        );

        let line = Line::from(vec![mode_indicator, status_text, help_text]);

        let paragraph = Paragraph::new(line)
            .style(Style::default().bg(Color::Black));

        frame.render_widget(paragraph, area);
    }

    // ═══════════════════════════════════════════════════════════════
    // Input line rendering - shows current buffer based on mode
    // ═══════════════════════════════════════════════════════════════

    fn render_input_line(&self, frame: &mut Frame, app: &App, area: Rect) {
        let (prefix, content, cursor_offset) = match app.mode() {
            Mode::Normal => ("", "", 0),
            Mode::Insert => ("> ", app.input_buffer(), 2),
            Mode::Command => (":", app.command_buffer(), 1),
        };

        let prefix_span = Span::styled(
            prefix,
            Style::default()
                .fg(app.mode().color())
                .add_modifier(Modifier::BOLD),
        );

        let content_span = Span::raw(content);

        let line = Line::from(vec![prefix_span, content_span]);

        let paragraph = Paragraph::new(line)
            .style(Style::default().bg(Color::Black).fg(Color::White));

        frame.render_widget(paragraph, area);

        // Set cursor position if in input mode
        if app.mode().shows_cursor() {
            let cursor_x = area.x + cursor_offset + app.cursor_pos() as u16;
            let cursor_y = area.y;

            if cursor_x < area.x + area.width {
                frame.set_cursor_position((cursor_x, cursor_y));
            }
        }
    }
}

impl Default for UI {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════
// Layout helper structure
// ═══════════════════════════════════════════════════════════════

struct LayoutAreas {
    output: Rect,
    status: Rect,
    input: Rect,
}
