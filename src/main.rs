use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use mcp_client::{app::App, config::Config, event::EventLoop, ui::UI};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Load config
    let config = Config::from_file("config.json")?;

    // Run app
    let app = App::new(config);
    let mut event_loop = EventLoop::new();
    let ui = UI::new();

    let res = run_loop(&mut terminal, app, &mut event_loop, &ui).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    res
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut app: App,
    event_loop: &mut EventLoop,
    ui: &UI,
) -> Result<()> {
    let mut last_mouse_state = app.mouse_enabled();
    let mut last_size = ratatui::layout::Size::default();

    loop {
        // Update app state with current UI geometry
        let size = terminal.size()?;
        if size != last_size {
            // CRITICAL: We need to pass the ACTUAL widget area height
            // The layout gives output: Constraint::Min(3)
            // This means: total - status(1) - input(1)
            
            // But we need to account for what the LAYOUT will actually give us
            // Let's recalculate using the same logic as create_layout
            
            let chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Min(3),      // Output
                    ratatui::layout::Constraint::Length(1),   // Status
                    ratatui::layout::Constraint::Length(1),   // Input
                ])
                .split(ratatui::layout::Rect::new(0, 0, size.width, size.height));
            
            // chunks[0] is the actual output area
            let output_area_height = chunks[0].height;
            
            app.set_output_height(output_area_height);
            last_size = size;
        }

        terminal.draw(|frame| ui.render(frame, &app))?;

        // Handle mouse capture state changes
        let current_mouse_state = app.mouse_enabled();
        if current_mouse_state != last_mouse_state {
            if current_mouse_state {
                execute!(terminal.backend_mut(), EnableMouseCapture)?;
            } else {
                execute!(terminal.backend_mut(), DisableMouseCapture)?;
            }
            last_mouse_state = current_mouse_state;
        }

        if let Some(event) = event_loop.next()? {
            app = app.handle_event(event).await?;

            if app.should_quit() {
                break;
            }
        }
    }

    Ok(())
}
