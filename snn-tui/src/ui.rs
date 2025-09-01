// TUI rendering: 2D spike raster (time on X, neuron IDs on Y) + status panel.

use std::io::Stdout;

use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Text,
    widgets::{Block, Borders, Paragraph},
    Terminal,
};

use crate::app::App;
use crate::backend::SnnBackend;

/// Draws the UI each frame:
/// - Top: Spike raster grid as rows (neurons) x columns (time, circular).
/// - Bottom: Status including tick, neuron count, run state, controls.
pub fn draw<B: SnnBackend>(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &App<B>,
) -> anyhow::Result<()> {
    terminal.draw(|f| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Percentage(80), Constraint::Percentage(20)].as_ref())
            .split(f.size());

        // Build raster lines. Each row: "nXX |....."
        let mut lines = Vec::with_capacity(app.raster.len());
        let mut buf = String::new();
        for (row_idx, row) in app.raster.iter().enumerate() {
            buf.clear();
            // label
            if row_idx < 10 {
                buf.push_str(&format!("n0{} |", row_idx));
            } else {
                buf.push_str(&format!("n{} |", row_idx));
            }
            // content
            for &ch in row.iter() {
                buf.push(ch);
            }
            lines.push(buf.clone());
        }

        let raster_text = Text::from(lines.join("\n"));
        let raster_widget = Paragraph::new(raster_text)
            .block(Block::default().title("Spike Raster  (time →)").borders(Borders::ALL))
            .style(Style::default().fg(Color::White));
        f.render_widget(raster_widget, chunks[0]);

        // Status and controls
        let status = format!(
            "Tick: {} | Neurons: {} | Running: {} | Controls: [s] Step  [r] Run/Pause  [q] Quit",
            app.tick,
            app.backend.neurons(),
            if app.running { "yes" } else { "no" }
        );
        let status_widget = Paragraph::new(status)
            .style(Style::default().fg(Color::Cyan))
            .block(Block::default().title("Status").borders(Borders::ALL));
        f.render_widget(status_widget, chunks[1]);
    })?;
    Ok(())
}