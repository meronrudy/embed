//! TUI entrypoint: renders a 2D spike raster (time on X, neuron IDs on Y)
//! Controls: [s] Step, [r] Run/Pause, [q] Quit

mod backend;
mod app;
mod ui;

use anyhow::Result;
use backend::CoreBackend;
use app::App;
use ui::draw;

use std::io;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event as CEvent, KeyCode},
    execute, terminal,
};
use ratatui::{backend::CrosstermBackend, Terminal};

fn restore_terminal() -> Result<()> {
    terminal::disable_raw_mode()?;
    // Leave alternate screen and show cursor
    execute!(io::stdout(), terminal::LeaveAlternateScreen)?;
    Ok(())
}

fn main() -> Result<()> {
    // Setup terminal
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    // Optional: clear screen on start
    terminal.clear()?;

    // Ensure terminal is restored on panic
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_terminal();
        default_hook(panic_info);
    }));

    // App state
    let backend = CoreBackend::new();
    let mut app = App::new(backend, 80); // raster width (columns)
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    // Event loop
    loop {
        draw(&mut terminal, &app)?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::from_millis(0));

        if event::poll(timeout)? {
            if let CEvent::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('s') => app.step(),
                    KeyCode::Char('r') => app.toggle_running(),
                    _ => {}
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            if app.running {
                app.step();
            }
            last_tick = Instant::now();
        }
    }

    // Cleanup
    restore_terminal()?;
    Ok(())
}