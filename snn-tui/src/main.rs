//! TUI entrypoint: renders a 2D spike raster (time on X, neuron IDs on Y)
//! Controls: [s] Step, [r] Run/Pause, [q] Quit

mod backend;
mod app;
mod ui;

use anyhow::Result;
use backend::CoreBackend;
use app::App;
use ui::draw;
use snn_core_plus::StepBudgets;

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
    let mut backend = CoreBackend::new();

    // Optionally enable plasticity at startup when feature is compiled and env var is set
    #[cfg(feature = "plasticity")]
    {
        if std::env::var("SNN_TUI_PLASTICITY").ok().as_deref() == Some("1") {
            backend.enable_default_plasticity();
        }
    }

    let mut app = App::new(backend, 80); // raster width (columns)
    // Initialize budgets from env if provided, else None (unbounded)
    let mut budgets: Option<StepBudgets> = None;
    if let Ok(v) = std::env::var("SNN_TUI_BUDGET_EDGES") {
        if let Ok(edges) = v.parse::<usize>() {
            budgets.get_or_insert(StepBudgets { max_edge_visits: None, max_spikes_scheduled: None }).max_edge_visits = Some(edges);
        }
    }
    if let Ok(v) = std::env::var("SNN_TUI_BUDGET_SPIKES") {
        if let Ok(spikes) = v.parse::<usize>() {
            budgets.get_or_insert(StepBudgets { max_edge_visits: None, max_spikes_scheduled: None }).max_spikes_scheduled = Some(spikes);
        }
    }
    app.set_budgets(budgets);
    #[cfg(feature = "plasticity")]
    { app.plast_on = std::env::var("SNN_TUI_PLASTICITY").ok().as_deref() == Some("1"); }
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
                    // budgets: +/- adjust max_edge_visits, [/] adjust max_spikes
                    KeyCode::Char('+') => {
                        let mut b = app.budgets.unwrap_or(StepBudgets { max_edge_visits: Some(0), max_spikes_scheduled: None });
                        let cur = b.max_edge_visits.unwrap_or(0);
                        b.max_edge_visits = Some(cur.saturating_add(10));
                        app.set_budgets(Some(b));
                    }
                    KeyCode::Char('-') => {
                        let mut b = app.budgets.unwrap_or(StepBudgets { max_edge_visits: Some(0), max_spikes_scheduled: None });
                        let cur = b.max_edge_visits.unwrap_or(0);
                        b.max_edge_visits = Some(cur.saturating_sub(10));
                        app.set_budgets(Some(b));
                    }
                    KeyCode::Char('[') => {
                        let mut b = app.budgets.unwrap_or(StepBudgets { max_edge_visits: None, max_spikes_scheduled: Some(0) });
                        let cur = b.max_spikes_scheduled.unwrap_or(0);
                        b.max_spikes_scheduled = Some(cur.saturating_add(10));
                        app.set_budgets(Some(b));
                    }
                    KeyCode::Char(']') => {
                        let mut b = app.budgets.unwrap_or(StepBudgets { max_edge_visits: None, max_spikes_scheduled: Some(0) });
                        let cur = b.max_spikes_scheduled.unwrap_or(0);
                        b.max_spikes_scheduled = Some(cur.saturating_sub(10));
                        app.set_budgets(Some(b));
                    }
                    // toggle plasticity (if compiled)
                    #[cfg(feature = "plasticity")]
                    KeyCode::Char('p') => {
                        if !app.plast_on {
                            app.backend.enable_default_plasticity();
                            app.plast_on = true;
                        } else {
                            // one-way enable in this minimal example; disable path could be added by resetting backend
                        }
                    }
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