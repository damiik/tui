use crate::app::App;
use crate::mode::Mode;
use crate::completion::CompletionResult;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};

/// Pure UI rendering logic - no side effects
#[derive(Default)]
pub struct UI;

impl UI {
    pub const fn new() -> Self {
        Self
    }

    /// Pure function: Frame × App → ()
    // pub fn render(&self, frame: &mut Frame, app: &App) {
    //     let layout = Self::create_layout(frame.area());

    //     self.render_output(frame, app, layout.output);
    //     self.render_status_bar(frame, app, layout.status);
    //     self.render_input_line(frame, app, layout.input);
    // }

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

    // CRITICAL: Verify scroll offset calculation
    let scroll_offset = app.scroll_offset();
    
    // DEBUG: Add temporary logging (remove in production)
    // eprintln!("DEBUG render_output:");
    // eprintln!("  area.height = {}", area.height);
    // eprintln!("  lines.len() = {}", lines.len());
    // eprintln!("  scroll_offset = {}", scroll_offset);
    // eprintln!("  view_height = {}", area.height.saturating_sub(2));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((scroll_offset, 0));

    frame.render_widget(paragraph, area);

    // Scrollbar rendering (existing code)
    let content_length = app.output().len();
    if content_length > area.height as usize - 2 {
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state = ScrollbarState::new(content_length)
            .position(scroll_offset as usize);

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

        // Determine mode indicator based on selection state
        let (mode_text, mode_color) = if app.tool_selection().is_some() {
            ("TOOL", Color::Yellow)
        } else if app.server_selection().is_some() {
            ("SELECT", Color::Magenta)
        } else {
            (mode.name(), mode.color())
        };

        let mode_indicator = Span::styled(
            format!(" {} ", mode_text),
            Style::default()
                .fg(Color::Black)
                .bg(mode_color)
                .add_modifier(Modifier::BOLD),
        );

        let status_text = Span::styled(
            format!(" {} ", app.status()),
            Style::default().fg(Color::White),
        );

        let help_text = if app.tool_selection().is_some() {
            Span::styled(
                " ↑↓:Navigate | Enter:Run | Esc:Cancel ",
                Style::default().fg(Color::DarkGray),
            )
        } else if app.server_selection().is_some() {
            Span::styled(
                " ↑↓:Navigate | Enter:Select | Esc:Cancel ",
                Style::default().fg(Color::DarkGray),
            )
        } else {
            Span::styled(
            format!(" {} ", mode.help_text()),
            Style::default().fg(Color::DarkGray),
            )
        };

        let line = Line::from(vec![mode_indicator, status_text, help_text]);

        let paragraph = Paragraph::new(line)
            .style(Style::default().bg(Color::Black));

        frame.render_widget(paragraph, area);
    }

    // ═══════════════════════════════════════════════════════════════
    // Input line rendering - shows current buffer based on mode
    // ═══════════════════════════════════════════════════════════════

    fn render_input_line(&self, frame: &mut Frame, app: &App, area: Rect) {
        // If in selection mode, hide input
        if app.server_selection().is_some() || app.tool_selection().is_some() {
            let paragraph = Paragraph::new("")
                .style(Style::default().bg(Color::Black));
            frame.render_widget(paragraph, area);
            return;
        }

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

    /// Render completion popup above input line (Vim-style)
    fn render_completion_popup(
        &self,
        frame: &mut Frame,
        completion: &CompletionResult,
        input_area: Rect,
    ) {
        if completion.is_empty() {
            return;
        }

        // Calculate popup dimensions
        let max_width = completion.candidates
            .iter()
            .map(|c| {
                let desc_len = c.description.as_ref().map_or(0, |d| d.len());
                c.text.len() + desc_len + 4 // padding
            })
            .max()
            .unwrap_or(20)
            .min(120) as u16;

        let height = (completion.len().min(30) as u16) + 2; // max 10 items + borders

        // Position above input line
        let popup_area = Rect {
            x: input_area.x.saturating_add(1), // Offset by ":" character
            y: input_area.y.saturating_sub(height),
            width: max_width,
            height,
        };

        // Create list items
        let items: Vec<ListItem> = completion.candidates
            .iter()
            .enumerate()
            .map(|(i, candidate)| {
                let is_selected = i == completion.selected;
                
                let text = if let Some(desc) = &candidate.description {
                    format!("  {:<20} {}", candidate.text, desc)
                } else {
                    format!("  {}", candidate.text)
                };

                let style = if is_selected {
                    Style::default()
                        .bg(Color::Blue)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::White)
                };

                let prefix = if is_selected { "▶" } else { " " };
                let line = Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(text, style),
                ]);

                ListItem::new(line)
            })
            .collect();

        // Create list widget
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow))
                    .title(Span::styled(
                        " Completions ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ))
            )
            .style(Style::default().bg(Color::Black));

        // Render with higher z-index (last)
        frame.render_widget(list, popup_area);
    }    

    // Update the main render method to include completion popup
    pub fn render(&self, frame: &mut Frame, app: &App) {
        let layout = Self::create_layout(frame.area());

        self.render_output(frame, app, layout.output);
        self.render_status_bar(frame, app, layout.status);
        self.render_input_line(frame, app, layout.input);

        // NEW: Render completion popup if active
        if let Some(completion) = app.completion_popup() {
            if app.mode() == Mode::Command {
                self.render_completion_popup(frame, completion, layout.input);
            }
        }
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