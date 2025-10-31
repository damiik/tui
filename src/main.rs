use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use mcp_client::{app::App, event::EventLoop, ui::UI};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let mut app = App::new();
    let mut event_loop = EventLoop::new();
    let ui = UI::new();

    let res = run_loop(&mut terminal, &mut app, &mut event_loop, &ui);

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

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    event_loop: &mut EventLoop,
    ui: &UI,
) -> Result<()> {
    loop {
        terminal.draw(|frame| ui.render(frame, app))?;

        if let Some(event) = event_loop.next()? {
            *app = app.clone().handle_event(event)?;

            if app.should_quit() {
                break;
            }
        }
    }

    Ok(())
}
