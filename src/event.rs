use anyhow::Result;
use crossterm::event::{self, KeyEvent};
use std::time::Duration;

/// Event stream abstraction
#[derive(Debug, Clone, Copy)]
pub enum Event {
    Key(KeyEvent),
    Tick,
}

/// Event loop with configurable tick rate
pub struct EventLoop {
    tick_rate: Duration,
}

impl EventLoop {
    pub fn new() -> Self {
        Self {
            tick_rate: Duration::from_millis(100),
        }
    }

    pub fn with_tick_rate(mut self, rate: Duration) -> Self {
        self.tick_rate = rate;
        self
    }

    /// Pure function: Self → Result<Option<Event>>
    /// Polls for events with timeout
    pub fn next(&mut self) -> Result<Option<Event>> {
        if event::poll(self.tick_rate)? {
            match event::read()? {
                event::Event::Key(key) => Ok(Some(Event::Key(key))),
                event::Event::Resize(_, _) => Ok(Some(Event::Tick)),
                _ => Ok(None),
            }
        } else {
            Ok(Some(Event::Tick))
        }
    }
}

impl Default for EventLoop {
    fn default() -> Self {
        Self::new()
    }
}
